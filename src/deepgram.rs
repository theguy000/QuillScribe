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

    // "Results", or a message with no `type` at all (interims).
    match value.get("type").and_then(Value::as_str) {
        Some("Metadata") => ServerMessage::Metadata,
        Some("UtteranceEnd") => ServerMessage::TurnBoundary,
        Some("SpeechStarted") => ServerMessage::SpeechStarted,
        Some("ConfigureSuccess") | Some("ConfigureFailure") => ServerMessage::Configured,
        Some("Error") => ServerMessage::Error(error_text(&value, raw)),
        _ => parse_results(&value),
    }
}

/// The reason an `Error` frame gives, or the frame itself when it gives none.
fn error_text(value: &Value, raw: &str) -> String {
    text_at(value, &["description"])
        .or_else(|| text_at(value, &["message"]))
        .or_else(|| text_at(value, &["err_msg"]))
        .unwrap_or_else(|| raw.trim().to_string())
}

/// A transcript frame, which carries no `type` field, as a Final or an Interim.
fn parse_results(value: &Value) -> ServerMessage {
    let text = text_at(value, &["channel", "alternatives", "0", "transcript"])
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);

    if flag("is_final") {
        ServerMessage::Final {
            text,
            from_finalize: flag("from_finalize"),
        }
    } else {
        ServerMessage::Interim(text)
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
/// One frame as the socket reads it, error included.
type WsMessage = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>;

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

/// Where one socket stands between the audio it hears and the frames it reads.
struct Connection<'a> {
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
async fn connect(cfg: &DeepgramConfig, url: &str) -> Result<(WsSink, WsStream)> {
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
async fn run_connection(
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
async fn send_audio(
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
fn apply_frame(raw: &str, state: &mut SessionState, on_progress: &ProgressSink) -> Result<()> {
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
#[path = "deepgram/tests.rs"]
mod tests;
