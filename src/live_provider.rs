//! The live execution shape, shared by the providers that have one.
//!
//! A live session sends audio while the user is still speaking and reports text
//! back during the utterance; a batch provider only starts once recording
//! stops. Gemini and Deepgram both run as live sessions, so the vocabulary they
//! report progress in, and the value the app holds while an utterance is in
//! flight, belong here rather than inside either provider's module.
//!
//! See `docs/adr/0004-gemini-live-session.md`.

use anyhow::Result;

/// What a live session tells the rest of the app while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Text that may still change; replaces the in-progress line.
    Interim(String),
    /// Text the provider committed to.
    Final(String),
    /// Human-readable status for the status line.
    Status(String),
}

/// Where a session sends its [`Progress`].
pub type ProgressSink = std::sync::Arc<dyn Fn(Progress) + Send + Sync>;

/// The live session for the utterance in flight.
///
/// One enum so the stop path flushes whichever provider was recording without
/// knowing which one it was.
pub enum LiveSession {
    Deepgram(crate::deepgram::LiveSession),
    Gemini(crate::whisper::gemini::live::LiveSession),
}

impl LiveSession {
    /// Flush the tail and wait for the transcript.
    pub async fn finish(self) -> Result<String> {
        match self {
            Self::Deepgram(session) => session.finish().await,
            Self::Gemini(session) => session.finish().await,
        }
    }
}
