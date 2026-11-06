//! One Gemini dictation, as the WebChannel protocol wants it: handshake, then
//! recognizer init, then the audio, then the transcript.
//!
//! The protocol is a long-poll with no close on its own, and the server refuses
//! audio until a downstream listener is attached, so the ordering here is part
//! of the protocol rather than a style choice. Each numbered step below is one
//! method, in the order the server expects them.

use super::super::WhisperManager;
use anyhow::{anyhow, Result};

impl WhisperManager {
    pub(crate) async fn transcribe_gemini(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        let webm = Self::encode_webm_opus(&audio_data, sample_rate)?;
        let session = self.open_session().await?;
        let (stream_task, done_rx) = Self::listen(&session).await?;
        Self::send_init(&session, &self.api_language).await?;
        Self::upload(&session, &webm).await?;
        let body = Self::await_body(done_rx).await?;
        let _ = stream_task.await;
        Self::transcript_from(&body)
    }

    /// The key to authenticate with: the user's, or Google's public browser key
    /// when they have not supplied one.
    fn gemini_active_key(&self) -> String {
        let own = self.gemini_api_key.trim();
        if own.is_empty() {
            Self::gemini_website_key()
        } else {
            own.to_string()
        }
    }

    /// Step 1: the opening request, which yields the session id and the SID
    /// every later request carries.
    pub(super) async fn open_session(&self) -> Result<Session> {
        let hdrs = Self::gemini_headers(&self.gemini_active_key());
        let zx = format!("{:012x}", rand::random::<u64>());
        let url = format!(
            "{}?VER=8&RID=10000&CVER=22&X-HTTP-Session-Id=gsessionid&zx={}&t=1",
            Self::GEMINI_S3_BASE,
            zx
        );
        let response = decorate(Self::http_client().post(url), &hdrs)
            .body("count=0")
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Gemini handshake failed: HTTP {}",
                response.status()
            ));
        }
        let gsid = response
            .headers()
            .get("x-http-session-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Gemini handshake missing x-http-session-id"))?
            .to_string();
        let sid = Self::parse_webchannel_sid(&response.text().await.unwrap_or_default())?;
        Ok(Session {
            gsid,
            sid,
            zx,
            hdrs,
        })
    }

    /// Step 2: attach the downstream stream **before** uploading, and return
    /// the reader task with the channel its result arrives on.
    ///
    /// The server rejects uploads with HTTP 400 while no listener is attached,
    /// so this waits for the first bytes rather than racing the two requests.
    /// WebChannel is a long-poll and the socket stays open after the final
    /// message, so completion is signalled from inside the reader.
    async fn listen(
        session: &Session,
    ) -> Result<(
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<Vec<u8>>,
    )> {
        let url = format!(
            "{}?gsessionid={}&VER=8&RID=rpc&SID={}&AID=0&CI=0&TYPE=xmlhttp&zx={}&t=1",
            Self::GEMINI_S3_BASE,
            session.gsid,
            session.sid,
            session.zx
        );
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
        let request = decorate(Self::http_client().get(url), &session.hdrs);
        let stream_task = tokio::spawn(read_stream(request, ready_tx, done_tx));
        ready_rx
            .await
            .map_err(|_| anyhow!("Gemini stream closed before it was established"))?;
        Ok((stream_task, done_rx))
    }

    /// Step 3: the recognizer init (encoding 11 = Opus).
    pub(super) async fn send_init(session: &Session, language: &str) -> Result<()> {
        let init = Self::base64_encode(&Self::build_recognizer_init_payload(language));
        let response = post_data(session, 10001, 0, 0, &init).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Gemini recognizer init failed: HTTP {}",
                response.status()
            ));
        }
        Ok(())
    }

    /// Steps 4 and 5: the audio in chunks, then the end marker.
    ///
    /// Each POST costs one round-trip (~400 ms to this endpoint), and the live
    /// client sends up to ~27 KB per message, so few large chunks beat many
    /// small ones.
    async fn upload(session: &Session, webm: &[u8]) -> Result<()> {
        let mut rid = 10002u64;
        let mut ofs = 1u64;
        for block in webm.chunks(24_000) {
            let b64 = Self::base64_encode(&Self::build_audio_chunk(block));
            let response = post_data(session, rid, ofs, 1, &b64).send().await?;
            if !response.status().is_success() {
                return Err(anyhow!(
                    "Gemini audio upload failed: HTTP {}",
                    response.status()
                ));
            }
            rid += 1;
            ofs += 1;
        }
        // The end marker is base64 of a proto field 3 = 1. Its answer is not
        // needed; the transcript arrives on the listener instead.
        let _ = post_data(session, rid, ofs, 1, "GAE=").send().await;
        Ok(())
    }

    /// Step 6: wait for the terminal message, which the reader task sends
    /// because the long-poll socket does not close on its own.
    async fn await_body(done_rx: tokio::sync::oneshot::Receiver<Vec<u8>>) -> Result<Vec<u8>> {
        tokio::time::timeout(std::time::Duration::from_secs(30), done_rx)
            .await
            .map_err(|_| anyhow!("Gemini stream timed out"))?
            .map_err(|_| anyhow!("Gemini stream ended unexpectedly"))
    }

    /// The newest transcript in the collected response lines.
    pub(super) fn transcript_from(body: &[u8]) -> Result<String> {
        let text = String::from_utf8_lossy(body);
        let transcript = text
            .lines()
            .filter_map(Self::parse_webchannel_transcript)
            .next_back()
            .unwrap_or_default();
        Ok(transcript.trim().to_string())
    }
}

/// The ids a Gemini session threads through every request, with the headers it
/// was opened with.
pub(super) struct Session {
    pub(super) gsid: String,
    pub(super) sid: String,
    pub(super) zx: String,
    pub(super) hdrs: Vec<(&'static str, String)>,
}

/// Apply the Gemini request headers to a request.
pub(super) fn decorate(
    mut rb: reqwest::RequestBuilder,
    hdrs: &[(&'static str, String)],
) -> reqwest::RequestBuilder {
    for (k, v) in hdrs {
        rb = rb.header(*k, v.clone());
    }
    rb
}

/// One `req0___data__` POST, addressed to the session.
pub(super) fn post_data(
    session: &Session,
    rid: u64,
    ofs: u64,
    aid: u64,
    data: &str,
) -> reqwest::RequestBuilder {
    let body = format!(
        "count=1&ofs={}&req0___data__={}",
        ofs,
        WhisperManager::url_encode(data)
    );
    decorate(
        WhisperManager::http_client().post(WhisperManager::GEMINI_S3_BASE),
        &session.hdrs,
    )
    .query(&[
        ("VER", "8".to_string()),
        ("gsessionid", session.gsid.clone()),
        ("SID", session.sid.clone()),
        ("RID", rid.to_string()),
        ("AID", aid.to_string()),
        ("zx", session.zx.clone()),
        ("t", "1".to_string()),
    ])
    .body(body)
}

/// Read the long-poll body until the server says the transcript is complete.
///
/// The first chunk signals readiness, because uploads are refused until a
/// listener exists. After that the terminal status frames end the read: without
/// them the socket would be held open forever.
async fn read_stream(
    request: reqwest::RequestBuilder,
    ready: tokio::sync::oneshot::Sender<()>,
    done: tokio::sync::oneshot::Sender<Vec<u8>>,
) {
    let Ok(mut response) = request.send().await else {
        return;
    };
    let mut ready = Some(ready);
    let mut done = Some(done);
    let mut body: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                body.extend_from_slice(&chunk);
                if let Some(tx) = ready.take() {
                    let _ = tx.send(());
                }
                let text = String::from_utf8_lossy(&body);
                // Terminal status messages: CAEoAw = transcript produced,
                // CAEoAg = accepted but nothing recognized.
                if text.contains("\"CAEoAw") || text.contains("\"CAEoAg") {
                    if let Some(tx) = done.take() {
                        let _ = tx.send(body);
                    }
                    return;
                }
            }
            // The server closed the stream: whatever we have is the answer.
            Ok(None) | Err(_) => {
                if let Some(tx) = done.take() {
                    let _ = tx.send(body);
                }
                return;
            }
        }
    }
}
