//! Gemini's live session: the recognizer runs while the user is still speaking.
//!
//! The batch path in [\`super::request\`] records first and uploads afterwards, so
//! every second of speech is answered for after the user stops. Measured through
//! this codebase on the same audio, a 60.8 s utterance cost 17.2 s after the stop
//! in that shape; streamed as it is spoken, the same audio costs 1.3 s and the
//! transcript is identical. A streaming recognizer consumes its input at its own
//! pace and cannot start before the bytes arrive, which is why sending early is
//! the only fix - bigger uploads still pay the whole decode after the stop.
//!
//! The wire protocol is the one captured in [\`super::request\`]: handshake, attach
//! the downstream long-poll, recognizer init, audio, end marker. What changes
//! here is timing and transport handling:
//!
//! * audio is posted as one Opus-in-WebM cluster per second of speech, so the
//!   container is written before its length is known:
//!   [\`WebmOpusStream\`];
//! * the downstream is a long-poll: the server ends it after about a minute even
//!   mid-utterance, so the reader re-attaches with the next AID. The recognizer
//!   keeps its state across that, and the transcript survives;
//! * a closing message only means the utterance is over once the end marker has
//!   been posted, so the reader ignores one that arrives earlier.

use anyhow::{anyhow, Result};
use magnum_opus::{Application, Channels, Encoder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use super::super::WhisperManager;
use super::request::{decorate, post_data, Session};
use crate::audio::AudioChunk;
use crate::live_provider::{Progress, ProgressSink};

/// Opus is native at 48 kHz, so resampling happens once, here.
const OPUS_SAMPLE_RATE: u32 = 48_000;

/// 20 ms at 48 kHz - one Opus frame.
const FRAME_SAMPLES: usize = 960;

/// One cluster per second, so one POST per second of speech. A POST costs a
/// round-trip, not bandwidth, and the recognizer consumes roughly real time.
const CLUSTER_MS: usize = 1_000;
const CLUSTER_FRAMES: usize = CLUSTER_MS / 20;

/// Handshake, upstream POSTs and the tail all cross the same link; any of them
/// taking longer than this means the session is dead, not slow.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const POST_TIMEOUT: Duration = Duration::from_secs(10);
const STREAM_READY_TIMEOUT: Duration = Duration::from_secs(10);

/// The recognizer answers almost immediately once the end marker arrives; this
/// is the outer bound for the whole flush.
const FINISH_TIMEOUT: Duration = Duration::from_secs(30);

/// How much of the accumulated downstream body is re-read for the in-progress
/// line. The final transcript is always read from the whole body.
const INTERIM_WINDOW_BYTES: usize = 16 * 1024;

// -- WebM-Opus, written as a stream -----------------------------------------

/// An Opus-in-WebM writer that never needs to know the total length.
///
/// The batch encoder can state the clip's duration in the header because it runs
/// once the recording is complete. A live session posts audio as it arrives, so
/// the Segment goes out with an unknown size and no Duration, which is what
/// MediaRecorder produces. Every element comes from the batch writer's own
/// builders, so there is one description of the container.
pub struct WebmOpusStream {
    encoder: Encoder,
    /// EBML header + Segment opener + Info + Tracks, prepended to the first
    /// cluster because nothing may be emitted before the first whole second.
    header: Vec<u8>,
    header_sent: bool,
    /// Captured samples not yet filling a whole cluster, at the recorder's rate.
    pending_pcm: Vec<f32>,
    /// The rate those samples were captured at. The device can change it between
    /// utterances, so the leftover is flushed at the rate it arrived with.
    pending_rate: u32,
    /// Encoded blocks of the cluster still being filled.
    pending_blocks: Vec<u8>,
    cluster_ms: u64,
    in_cluster: usize,
}

impl WebmOpusStream {
    pub fn new() -> Result<Self> {
        let encoder = Encoder::new(OPUS_SAMPLE_RATE, Channels::Mono, Application::Voip)
            .map_err(|e| anyhow!("Failed to create Opus encoder: {}", e))?;

        let mut header = Vec::new();
        WhisperManager::ebml_elem(
            &mut header,
            &[0x1A, 0x45, 0xDF, 0xA3],
            &WhisperManager::webm_header(),
        );

        let mut info = Vec::new();
        WhisperManager::ebml_elem(
            &mut info,
            &[0x2A, 0xD7, 0xB1],
            &WhisperManager::ebml_uint(1_000_000),
        ); // TimestampScale
        WhisperManager::ebml_elem(&mut info, &[0x4D, 0x80], b"QuillScribe"); // MuxingApp
        WhisperManager::ebml_elem(&mut info, &[0x57, 0x41], b"QuillScribe"); // WritingApp

        // Segment with the "unknown size" VINT: the length is only known once
        // the user stops, and by then the bytes have already been posted.
        header.extend_from_slice(&[
            0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        WhisperManager::ebml_elem(&mut header, &[0x15, 0x49, 0xA9, 0x66], &info);
        WhisperManager::ebml_elem(
            &mut header,
            &[0x16, 0x54, 0xAE, 0x6B],
            &WhisperManager::webm_tracks(),
        );

        Ok(Self {
            encoder,
            header,
            header_sent: false,
            pending_pcm: Vec::new(),
            pending_rate: OPUS_SAMPLE_RATE,
            pending_blocks: Vec::new(),
            cluster_ms: 0,
            in_cluster: 0,
        })
    }

    /// Add captured samples, appending every cluster that became complete.
    ///
    /// The rate travels with each block because the recorder only learns the
    /// device rate once its stream is running.
    pub fn push(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        out: &mut Vec<Vec<u8>>,
    ) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        self.pending_pcm.extend_from_slice(samples);
        self.pending_rate = sample_rate;

        // A cluster is one second of audio as the recorder produced it. Slicing
        // the captured samples first keeps the resampler's output a whole number
        // of 20 ms frames.
        let per_cluster = (sample_rate as u64 * CLUSTER_MS as u64 / 1000) as usize;
        while per_cluster > 0 && self.pending_pcm.len() >= per_cluster {
            let block: Vec<f32> = self.pending_pcm.drain(..per_cluster).collect();
            self.encode_block(&block, sample_rate, out)?;
        }
        Ok(())
    }

    /// Encode whatever is left and emit it as a final, shorter cluster.
    pub fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<()> {
        if !self.pending_pcm.is_empty() {
            let block = std::mem::take(&mut self.pending_pcm);
            self.encode_block(&block, self.pending_rate, out)?;
        }
        if !self.pending_blocks.is_empty() {
            self.emit_cluster(out);
        }
        Ok(())
    }

    fn encode_block(
        &mut self,
        block: &[f32],
        sample_rate: u32,
        out: &mut Vec<Vec<u8>>,
    ) -> Result<()> {
        let samples: Vec<i16> = WhisperManager::resample(block, sample_rate, OPUS_SAMPLE_RATE)
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        for chunk in samples.chunks(FRAME_SAMPLES) {
            let mut frame = chunk.to_vec();
            frame.resize(FRAME_SAMPLES, 0); // libopus needs a full frame
            let packet = self
                .encoder
                .encode_vec(&frame, 4000)
                .map_err(|e| anyhow!("Opus encode failed: {}", e))?;

            let rel = ((self.in_cluster % CLUSTER_FRAMES) * 20) as i16;
            let mut frame_block = Vec::with_capacity(packet.len() + 4);
            frame_block.push(0x81); // track number 1 (VINT)
            frame_block.extend_from_slice(&rel.to_be_bytes());
            frame_block.push(0x80); // flags: keyframe
            frame_block.extend_from_slice(&packet);
            WhisperManager::ebml_elem(&mut self.pending_blocks, &[0xA3], &frame_block);

            self.in_cluster += 1;
            if self.in_cluster == CLUSTER_FRAMES {
                self.emit_cluster(out);
            }
        }
        Ok(())
    }

    fn emit_cluster(&mut self, out: &mut Vec<Vec<u8>>) {
        let mut cluster = Vec::new();
        WhisperManager::ebml_elem(
            &mut cluster,
            &[0xE7],
            &WhisperManager::ebml_uint(self.cluster_ms),
        );
        cluster.extend_from_slice(&self.pending_blocks);

        let mut element = Vec::new();
        WhisperManager::ebml_elem(&mut element, &[0x1F, 0x43, 0xB6, 0x75], &cluster);

        self.pending_blocks.clear();
        self.cluster_ms += (CLUSTER_FRAMES * 20) as u64;
        self.in_cluster = 0;

        if !self.header_sent {
            let mut first = std::mem::take(&mut self.header);
            first.extend_from_slice(&element);
            element = first;
            self.header_sent = true;
        }
        out.push(element);
    }
}

// -- The downstream long-poll ----------------------------------------------

/// Read the downstream long-poll, re-attaching when the server closes it, and
/// hand back the accumulated body once the recognizer reports it is done.
///
/// The server ends an attach after about a minute even while audio keeps
/// arriving, and the recognizer keeps its state across the next one, so the loop
/// asks for the next stream with the next AID. A closing message only counts
/// once the end marker has been posted - the earlier ones belong to the
/// utterance still in progress.
fn spawn_downstream(
    session: Session,
    ended: Arc<AtomicBool>,
    mut stop_rx: oneshot::Receiver<()>,
    on_progress: ProgressSink,
) -> (oneshot::Receiver<()>, oneshot::Receiver<Vec<u8>>) {
    let (ready_tx, ready_rx) = oneshot::channel();
    let (tail_tx, tail_rx) = oneshot::channel();

    tokio::spawn(async move {
        let mut ready = Some(ready_tx);
        let tail = Some(tail_tx);
        let mut body: Vec<u8> = Vec::new();
        let mut last_interim = String::new();
        let mut aid = 0u64;

        loop {
            let url = format!(
                "{}?gsessionid={}&VER=8&RID=rpc&SID={}&AID={}&CI=0&TYPE=xmlhttp&zx={}&t=1",
                WhisperManager::GEMINI_S3_BASE,
                session.gsid,
                session.sid,
                aid,
                session.zx
            );
            let request = decorate(WhisperManager::http_client().get(url), &session.hdrs);
            let Ok(mut response) = request.send().await else {
                break;
            };
            if !response.status().is_success() {
                break;
            }

            let mut done = false;
            loop {
                // The caller abandons the session on every failure path; without
                // this the task would keep re-attaching for the life of the app.
                let chunk = tokio::select! {
                    biased;
                    _ = &mut stop_rx => return,
                    chunk = response.chunk() => chunk,
                };
                match chunk {
                    Ok(Some(chunk)) => {
                        body.extend_from_slice(&chunk);
                        if let Some(tx) = ready.take() {
                            let _ = tx.send(());
                        }
                        if is_finished(&body) && ended.load(Ordering::SeqCst) {
                            done = true;
                            break;
                        }
                        report_interim(&body, &on_progress, &mut last_interim);
                    }
                    // The server closed this attach: ask for the next one.
                    Ok(None) | Err(_) => break,
                }
            }

            if done {
                break;
            }
            aid += 1;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if let Some(tx) = tail {
            let _ = tx.send(body);
        }
    });

    (ready_rx, tail_rx)
}

/// The recognizer's closing messages: "transcript produced" and "accepted, but
/// nothing recognized".
fn is_finished(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body);
    text.contains("\"CAEoAw") || text.contains("\"CAEoAg")
}

/// Show what the recognizer has so far while the user is still speaking.
fn report_interim(body: &[u8], on_progress: &ProgressSink, last: &mut String) {
    let text = String::from_utf8_lossy(body);
    let Some(found) = WhisperManager::parse_webchannel_transcript(tail_window(&text)) else {
        return;
    };
    if found != *last {
        last.clear();
        last.push_str(&found);
        on_progress(Progress::Interim(found));
    }
}

/// The last INTERIM_WINDOW_BYTES of the body, on a character boundary.
fn tail_window(body: &str) -> &str {
    if body.len() <= INTERIM_WINDOW_BYTES {
        return body;
    }
    let mut start = body.len() - INTERIM_WINDOW_BYTES;
    while start < body.len() && !body.is_char_boundary(start) {
        start += 1;
    }
    &body[start..]
}

// -- The session the app holds ---------------------------------------------

/// A running live session.
///
/// Created at record-start, fed by the audio tap, and consumed by
/// [\`LiveSession::finish\`], which posts the tail and waits for the transcript.
pub struct LiveSession {
    command_tx: mpsc::UnboundedSender<SessionCommand>,
    result_rx: oneshot::Receiver<Result<String>>,
}

enum SessionCommand {
    /// The user stopped: flush the tail, post the end marker, collect the text.
    Stop,
}

impl LiveSession {
    /// Start a session. Audio arrives through the tap tagged with the recorder's
    /// own rate; the WebM writer resamples it to 48 kHz itself.
    ///
    /// The manager carries the key and the language the recognizer is built
    /// with, including the fallback to the key the Gemini web app ships. The
    /// runtime handle is passed in because this is called from the Slint UI
    /// thread, which is outside any Tokio runtime context.
    pub fn start(
        runtime: tokio::runtime::Handle,
        manager: WhisperManager,
        audio_rx: mpsc::UnboundedReceiver<AudioChunk>,
        on_progress: ProgressSink,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = oneshot::channel();

        runtime.spawn(async move {
            let result = run_session(manager, audio_rx, command_rx, on_progress).await;
            let _ = result_tx.send(result);
        });

        Self {
            command_tx,
            result_rx,
        }
    }

    /// Ask the session to flush and return the transcript.
    pub async fn finish(self) -> Result<String> {
        let _ = self.command_tx.send(SessionCommand::Stop);
        match tokio::time::timeout(FINISH_TIMEOUT, self.result_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(anyhow!("Gemini session ended without a result")),
            Err(_) => Err(anyhow!(
                "Gemini did not finish within {} seconds",
                FINISH_TIMEOUT.as_secs()
            )),
        }
    }
}

async fn run_session(
    manager: WhisperManager,
    mut audio_rx: mpsc::UnboundedReceiver<AudioChunk>,
    mut command_rx: mpsc::UnboundedReceiver<SessionCommand>,
    on_progress: ProgressSink,
) -> Result<String> {
    let session = tokio::time::timeout(HANDSHAKE_TIMEOUT, manager.open_session())
        .await
        .map_err(|_| anyhow!("Gemini handshake timed out"))??;
    let language = manager.api_language.clone();

    // The server rejects uploads while no listener is attached, so the
    // downstream opens before the init message and is never torn down before the
    // end marker has been answered.
    let ended = Arc::new(AtomicBool::new(false));
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let (ready_rx, tail_rx) = spawn_downstream(
        Session {
            gsid: session.gsid.clone(),
            sid: session.sid.clone(),
            zx: session.zx.clone(),
            hdrs: session.hdrs.clone(),
        },
        Arc::clone(&ended),
        stop_rx,
        on_progress,
    );

    // Every way out of an utterance - the transcript, a dead connection, no
    // audio at all - has to close the downstream task, so the session body is
    // one block whose result is returned after the task has been stood down.
    let outcome = async {
        tokio::time::timeout(STREAM_READY_TIMEOUT, ready_rx)
            .await
            .map_err(|_| anyhow!("Gemini stream did not open"))?
            .map_err(|_| anyhow!("Gemini stream closed before it opened"))?;

        WhisperManager::send_init(&session, &language).await?;

        let mut encoder = WebmOpusStream::new()?;
        let mut heard_samples = 0usize;
        let mut rid = 10002u64;
        let mut ofs = 1u64;

        // Everything the mic produces is posted as it arrives, so the words are
        // already at Google by the time the user stops talking.
        loop {
            tokio::select! {
                command = command_rx.recv() => match command {
                    Some(SessionCommand::Stop) => break,
                    None => {
                        return Err(anyhow!("Gemini session was dropped before it finished"))
                    }
                },
                chunk = audio_rx.recv() => match chunk {
                    Some(chunk) => {
                        let mut clusters = Vec::new();
                        heard_samples += chunk.samples.len();
                        encoder.push(&chunk.samples, chunk.sample_rate, &mut clusters)?;
                        for cluster in clusters {
                            post_audio(&session, &mut rid, &mut ofs, &cluster).await?;
                        }
                    }
                    None => break,
                },
            }
        }

        // The tap is detached before the stop path flushes, so anything it
        // already handed over is still in the channel and belongs to this
        // utterance.
        let mut clusters = Vec::new();
        while let Ok(chunk) = audio_rx.try_recv() {
            heard_samples += chunk.samples.len();
            encoder.push(&chunk.samples, chunk.sample_rate, &mut clusters)?;
        }
        if heard_samples == 0 {
            // Nothing reached the recognizer, so there is no utterance to
            // finish. Reporting an empty success would hide a dead microphone.
            return Err(anyhow!("No audio to encode."));
        }
        encoder.finish(&mut clusters)?;
        for cluster in clusters {
            post_audio(&session, &mut rid, &mut ofs, &cluster).await?;
        }

        // The end marker is what makes the recognizer commit; until it is
        // posted, a closing message would only be the mid-utterance kind.
        ended.store(true, Ordering::SeqCst);
        let _ = post_data(&session, rid, ofs, 1, "GAE=")
            .timeout(POST_TIMEOUT)
            .send()
            .await;

        let body = tokio::time::timeout(FINISH_TIMEOUT, tail_rx)
            .await
            .map_err(|_| anyhow!("Gemini did not return a transcript in time"))?
            .map_err(|_| anyhow!("Gemini stream ended without a transcript"))?;

        WhisperManager::transcript_from(&body)
    }
    .await;

    ended.store(true, Ordering::SeqCst);
    let _ = stop_tx.send(());
    outcome
}

/// Post one WebChannel audio message and advance the request counters.
async fn post_audio(session: &Session, rid: &mut u64, ofs: &mut u64, cluster: &[u8]) -> Result<()> {
    let envelope = WhisperManager::base64_encode(&WhisperManager::build_audio_chunk(cluster));
    let response = post_data(session, *rid, *ofs, 1, &envelope)
        .timeout(POST_TIMEOUT)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Gemini rejected audio message {}: HTTP {}",
            *rid,
            response.status()
        ));
    }
    *rid += 1;
    *ofs += 1;
    Ok(())
}
