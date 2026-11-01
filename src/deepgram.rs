//! Deepgram live-session provider.
//!
//! Deepgram is the only provider QuillScribe talks to while the user is still
//! speaking. Every other provider records first and transcribes afterwards, so
//! all of their work happens once the user stops; here the transcript accrues
//! during the utterance and only one flush round-trip remains at the end.
//! See `docs/adr/0001-deepgram-live-streaming-only.md`.
//!
//! Wire summary (verified against the Deepgram API reference):
//!
//! * audio is sent as **raw binary** WebSocket frames of 16 kHz mono
//!   little-endian `linear16` PCM â€” no base64, no JSON envelope;
//! * text frames carry control messages only (`KeepAlive`, `Finalize`,
//!   `CloseStream`);
//! * `Authorization: Token <KEY>` is set on the handshake, so no browser
//!   subprotocol trick is needed;
//! * the server closes the socket after ~10s without audio unless a
//!   `KeepAlive` text frame arrives;
//! * `is_final` is the commit signal, `speech_final`/`UtteranceEnd` are turn
//!   boundaries and must never be treated as commits.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, Instant};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use crate::audio::AudioChunk;
use crate::config::DeepgramConfig;
use crate::whisper::WhisperManager;

/// Deepgram accepts 16 kHz mono `linear16` for speech, and every model is
/// trained around it.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// How much audio to accumulate before sending a frame. Deepgram recommends
/// 20â€“100 ms; 100 ms keeps the frame count low without adding audible lag.
const BLOCK_MS: u32 = 100;

/// Sending nothing for ~10 s gets the socket closed with `NET-0001`, so stay
/// comfortably inside that window while the user pauses.
const KEEPALIVE_PERIOD: Duration = Duration::from_secs(4);
const KEEPALIVE_MIN_IDLE: Duration = Duration::from_secs(2);

/// `Finalize` answers almost immediately; `CloseStream` sends the final
/// `Metadata` before terminating.
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(2);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);
const FINISH_TIMEOUT: Duration = Duration::from_secs(10);

/// Deepgram has no session resumption: a reconnect starts a fresh session with
/// timestamps reset, so we rebuild the transcript by replaying our own audio.
const MAX_RECONNECT_ATTEMPTS: u32 = 2;

/// How much audio is worth replaying after a dropped socket. Deepgram cannot
/// resume a session, so recovery means re-uploading everything â€” which is only
/// sane for something dictation-shaped. Beyond this we stop pretending and
/// report the failure instead of silently truncating someone's words.
const MAX_REPLAY_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 60 * 5;

// â”€â”€ Progress reported to the UI â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// What the session tells the rest of the app while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Text that may still change; replaces the in-progress line.
    Interim(String),
    /// Text the provider committed to.
    Final(String),
    /// Human-readable status for the status line.
    Status(String),
}

// â”€â”€ Server messages â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// A parsed message from Deepgram's live socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerMessage {
    /// `is_final: false` â€” may still change.
    Interim(String),
    /// `is_final: true` â€” committed. `from_finalize` marks the response to our
    /// `Finalize`, which is the signal that the flushed tail has arrived.
    Final { text: String, from_finalize: bool },
    /// The speaker paused. Not a commit signal.
    TurnBoundary,
    /// Closing summary; arrives after `CloseStream`.
    Metadata,
    /// `vad_events` notification.
    SpeechStarted,
    /// Response to a mid-stream `Configure`.
    Configured,
    /// A fatal error reported by the service.
    Error(String),
    /// Anything we do not need to act on.
    Ignored,
}

/// Walk a JSON path, supporting both object keys and array indices.
///
/// `Value::get(&str)` only indexes objects, so `channel.alternatives.0.transcript`
/// needs the array case handled explicitly.
fn text_at(v: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = v;
    for key in path {
        cursor = match cursor {
            Value::Array(items) => items.get(key.parse::<usize>().ok()?)?,
            _ => cursor.get(*key)?,
        };
    }
    cursor.as_str().map(|s| s.to_string())
}

/// Parse one server text frame.
///
/// Note the shape: live responses have **no `results` wrapper and no `channels`
/// array** â€” that is the prerecorded shape. Live nests the transcript at
/// `channel.alternatives[0].transcript`, and interim messages carry no `type`
/// field at all, so `type` being absent still means "a Results message".
pub fn parse_server_message(raw: &str) -> ServerMessage {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return ServerMessage::Ignored;
    };

    match value.get("type").and_then(Value::as_str) {
        Some("Metadata") => ServerMessage::Metadata,
        Some("UtteranceEnd") => ServerMessage::TurnBoundary,
        Some("SpeechStarted") => ServerMessage::SpeechStarted,
        Some("ConfigureSuccess") | Some("ConfigureFailure") => ServerMessage::Configured,
        Some("Error") => ServerMessage::Error(
            text_at(&value, &["description"])
                .or_else(|| text_at(&value, &["message"]))
                .or_else(|| text_at(&value, &["err_msg"]))
                .unwrap_or_else(|| raw.trim().to_string()),
        ),
        // "Results", or a message with no `type` at all (interims).
        _ => {
            let text = text_at(&value, &["channel", "alternatives", "0", "transcript"])
                .map(|t| t.trim().to_string())
                .unwrap_or_default();
            let is_final = value
                .get("is_final")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let from_finalize = value
                .get("from_finalize")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            if is_final {
                ServerMessage::Final {
                    text,
                    from_finalize,
                }
            } else {
                ServerMessage::Interim(text)
            }
        }
    }
}

// â”€â”€ URL building â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Percent-encode the few characters that could appear in a model or language
/// code. Deliberately tiny: everything else Deepgram accepts is unreserved.
fn encode_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

/// Flags that mean the same thing on both endpoints.
fn formatting_params(cfg: &DeepgramConfig) -> Vec<(&'static str, String)> {
    let mut params = vec![
        ("model", encode_query_value(&cfg.model)),
        ("language", encode_query_value(&cfg.language)),
    ];

    if cfg.smart_format {
        params.push(("smart_format", "true".to_string()));
    }
    if cfg.punctuate || cfg.dictation {
        params.push(("punctuate", "true".to_string()));
    }
    if cfg.dictation {
        params.push(("dictation", "true".to_string()));
    }

    params
}

fn assemble(base: &str, params: Vec<(&str, String)>) -> String {
    let query = params
        .into_iter()
        .map(|(key, value)| format!("{}={}", key, value))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", base, query)
}

/// Build the prerecorded (recorded-upload) URL.
///
/// The audio is sent as a complete WAV, so the raw-PCM parameters
/// (`encoding`, `sample_rate`, `channels`) are omitted and Deepgram detects
/// the format itself. Live-only parameters make no sense here.
pub fn build_recorded_url(cfg: &DeepgramConfig) -> String {
    assemble("https://api.deepgram.com/v1/listen", formatting_params(cfg))
}

/// Build the live-session URL for the given settings.
///
/// Only flags that are *on* are emitted, so the URL also shows exactly what was
/// requested. `punctuate` is forced on whenever `dictation` is on, because
/// Deepgram's spoken-punctuation formatter does nothing without it.
pub fn build_listen_url(cfg: &DeepgramConfig) -> String {
    let mut params = formatting_params(cfg);
    params.push(("encoding", "linear16".to_string()));
    params.push(("sample_rate", TARGET_SAMPLE_RATE.to_string()));
    params.push(("channels", "1".to_string()));

    if cfg.interim_results {
        params.push(("interim_results", "true".to_string()));
        // Deepgram requires interim results for utterance-end detection, and
        // only accepts 1000..=5000.
        let utterance_end = cfg.utterance_end_ms.clamp(1_000, 5_000);
        params.push(("utterance_end_ms", utterance_end.to_string()));
        params.push(("vad_events", "true".to_string()));
    }
    // The default is 10 ms, which finalises mid-word; a dictation app wants a
    // deliberate pause.
    params.push(("endpointing", cfg.endpointing_ms.max(10).to_string()));

    assemble("wss://api.deepgram.com/v1/listen", params)
}

/// Pull the transcript out of a prerecorded response.
pub fn parse_recorded_transcript(body: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(body).context("Deepgram returned malformed JSON")?;
    let transcript = text_at(
        &value,
        &[
            "results",
            "channels",
            "0",
            "alternatives",
            "0",
            "transcript",
        ],
    )
    .unwrap_or_default();
    let transcript = transcript.trim().to_string();
    if transcript.is_empty() {
        bail!("Deepgram returned no transcript");
    }
    Ok(transcript)
}

/// Transcribe a finished utterance by uploading it to the prerecorded endpoint.
///
/// This is Deepgram's default mode in QuillScribe: the app delivers text when
/// recording stops anyway, so streaming buys nothing until live typing exists.
pub async fn transcribe_recorded(
    cfg: &DeepgramConfig,
    audio: Vec<f32>,
    sample_rate: u32,
) -> Result<String> {
    if cfg.api_key.trim().is_empty() {
        bail!("No Deepgram API key. Add one in Settings â†’ Engine.");
    }
    if audio.is_empty() {
        bail!("No audio was captured.");
    }

    let wav = WhisperManager::encode_wav(&audio, sample_rate)?;
    let url = build_recorded_url(cfg);
    log::info!("Deepgram recorded request: {}", url);

    let response = WhisperManager::http_client()
        .post(&url)
        .header("Authorization", format!("Token {}", cfg.api_key.trim()))
        .header("Content-Type", "audio/wav")
        .body(wav)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .context("Could not reach Deepgram")?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "Deepgram returned HTTP {}: {}",
            status.as_u16(),
            body.trim()
        );
    }

    parse_recorded_transcript(&body)
}

// â”€â”€ Session â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

enum SessionCommand {
    Stop,
}

type WsSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type WsSink = futures_util::stream::SplitSink<WsSocket, Message>;
type WsStream = futures_util::stream::SplitStream<WsSocket>;

/// Callback used to push progress onto the UI thread.
pub type ProgressSink = Arc<dyn Fn(Progress) + Send + Sync>;

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
struct SessionState {
    /// 16 kHz mono PCM for the whole utterance, kept so a dropped socket can be
    /// replayed from the beginning (Deepgram cannot resume a session).
    pcm: Vec<i16>,
    /// Committed text for the *current* connection. Cleared on reconnect, since
    /// the replay re-transcribes from the start.
    finals: Vec<String>,
    /// Finals the sink has already been given, for the whole utterance. It is
    /// not cleared on reconnect, because the replay re-reports the sentences an
    /// earlier connection already reported.
    emitted: Vec<String>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            pcm: Vec::new(),
            finals: Vec::new(),
            emitted: Vec::new(),
        }
    }

    fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            self.pcm.push((clamped * i16::MAX as f32) as i16);
        }
    }

    fn commit(&mut self, text: &str) {
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
    fn is_replay(&self, text: &str) -> bool {
        let position = self.finals.len().saturating_sub(1);
        self.emitted.get(position).map(String::as_str) == Some(text.trim())
    }

    fn remember_emitted(&mut self, text: &str) {
        self.emitted.push(text.trim().to_string());
    }

    fn transcript(&self) -> String {
        // Segments are already trimmed; join with a single space and collapse
        // any double spaces punctuation may have left behind.
        self.finals
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

async fn run_session(
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

/// Run one socket from connect to close.
///
/// `Ok` means the user stopped recording and the transcript is final. `Err`
/// means the connection failed and the caller should reconnect and replay.
#[allow(clippy::too_many_arguments)]
async fn run_connection(
    cfg: &DeepgramConfig,
    url: &str,
    state: &mut SessionState,
    audio_rx: &mut mpsc::UnboundedReceiver<AudioChunk>,
    command_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
    on_progress: &ProgressSink,
) -> Result<String> {
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
    let (mut sink, mut stream) = socket.split();

    // A reconnect begins a brand new session with timestamps reset to zero, so
    // the only way to keep the transcript whole is to send everything again.
    state.finals.clear();
    let mut sent = 0usize;
    flush_audio(&mut sink, &state.pcm, &mut sent).await?;

    let mut pending: Vec<f32> = Vec::new();
    let mut pending_rate = TARGET_SAMPLE_RATE;
    let mut last_send = Instant::now();
    let mut audio_open = true;
    let mut keepalive = interval(KEEPALIVE_PERIOD);
    keepalive.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                // `Stop`, or the sender being dropped, both mean "end now".
                if command.is_none() || matches!(command, Some(SessionCommand::Stop)) {
                    if !pending.is_empty() {
                        let tail = WhisperManager::resample(
                            &pending,
                            pending_rate,
                            TARGET_SAMPLE_RATE,
                        );
                        state.push_samples(&tail);
                        flush_audio(&mut sink, &state.pcm, &mut sent).await?;
                    }
                    return finish(&mut sink, &mut stream, state, on_progress).await;
                }
            }

            chunk = audio_rx.recv(), if audio_open => {
                match chunk {
                    Some(chunk) => {
                        if chunk.sample_rate == 0 {
                            continue;
                        }
                        pending_rate = chunk.sample_rate;
                        pending.extend_from_slice(&chunk.samples);
                        let block_samples =
                            ((pending_rate as usize) * BLOCK_MS as usize / 1000).max(1);
                        while pending.len() >= block_samples {
                            let rest = pending.split_off(block_samples);
                            let block = std::mem::replace(&mut pending, rest);
                            let resampled = WhisperManager::resample(
                                &block,
                                pending_rate,
                                TARGET_SAMPLE_RATE,
                            );
                            state.push_samples(&resampled);
                            flush_audio(&mut sink, &state.pcm, &mut sent).await?;
                            last_send = Instant::now();
                        }
                    }
                    // The tap went away; keep draining the socket text instead
                    // of spinning on a closed channel.
                    None => audio_open = false,
                }
            }

            message = stream.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        if let ServerMessage::Error(err) = parse_server_message(&text) {
                            bail!("Deepgram: {}", err);
                        }
                        apply_message(&text, state, on_progress);
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        return Err(anyhow!("Deepgram closed the connection"));
                    }
                    Some(Err(err)) => {
                        return Err(anyhow!("Deepgram socket error: {}", err));
                    }
                    _ => {}
                }
            }

            _ = keepalive.tick() => {
                if last_send.elapsed() >= KEEPALIVE_MIN_IDLE {
                    sink.send(Message::text("{\"type\":\"KeepAlive\"}"))
                        .await
                        .context("Could not send a Deepgram keep-alive")?;
                    last_send = Instant::now();
                }
            }
        }
    }
}

/// Flush the tail, close the stream, and collect the final transcript.
///
/// Order matters: `Finalize` recovers the audio still in Deepgram's buffer,
/// then `CloseStream` reports and terminates.
async fn finish(
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
fn apply_message(raw: &str, state: &mut SessionState, on_progress: &ProgressSink) {
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

async fn flush_audio(sink: &mut WsSink, pcm: &[i16], sent: &mut usize) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> DeepgramConfig {
        DeepgramConfig {
            api_key: "not-a-real-deepgram-key".to_string(),
            ..DeepgramConfig::default()
        }
    }

    // â”€â”€ URL building â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    #[test]
    fn test_build_listen_url_enables_the_normal_things() {
        let url = build_listen_url(&cfg());

        assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
        assert!(url.contains("model=nova-3"));
        assert!(url.contains("language=en"));
        assert!(url.contains("encoding=linear16"));
        assert!(url.contains("sample_rate=16000"));
        assert!(url.contains("channels=1"));
        assert!(url.contains("smart_format=true"));
        assert!(url.contains("punctuate=true"));
        // `dictation` is deliberately absent by default: it replaces automatic
        // punctuation, it does not add to it.
        assert!(!url.contains("dictation"), "URL was: {}", url);
        assert!(url.contains("interim_results=true"));
        assert!(url.contains("endpointing=300"));
        assert!(url.contains("utterance_end_ms=1000"));
    }

    #[test]
    fn test_build_recorded_url_is_a_batch_request() {
        let url = build_recorded_url(&cfg());

        assert!(url.starts_with("https://api.deepgram.com/v1/listen?"));
        assert!(url.contains("model=nova-3"));
        assert!(url.contains("language=en"));
        assert!(url.contains("smart_format=true"));
        assert!(url.contains("punctuate=true"));
        // The audio is a complete WAV, so raw-PCM hints must be absentâ€¦
        assert!(!url.contains("encoding"), "URL was: {}", url);
        assert!(!url.contains("sample_rate"));
        assert!(!url.contains("channels"));
        // â€¦and so must live-only parameters.
        assert!(!url.contains("interim_results"));
        assert!(!url.contains("utterance_end_ms"));
        assert!(!url.contains("endpointing"));
        assert!(!url.contains("vad_events"));
    }

    #[test]
    fn test_parse_recorded_transcript_extracts_the_text() {
        let body = r#"{"metadata":{"request_id":"x"},
            "results":{"channels":[{"alternatives":[{"transcript":" The quick brown fox. ",
            "confidence":0.99,"words":[]}]}]}}"#;
        assert_eq!(
            parse_recorded_transcript(body).unwrap(),
            "The quick brown fox."
        );
    }

    #[test]
    fn test_parse_recorded_transcript_rejects_an_empty_result() {
        let body = r#"{"results":{"channels":[{"alternatives":[{"transcript":""}]}]}}"#;
        assert!(parse_recorded_transcript(body).is_err());
        assert!(parse_recorded_transcript("not json").is_err());
    }

    #[test]
    fn test_build_listen_url_never_sends_diarization() {
        // Diarization was deliberately left out of this provider.
        let url = build_listen_url(&cfg());
        assert!(!url.contains("diarize"), "URL was: {}", url);
    }

    #[test]
    fn test_build_listen_url_forces_punctuate_when_dictation_is_on() {
        let mut config = cfg();
        config.punctuate = false;
        config.dictation = true;

        let url = build_listen_url(&config);
        assert!(url.contains("punctuate=true"), "URL was: {}", url);
        assert!(url.contains("dictation=true"));
    }

    #[test]
    fn test_build_listen_url_sends_dictation_only_when_enabled() {
        let mut config = cfg();
        config.dictation = true;
        assert!(build_listen_url(&config).contains("dictation=true"));

        config.dictation = false;
        assert!(!build_listen_url(&config).contains("dictation"));
    }

    #[test]
    fn test_build_listen_url_omits_disabled_flags() {
        let mut config = cfg();
        config.smart_format = false;
        config.punctuate = false;
        config.dictation = false;
        config.interim_results = false;

        let url = build_listen_url(&config);
        assert!(!url.contains("smart_format"));
        assert!(!url.contains("punctuate"));
        assert!(!url.contains("dictation"));
        assert!(!url.contains("interim_results"));
        // utterance end only means something with interim results
        assert!(!url.contains("utterance_end_ms"));
        assert!(!url.contains("vad_events"));
    }

    #[test]
    fn test_build_listen_url_clamps_utterance_end_to_the_supported_range() {
        let mut config = cfg();
        config.utterance_end_ms = 200; // below Deepgram's 1000 ms minimum

        let url = build_listen_url(&config);
        assert!(url.contains("utterance_end_ms=1000"), "URL was: {}", url);

        config.utterance_end_ms = 90_000; // above the 5000 ms maximum
        let url = build_listen_url(&config);
        assert!(url.contains("utterance_end_ms=5000"), "URL was: {}", url);
    }

    #[test]
    fn test_build_listen_url_encodes_model_and_language() {
        let mut config = cfg();
        config.model = "nova-3 custom".to_string();
        config.language = "en-US".to_string();

        let url = build_listen_url(&config);
        assert!(url.contains("model=nova-3%20custom"), "URL was: {}", url);
        assert!(url.contains("language=en-US"));
    }

    // â”€â”€ Message parsing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    #[test]
    fn test_parse_interim_result() {
        // Note: no "type" field, exactly as Deepgram sends it.
        let raw = r#"{"channel_index":[0,1],"duration":1.039875,"start":0,"is_final":false,
            "channel":{"alternatives":[{"transcript":"another big","confidence":0.96,
            "words":[]}]}}"#;
        assert_eq!(
            parse_server_message(raw),
            ServerMessage::Interim("another big".to_string())
        );
    }

    #[test]
    fn test_parse_final_result() {
        let raw = r#"{"type":"Results","is_final":true,"speech_final":true,
            "channel":{"alternatives":[{"transcript":" hello there. ","confidence":1.0}]}}"#;
        assert_eq!(
            parse_server_message(raw),
            ServerMessage::Final {
                text: "hello there.".to_string(),
                from_finalize: false,
            }
        );
    }

    #[test]
    fn test_parse_final_from_finalize() {
        let raw = r#"{"type":"Results","is_final":true,"from_finalize":true,
            "channel":{"alternatives":[{"transcript":" tail words"}]}}"#;
        match parse_server_message(raw) {
            ServerMessage::Final {
                text,
                from_finalize,
            } => {
                assert_eq!(text, "tail words");
                assert!(from_finalize, "from_finalize must be preserved");
            }
            other => panic!("expected a final, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_speech_final_without_is_final_is_not_a_commit() {
        // Deepgram's own docs show is_final:false together with speech_final:true.
        let raw = r#"{"type":"Results","is_final":false,"speech_final":true,
            "channel":{"alternatives":[{"transcript":"half a thought"}]}}"#;
        assert_eq!(
            parse_server_message(raw),
            ServerMessage::Interim("half a thought".to_string())
        );
    }

    #[test]
    fn test_parse_utterance_end_is_a_turn_boundary() {
        let raw = r#"{"type":"UtteranceEnd","channel":[0,1],"last_word_end":2.395}"#;
        assert_eq!(parse_server_message(raw), ServerMessage::TurnBoundary);
    }

    fn final_frame(text: &str) -> String {
        format!(
            r#"{{"type":"Results","is_final":true,
                "channel":{{"alternatives":[{{"transcript":"{}"}}]}}}}"#,
            text
        )
    }

    /// Collect the finals a sink is given, in order.
    fn collecting_sink() -> (ProgressSink, Arc<std::sync::Mutex<Vec<String>>>) {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink: ProgressSink = {
            let seen = Arc::clone(&seen);
            Arc::new(move |event: Progress| {
                if let Progress::Final(text) = event {
                    seen.lock().unwrap().push(text);
                }
            })
        };
        (sink, seen)
    }

    #[test]
    fn test_a_replayed_final_is_not_reported_twice() {
        let (sink, seen) = collecting_sink();
        let mut state = SessionState::new();

        apply_message(&final_frame("one"), &mut state, &sink);
        apply_message(&final_frame("two"), &mut state, &sink);

        // The socket dropped: the next connection re-transcribes the whole
        // utterance from the beginning and reports the same sentences again.
        state.finals.clear();
        apply_message(&final_frame("one"), &mut state, &sink);
        apply_message(&final_frame("two"), &mut state, &sink);

        assert_eq!(*seen.lock().unwrap(), ["one", "two"]);
        assert_eq!(state.transcript(), "one two");
    }

    #[test]
    fn test_a_final_from_new_audio_after_a_replay_is_still_reported() {
        let (sink, seen) = collecting_sink();
        let mut state = SessionState::new();

        apply_message(&final_frame("one"), &mut state, &sink);
        apply_message(&final_frame("two"), &mut state, &sink);

        state.finals.clear();
        apply_message(&final_frame("one"), &mut state, &sink);
        // The audio spoken after the reconnect commits its own sentence.
        apply_message(&final_frame("three"), &mut state, &sink);

        assert_eq!(*seen.lock().unwrap(), ["one", "two", "three"]);
    }

    #[test]
    fn test_a_blank_final_does_not_desync_the_replay_guard() {
        let (sink, seen) = collecting_sink();
        let mut state = SessionState::new();

        // Deepgram emits whitespace-only finals on silence.
        apply_message(&final_frame("one"), &mut state, &sink);
        apply_message(&final_frame("   "), &mut state, &sink);
        apply_message(&final_frame("two"), &mut state, &sink);

        state.finals.clear();
        apply_message(&final_frame("one"), &mut state, &sink);
        apply_message(&final_frame("   "), &mut state, &sink);
        apply_message(&final_frame("two"), &mut state, &sink);

        assert_eq!(*seen.lock().unwrap(), ["one", "two"]);
        assert_eq!(state.transcript(), "one two");
    }

    #[test]
    fn test_parse_metadata() {
        let raw = r#"{"type":"Metadata","request_id":"abc","duration":0.0,"channels":0}"#;
        assert_eq!(parse_server_message(raw), ServerMessage::Metadata);
    }

    #[test]
    fn test_parse_error_carries_a_message() {
        let raw = r#"{"type":"Error","description":"NET-0001"}"#;
        assert_eq!(
            parse_server_message(raw),
            ServerMessage::Error("NET-0001".to_string())
        );
    }

    #[test]
    fn test_parse_garbage_is_ignored() {
        assert_eq!(parse_server_message("not json"), ServerMessage::Ignored);
        assert_eq!(
            parse_server_message("{}"),
            ServerMessage::Interim(String::new())
        );
    }

    // â”€â”€ Transcript assembly â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    #[test]
    fn test_session_state_joins_finals_once() {
        let mut state = SessionState::new();
        state.commit(" hello there. ");
        state.commit("");
        state.commit("how are you?");
        assert_eq!(state.transcript(), "hello there. how are you?");
    }

    #[test]
    fn test_session_state_clears_finals_on_reconnect() {
        let mut state = SessionState::new();
        state.commit("first attempt");
        state.finals.clear(); // what run_connection does on reconnect
        state.commit("replayed properly");
        assert_eq!(state.transcript(), "replayed properly");
    }

    #[test]
    fn test_session_state_accumulates_pcm_for_replay() {
        let mut state = SessionState::new();
        let chunk = vec![0.0f32; 100];
        state.push_samples(&chunk);
        state.push_samples(&chunk);
        assert_eq!(state.pcm.len(), 200);
    }

    // â”€â”€ Live network test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    /// The key the app itself would use: `settings.json` first, because that is
    /// where Settings â†’ Engine writes it, then an environment override.
    fn live_api_key() -> Option<String> {
        let from_env = std::env::var("DEEPGRAM_API_KEY")
            .ok()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty());
        if from_env.is_some() {
            return from_env;
        }

        let path = crate::config::ConfigManager::config_dir()
            .ok()?
            .join("settings.json");
        let raw = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
        json.get("deepgram")?
            .get("api_key")?
            .as_str()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
    }

    /// End-to-end check of the recorded (prerecorded) path.
    ///
    /// ```text
    /// cargo test --lib live_deepgram_recorded -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn live_deepgram_recorded_transcribes_synthesized_speech() {
        let _ = env_logger::builder().is_test(true).try_init();

        let Some(api_key) = live_api_key() else {
            eprintln!("skipping: no Deepgram key (Settings → Engine or DEEPGRAM_API_KEY)");
            return;
        };
        let fixture = std::path::Path::new("spike_speech.wav");
        if !fixture.exists() {
            eprintln!("skipping: spike_speech.wav is missing");
            return;
        }

        let reader = hound::WavReader::open(fixture).expect("open fixture");
        let spec = reader.spec();
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / i16::MAX as f32)
            .collect();

        let config = DeepgramConfig {
            api_key,
            ..DeepgramConfig::default()
        };
        assert!(!config.is_live(), "recorded must be the default mode");

        let transcript = transcribe_recorded(&config, samples, spec.sample_rate)
            .await
            .expect("recorded transcription");
        println!("recorded transcript: {}", transcript);
        assert!(
            transcript.trim_end().ends_with('.'),
            "expected punctuation from the recorded endpoint, got {:?}",
            transcript
        );
    }
    /// End-to-end check against the real service.
    ///
    /// The key is taken from the app's own settings first, so a key pasted into
    /// Settings â†’ Engine â†’ Deepgram is enough; `DEEPGRAM_API_KEY` is only an
    /// override for CI. Also needs the gitignored `spike_speech.wav` fixture.
    ///
    /// ```text
    /// cargo test --lib live_deepgram -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn live_deepgram_transcribes_synthesized_speech() {
        // `RUST_LOG=debug` dumps every raw frame, which is how you tell
        // "Deepgram didn't punctuate" apart from "we dropped the punctuation".
        let _ = env_logger::builder().is_test(true).try_init();

        let Some(api_key) = live_api_key() else {
            eprintln!(
                "skipping: no Deepgram key. Paste one into Settings â†’ Engine â†’ Deepgram \
                 (written to settings.json) or set DEEPGRAM_API_KEY."
            );
            return;
        };
        let fixture = std::path::Path::new("spike_speech.wav");
        if !fixture.exists() {
            eprintln!("skipping: spike_speech.wav is missing");
            return;
        }

        let reader = hound::WavReader::open(fixture).expect("open fixture");
        let spec = reader.spec();
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / i16::MAX as f32)
            .collect();
        if samples.is_empty() {
            eprintln!("skipping: fixture contains no samples");
            return;
        }

        let config = DeepgramConfig {
            api_key,
            ..DeepgramConfig::default()
        };
        let (audio_tx, audio_rx) = mpsc::unbounded_channel();
        let progress: ProgressSink = Arc::new(|event: Progress| {
            if let Progress::Interim(text) = event {
                println!("interim: {}", text);
            }
        });

        let session = LiveSession::start(
            tokio::runtime::Handle::current(),
            config,
            audio_rx,
            progress,
        );

        // Feed the clip at roughly realtime: Deepgram accepts at most 1.25x.
        let block = (spec.sample_rate as usize) / 10;
        for chunk in samples.chunks(block.max(1)) {
            audio_tx
                .send(AudioChunk {
                    samples: chunk.to_vec(),
                    sample_rate: spec.sample_rate,
                })
                .expect("session alive");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        drop(audio_tx);

        let transcript = session.finish().await.expect("live transcription");
        println!("transcript: {}", transcript);
        assert!(
            transcript.split_whitespace().count() >= 2,
            "expected a real transcript, got {:?}",
            transcript
        );
        // Regression guard for the measured `dictation` behaviour: turning spoken
        // punctuation on silently removes automatic punctuation, so if this ever
        // comes back unpunctuated the defaults have drifted.
        assert!(
            transcript.trim_end().ends_with('.'),
            "expected Deepgram to punctuate the sentence, got {:?}",
            transcript
        );
    }
}
