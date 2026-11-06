//! The socket side of a live session: connecting, feeding audio, reconnecting.
//!
//! Everything here is transport. What a frame means is decided in the parent
//! module by [`parse_server_message`], so the protocol rules and the reconnect
//! rules can each be read on their own.
//!
//! One file rather than several because these pieces share the session state
//! they all rewrite: the socket, the loop that reads it, and the reconnect that
//! replays what was already said.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, Instant};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::{
    build_listen_url, parse_server_message, ServerMessage, CLOSE_TIMEOUT, FINALIZE_TIMEOUT,
    FINISH_TIMEOUT, KEEPALIVE_MIN_IDLE, KEEPALIVE_PERIOD, MAX_RECONNECT_ATTEMPTS,
    MAX_REPLAY_SAMPLES, TARGET_SAMPLE_RATE,
};
use crate::audio::AudioChunk;
use crate::config::DeepgramConfig;
use crate::live_provider::{Progress, ProgressSink};
use crate::whisper::WhisperManager;

/// How much audio to accumulate before sending a frame. Deepgram recommends
/// 20-100 ms; 100 ms keeps the frame count low without adding audible lag.
const BLOCK_MS: u32 = 100;

/// What the UI asks a running session to do.
#[derive(Debug)]
pub(crate) enum SessionCommand {
    Stop,
}

pub(super) type WsSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
pub(super) type WsSink = futures_util::stream::SplitSink<WsSocket, Message>;
pub(super) type WsStream = futures_util::stream::SplitStream<WsSocket>;
/// One frame as the socket reads it, error included.
pub(super) type WsMessage = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>;

/// A running live transcription session.
///
/// Created at record-start, fed by the audio tap, and consumed by [`finish`],
/// which flushes the tail and returns the committed transcript.
pub struct LiveSession {
    command_tx: mpsc::UnboundedSender<SessionCommand>,
    result_rx: oneshot::Receiver<Result<String>>,
}

impl LiveSession {
    /// Start a session. Audio arrives through `audio_rx` already tagged with the
    /// recorder's native rate; the session resamples to 16 kHz itself.
    ///
    /// The runtime handle is passed in because this is called from the Slint UI
    /// thread, which is outside any Tokio runtime context.
    pub fn start(
        runtime: tokio::runtime::Handle,
        cfg: DeepgramConfig,
        audio_rx: mpsc::UnboundedReceiver<AudioChunk>,
        on_progress: ProgressSink,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = oneshot::channel();

        runtime.spawn(async move {
            let result = run_session(cfg, audio_rx, command_rx, on_progress).await;
            let _ = result_tx.send(result);
        });

        Self {
            command_tx,
            result_rx,
        }
    }

    /// Ask the session to flush and close, then wait for the transcript.
    ///
    /// `Finalize` is not optional: skipping it is the documented cause of the
    /// last words going missing, because the tail is still in Deepgram's buffer.
    pub async fn finish(self) -> Result<String> {
        let _ = self.command_tx.send(SessionCommand::Stop);
        match tokio::time::timeout(FINISH_TIMEOUT, self.result_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(anyhow!("Deepgram session ended without a result")),
            Err(_) => Err(anyhow!(
                "Deepgram did not finish within {} seconds",
                FINISH_TIMEOUT.as_secs()
            )),
        }
    }
}

/// Everything that survives a reconnect.
pub(crate) struct SessionState {
    /// 16 kHz mono PCM for the whole utterance, kept so a dropped socket can be
    /// replayed from the beginning (Deepgram cannot resume a session).
    pub(crate) pcm: Vec<i16>,
    /// Committed text for the *current* connection. Cleared on reconnect, since
    /// the replay re-transcribes from the start.
    pub(crate) finals: Vec<String>,
    /// Finals the sink has already been given, for the whole utterance. It is
    /// not cleared on reconnect, because the replay re-reports the sentences an
    /// earlier connection already reported.
    emitted: Vec<String>,
}

impl SessionState {
    pub(crate) fn new() -> Self {
        Self {
            pcm: Vec::new(),
            finals: Vec::new(),
            emitted: Vec::new(),
        }
    }

    pub(crate) fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            self.pcm.push((clamped * i16::MAX as f32) as i16);
        }
    }

    pub(crate) fn commit(&mut self, text: &str) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            self.finals.push(trimmed.to_string());
        }
    }

    /// Whether an earlier connection already reported this final.
    ///
    /// `finals` holds the current connection's transcript and resets on a
    /// reconnect, so a final's position in it is the position it had in the
    /// utterance: after a replay, position 0 is the first sentence again.
    pub(crate) fn is_replay(&self, text: &str) -> bool {
        let position = self.finals.len().saturating_sub(1);
        self.emitted.get(position).map(String::as_str) == Some(text.trim())
    }

    pub(crate) fn remember_emitted(&mut self, text: &str) {
        self.emitted.push(text.trim().to_string());
    }

    pub(crate) fn transcript(&self) -> String {
        // Segments are already trimmed; join with a single space and collapse
        // any double spaces punctuation may have left behind.
        self.finals
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub(super) async fn run_session(
    cfg: DeepgramConfig,
    mut audio_rx: mpsc::UnboundedReceiver<AudioChunk>,
    mut command_rx: mpsc::UnboundedReceiver<SessionCommand>,
    on_progress: ProgressSink,
) -> Result<String> {
    if cfg.api_key.trim().is_empty() {
        bail!("No Deepgram API key. Add one in Settings â†’ Engine.");
    }

    let url = build_listen_url(&cfg);
    // The URL carries no secret and is the single best answer to "why is my
    // punctuation not working?" â€” it shows exactly which flags were requested.
    log::info!("Deepgram session: {}", url);
    let mut state = SessionState::new();
    let mut attempt = 0u32;

    loop {
        match run_connection(
            &cfg,
            &url,
            &mut state,
            &mut audio_rx,
            &mut command_rx,
            &on_progress,
        )
        .await
        {
            Ok(transcript) => return Ok(transcript),
            Err(err) if attempt < MAX_RECONNECT_ATTEMPTS => {
                if state.pcm.len() > MAX_REPLAY_SAMPLES {
                    return Err(anyhow!(
                        "Lost the Deepgram connection after {} minutes of audio, which is too \
                         long to replay. The transcript up to that point is lost. ({})",
                        state.pcm.len() / TARGET_SAMPLE_RATE as usize / 60,
                        err
                    ));
                }
                attempt += 1;
                (on_progress)(Progress::Status(format!(
                    "Deepgram connection lost â€” reconnecting ({}/{})â€¦",
                    attempt, MAX_RECONNECT_ATTEMPTS
                )));
                tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
                log::warn!("Deepgram session dropped, replaying: {}", err);
            }
            Err(err) => return Err(err),
        }
    }
}

/// Where one socket stands between the audio it hears and the frames it reads.
pub(super) struct Connection<'a> {
    sink: WsSink,
    stream: WsStream,
    state: &'a mut SessionState,
    on_progress: &'a ProgressSink,
    /// Samples waiting for a whole send block, and the rate they arrived at.
    pending: Vec<f32>,
    pending_rate: u32,
    /// How much of `state.pcm` has reached the socket.
    sent: usize,
    last_send: Instant,
    /// Cleared when the recorder's tap goes away; the socket text keeps
    /// draining after that.
    audio_open: bool,
}

impl<'a> Connection<'a> {
    /// Connect, and replay everything recorded so far.
    ///
    /// A reconnect begins a brand new session with timestamps reset to zero, so
    /// the only way to keep the transcript whole is to send everything again.
    async fn open(
        cfg: &DeepgramConfig,
        url: &str,
        state: &'a mut SessionState,
        on_progress: &'a ProgressSink,
    ) -> Result<Self> {
        let (mut sink, stream) = connect(cfg, url).await?;
        state.finals.clear();
        let mut sent = 0usize;
        flush_audio(&mut sink, &state.pcm, &mut sent).await?;

        Ok(Self {
            sink,
            stream,
            state,
            on_progress,
            pending: Vec::new(),
            pending_rate: TARGET_SAMPLE_RATE,
            sent,
            last_send: Instant::now(),
            audio_open: true,
        })
    }
    /// End the utterance when the command channel says so.
    ///
    /// `Ok(None)` means keep listening: the command was not a stop.
    async fn on_command(&mut self, command: Option<SessionCommand>) -> Result<Option<String>> {
        // `Stop`, or the sender being dropped, both mean "end now".
        if command.is_some() && !matches!(command, Some(SessionCommand::Stop)) {
            return Ok(None);
        }
        if !self.pending.is_empty() {
            let rate = self.pending_rate;
            let tail = WhisperManager::resample(&self.pending, rate, TARGET_SAMPLE_RATE);
            self.state.push_samples(&tail);
            flush_audio(&mut self.sink, &self.state.pcm, &mut self.sent).await?;
        }
        finish(
            &mut self.sink,
            &mut self.stream,
            self.state,
            self.on_progress,
        )
        .await
        .map(Some)
    }

    /// Buffer one chunk, or notice that the recorder's tap went away.
    ///
    /// A missing tap keeps the socket text draining instead of spinning on a
    /// closed channel, and a chunk with no rate carries no samples anyone could
    /// send.
    async fn on_audio(&mut self, chunk: Option<AudioChunk>) -> Result<()> {
        let Some(chunk) = chunk else {
            self.audio_open = false;
            return Ok(());
        };
        if chunk.sample_rate == 0 {
            return Ok(());
        }
        send_audio(
            &mut self.sink,
            self.state,
            &mut self.sent,
            &mut self.pending,
            &mut self.pending_rate,
            chunk,
        )
        .await?;
        self.last_send = Instant::now();
        Ok(())
    }

    /// Fold in one frame, or end the connection because the socket closed or
    /// failed.
    fn on_socket(&mut self, message: Option<WsMessage>) -> Result<()> {
        match message {
            Some(Ok(Message::Text(text))) => {
                apply_frame(&text, self.state, self.on_progress)?;
                Ok(())
            }
            Some(Ok(Message::Close(_))) | None => Err(anyhow!("Deepgram closed the connection")),
            Some(Err(err)) => Err(anyhow!("Deepgram socket error: {}", err)),
            _ => Ok(()),
        }
    }

    /// Send a keep-alive when the socket has been quiet long enough.
    async fn on_keepalive(&mut self) -> Result<()> {
        if self.last_send.elapsed() < KEEPALIVE_MIN_IDLE {
            return Ok(());
        }
        self.sink
            .send(Message::text("{\"type\":\"KeepAlive\"}"))
            .await
            .context("Could not send a Deepgram keep-alive")?;
        self.last_send = Instant::now();
        Ok(())
    }
}

/// Open the socket, with the key in the header rather than the URL.
pub(super) async fn connect(cfg: &DeepgramConfig, url: &str) -> Result<(WsSink, WsStream)> {
    let mut request = url
        .into_client_request()
        .context("Could not build the Deepgram request")?;
    request.headers_mut().insert(
        "Authorization",
        format!("Token {}", cfg.api_key.trim())
            .parse()
            .context("Deepgram API key contains characters that are not valid in a header")?,
    );

    let (socket, _response) = tokio_tungstenite::connect_async(request)
        .await
        .context("Could not connect to Deepgram")?;
    Ok(socket.split())
}

/// Run one socket from connect to close.
///
/// `Ok` means the user stopped recording and the transcript is final. `Err`
/// means the connection failed and the caller should reconnect and replay.
pub(super) async fn run_connection(
    cfg: &DeepgramConfig,
    url: &str,
    state: &mut SessionState,
    audio_rx: &mut mpsc::UnboundedReceiver<AudioChunk>,
    command_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
    on_progress: &ProgressSink,
) -> Result<String> {
    let mut connection = Connection::open(cfg, url, state, on_progress).await?;
    let mut keepalive = interval(KEEPALIVE_PERIOD);
    keepalive.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                if let Some(transcript) = connection.on_command(command).await? {
                    return Ok(transcript);
                }
            }

            chunk = audio_rx.recv(), if connection.audio_open => {
                connection.on_audio(chunk).await?;
            }

            message = connection.stream.next() => {
                let () = connection.on_socket(message)?;
            }

            _ = keepalive.tick() => connection.on_keepalive().await?,
        }
    }
}

/// Buffer one chunk and send every whole block it completes.
///
/// Deepgram reads audio in blocks of `BLOCK_MS`, so a chunk that arrives in two
/// halves is held until the second half completes the block.
pub(super) async fn send_audio(
    sink: &mut WsSink,
    state: &mut SessionState,
    sent: &mut usize,
    pending: &mut Vec<f32>,
    pending_rate: &mut u32,
    chunk: AudioChunk,
) -> Result<()> {
    *pending_rate = chunk.sample_rate;
    pending.extend_from_slice(&chunk.samples);
    let block_samples = ((*pending_rate as usize) * BLOCK_MS as usize / 1000).max(1);
    while pending.len() >= block_samples {
        let rest = pending.split_off(block_samples);
        let block = std::mem::replace(pending, rest);
        let resampled = WhisperManager::resample(&block, *pending_rate, TARGET_SAMPLE_RATE);
        state.push_samples(&resampled);
        flush_audio(sink, &state.pcm, sent).await?;
    }
    Ok(())
}

/// One text frame: an error ends the connection, anything else is folded into
/// the session and reported.
pub(super) fn apply_frame(
    raw: &str,
    state: &mut SessionState,
    on_progress: &ProgressSink,
) -> Result<()> {
    if let ServerMessage::Error(err) = parse_server_message(raw) {
        bail!("Deepgram: {}", err);
    }
    apply_message(raw, state, on_progress);
    Ok(())
}

/// Flush the tail, close the stream, and collect the final transcript.
///
/// Order matters: `Finalize` recovers the audio still in Deepgram's buffer,
/// then `CloseStream` reports and terminates.
pub(super) async fn finish(
    sink: &mut WsSink,
    stream: &mut WsStream,
    state: &mut SessionState,
    on_progress: &ProgressSink,
) -> Result<String> {
    sink.send(Message::text("{\"type\":\"Finalize\"}"))
        .await
        .context("Could not finalize the Deepgram stream")?;

    let deadline = Instant::now() + FINALIZE_TIMEOUT;
    let mut finalized = false;
    while !finalized {
        match tokio::time::timeout_at(deadline, stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                // Deepgram documents the Finalize response as usually present
                // but not guaranteed, so waiting only for `from_finalize` would
                // cost the full timeout on every dictation. Any final after
                // Finalize is the flushed tail; anything still pending is
                // collected by the CloseStream drain below.
                let is_final = matches!(parse_server_message(&text), ServerMessage::Final { .. });
                apply_message(&text, state, on_progress);
                finalized = is_final;
            }
            // Any error, close, or timeout means we stop waiting and take what
            // we have; the audio is still locally buffered either way.
            _ => break,
        }
    }

    sink.send(Message::text("{\"type\":\"CloseStream\"}"))
        .await
        .context("Could not close the Deepgram stream")?;

    let deadline = Instant::now() + CLOSE_TIMEOUT;
    while let Ok(Some(Ok(Message::Text(text)))) =
        tokio::time::timeout_at(deadline, stream.next()).await
    {
        let parsed = parse_server_message(&text);
        apply_message(&text, state, on_progress);
        if matches!(parsed, ServerMessage::Metadata) {
            break;
        }
    }

    let _ = sink.close().await;

    let transcript = state.transcript();
    if transcript.is_empty() {
        bail!("Deepgram returned no transcript");
    }
    Ok(transcript)
}

/// Fold one parsed message into session state and report it to the UI.
pub(crate) fn apply_message(raw: &str, state: &mut SessionState, on_progress: &ProgressSink) {
    log::debug!("deepgram frame: {}", raw);
    match parse_server_message(raw) {
        ServerMessage::Interim(text) if !text.is_empty() => {
            (on_progress)(Progress::Interim(text));
        }
        ServerMessage::Final { text, .. } => {
            // Trimmed once, so the transcript and the replay guard always see
            // the same string and cannot drift apart by one position.
            let text = text.trim();
            if text.is_empty() {
                return;
            }
            state.commit(text);
            if state.is_replay(text) {
                log::debug!("Suppressing a replayed final: {}", text);
                return;
            }
            state.remember_emitted(text);
            (on_progress)(Progress::Final(text.to_string()));
        }
        // Turn boundaries are not commits; nothing to do but ignore them.
        _ => {}
    }
}

pub(super) async fn flush_audio(sink: &mut WsSink, pcm: &[i16], sent: &mut usize) -> Result<()> {
    if *sent >= pcm.len() {
        return Ok(());
    }
    let mut bytes = Vec::with_capacity((pcm.len() - *sent) * 2);
    for sample in &pcm[*sent..] {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    *sent = pcm.len();
    sink.send(Message::binary(bytes))
        .await
        .context("Could not send audio to Deepgram")?;
    Ok(())
}
