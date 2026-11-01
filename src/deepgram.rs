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

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

use crate::config::DeepgramConfig;
use crate::whisper::WhisperManager;

/// Deepgram accepts 16 kHz mono `linear16` for speech, and every model is
/// trained around it.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Sending nothing for ~10 s gets the socket closed with `NET-0001`, so stay
/// comfortably inside that window while the user pauses.
pub(super) const KEEPALIVE_PERIOD: Duration = Duration::from_secs(4);
pub(super) const KEEPALIVE_MIN_IDLE: Duration = Duration::from_secs(2);

/// `Finalize` answers almost immediately; `CloseStream` sends the final
/// `Metadata` before terminating.
pub(super) const FINALIZE_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);
pub(super) const FINISH_TIMEOUT: Duration = Duration::from_secs(10);

/// Deepgram has no session resumption: a reconnect starts a fresh session with
/// timestamps reset, so we rebuild the transcript by replaying our own audio.
pub(super) const MAX_RECONNECT_ATTEMPTS: u32 = 2;

/// How much audio is worth replaying after a dropped socket. Deepgram cannot
/// resume a session, so recovery means re-uploading everything â€” which is only
/// sane for something dictation-shaped. Beyond this we stop pretending and
/// report the failure instead of silently truncating someone's words.
pub(super) const MAX_REPLAY_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 60 * 5;

pub use stream::LiveSession;

/// Callback used to push progress onto the UI thread.
pub type ProgressSink = Arc<dyn Fn(Progress) + Send + Sync>;

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
pub(super) enum ServerMessage {
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

mod stream;

#[cfg(test)]
#[path = "deepgram/tests.rs"]
mod tests;
