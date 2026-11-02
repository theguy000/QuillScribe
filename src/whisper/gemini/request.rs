//! One Gemini dictation, as the WebChannel protocol wants it: handshake, then
//! recognizer init, then the audio, then the transcript.
//!
//! The protocol is a long-poll with no close on its own, and the server refuses
//! audio until a downstream listener is attached, so the ordering here is part
//! of the protocol rather than a style choice.

use super::super::WhisperManager;
use anyhow::{anyhow, Result};

impl WhisperManager {
    pub(crate) async fn transcribe_gemini(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        let website_key;
        let key = if self.gemini_api_key.trim().is_empty() {
            website_key = Self::gemini_website_key();
            &website_key
        } else {
            self.gemini_api_key.trim()
        };

        let webm = Self::encode_webm_opus(&audio_data, sample_rate)?;
        // Reuse one pooled client so the TLS connection stays warm between
        // dictations instead of renegotiating on every call.
        let client = Self::http_client().clone();
        let hdrs = Self::gemini_headers(key);
        let zx = format!("{:012x}", rand::random::<u64>());

        let decorate = |rb: reqwest::RequestBuilder, hdrs: &[(&'static str, String)]| {
            let mut rb = rb;
            for (k, v) in hdrs {
                rb = rb.header(*k, v.clone());
            }
            rb
        };

        // 1. Handshake -> gsessionid + SID
        let hs = decorate(
            client.post(format!(
                "{}?VER=8&RID=10000&CVER=22&X-HTTP-Session-Id=gsessionid&zx={}&t=1",
                Self::GEMINI_S3_BASE,
                zx
            )),
            &hdrs,
        )
        .body("count=0")
        .send()
        .await?;

        if !hs.status().is_success() {
            return Err(anyhow!("Gemini handshake failed: HTTP {}", hs.status()));
        }
        let gsid = hs
            .headers()
            .get("x-http-session-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Gemini handshake missing x-http-session-id"))?
            .to_string();
        let sid = Self::parse_webchannel_sid(&hs.text().await.unwrap_or_default())?;

        // 2. Attach the downstream stream BEFORE uploading. The server rejects
        //    uploads with HTTP 400 when no listener is attached yet, so we wait
        //    for the first bytes rather than racing the two requests.
        //
        //    WebChannel is a long-poll: the socket stays open after the final
        //    message, so we must signal completion from inside the reader
        //    instead of waiting for the connection to close.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
        let stream_url = format!(
            "{}?gsessionid={}&VER=8&RID=rpc&SID={}&AID=0&CI=0&TYPE=xmlhttp&zx={}&t=1",
            Self::GEMINI_S3_BASE,
            gsid,
            sid,
            zx
        );
        let (client_c, hdrs_c) = (client.clone(), hdrs.clone());
        let stream_task = tokio::spawn(async move {
            let mut resp = decorate(client_c.get(stream_url), &hdrs_c).send().await?;
            let mut body: Vec<u8> = Vec::new();
            let mut ready = Some(ready_tx);
            let mut done = Some(done_tx);
            loop {
                match resp.chunk().await {
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
                            break;
                        }
                    }
                    // Server closed the stream.
                    Ok(None) => {
                        if let Some(tx) = done.take() {
                            let _ = tx.send(body);
                        }
                        break;
                    }
                    // Read error: still hand back what we collected.
                    Err(_) => {
                        if let Some(tx) = done.take() {
                            let _ = tx.send(body);
                        }
                        return Ok(());
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        });

        // Proceed only once the stream is established.
        ready_rx
            .await
            .map_err(|_| anyhow!("Gemini stream closed before it was established"))?;

        let post_data = |rb: reqwest::RequestBuilder, rid: u64, ofs: u64, aid: u64, data: &str| {
            let body = format!(
                "count=1&ofs={}&req0___data__={}",
                ofs,
                Self::url_encode(data)
            );
            decorate(rb, &hdrs)
                .query(&[
                    ("VER", "8"),
                    ("gsessionid", gsid.as_str()),
                    ("SID", sid.as_str()),
                    ("RID", &rid.to_string()),
                    ("AID", &aid.to_string()),
                    ("zx", zx.as_str()),
                    ("t", "1"),
                ])
                .body(body)
        };

        // 3. Recognizer init (encoding 11 = Opus)
        let init = Self::base64_encode(&Self::build_recognizer_init_payload(&self.api_language));
        let r = post_data(
            client.post(Self::GEMINI_S3_BASE.to_string()),
            10001,
            0,
            0,
            &init,
        )
        .send()
        .await?;
        if !r.status().is_success() {
            return Err(anyhow!(
                "Gemini recognizer init failed: HTTP {}",
                r.status()
            ));
        }

        // 4. Stream the WebM-Opus payload
        let mut rid = 10002u64;
        let mut ofs = 1u64;
        // Each POST costs one round-trip (~400 ms to this endpoint), and the
        // live client sends up to ~27 KB per message, so prefer few large
        // chunks over many small ones.
        for block in webm.chunks(24_000) {
            let b64 = Self::base64_encode(&Self::build_audio_chunk(block));
            let r = post_data(
                client.post(Self::GEMINI_S3_BASE.to_string()),
                rid,
                ofs,
                1,
                &b64,
            )
            .send()
            .await?;
            if !r.status().is_success() {
                return Err(anyhow!("Gemini audio upload failed: HTTP {}", r.status()));
            }
            rid += 1;
            ofs += 1;
        }

        // 5. End of stream
        let _ = post_data(
            client.post(Self::GEMINI_S3_BASE.to_string()),
            rid,
            ofs,
            1,
            "GAE=",
        )
        .send()
        .await;

        // 6. Collect the transcript. The terminal message is delivered via the
        //    done channel because the long-poll socket does not close on its own.
        let body = tokio::time::timeout(std::time::Duration::from_secs(30), done_rx)
            .await
            .map_err(|_| anyhow!("Gemini stream timed out"))?
            .map_err(|_| anyhow!("Gemini stream ended unexpectedly"))?;
        let _ = stream_task.await;

        let text = String::from_utf8_lossy(&body);
        let transcript = text
            .lines()
            .filter_map(Self::parse_webchannel_transcript)
            .next_back()
            .unwrap_or_default();

        Ok(transcript.trim().to_string())
    }
}
