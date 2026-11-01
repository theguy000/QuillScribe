//! Gemini's WebChannel transport: the hand-built protobuf and base64.
//!
//! Google's documented Gemini API does not do speech, so this talks to the
//! endpoint the Gemini web app uses and decodes what comes back. Everything
//! here is wire format, which is why it sits apart from the request code that
//! decides what to send.

use super::WhisperManager;
use anyhow::{anyhow, Result};

impl WhisperManager {
    //
    // Mirrors the Gemini web app's dictate button. Wire format captured from
    // the live client:
    //   body       = form-urlencoded `count=1&ofs=N&req0___data__=<b64 proto>`
    //   init       = proto naming "beyond-a2a-recognizer" (encoding 11 = Opus)
    //   each audio = ext_field(293101) { field1: <WebM bytes> }
    //   end        = proto field 3 = varint 1 (base64 "GAE=")
    // Audio must be Opus-in-WebM as `MediaRecorder` produces it. Ogg, raw PCM
    // and MULAW are all accepted (HTTP 200) but never transcribed.

    pub(super) const GEMINI_WEBSITE_KEY: &str = "AIzaSyD6n9asBjvx1yBHfhFhfw_kpS9Faq0BZHM";
    const GEMINI_S3_BASE: &str =
        "https://speechs3proto2-pa.googleapis.com/s3web/prod/streaming/channel";

    /// Shared HTTP client: one connection pool for all Gemini traffic.
    pub(crate) fn http_client() -> &'static reqwest::Client {
        static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
        CLIENT.get_or_init(reqwest::Client::new)
    }

    fn gemini_headers(key: &str) -> Vec<(&'static str, String)> {
        vec![
            ("Referer", "https://gemini.google.com/".to_string()),
            ("X-Goog-Api-Key", key.to_string()),
            (
                "X-WebChannel-Content-Type",
                "application/x-protobuf".to_string(),
            ),
            (
                "Content-Type",
                "application/x-www-form-urlencoded".to_string(),
            ),
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36".to_string()),
        ]
    }

    /// Write an EBML element: id + 8-byte size + payload.
    fn ebml_elem(out: &mut Vec<u8>, id: &[u8], payload: &[u8]) {
        out.extend_from_slice(id);
        // 8-byte VINT: marker bit in the first byte, value in the low 56 bits.
        out.push(0x01);
        out.extend_from_slice(&(payload.len() as u64).to_be_bytes()[1..]);
        out.extend_from_slice(payload);
    }

    /// Minimal big-endian integer payload (at least one byte).
    fn ebml_uint(v: u64) -> Vec<u8> {
        let b = v.to_be_bytes();
        let start = b.iter().position(|&x| x != 0).unwrap_or(7);
        b[start..].to_vec()
    }

    /// Encode mono f32 samples as Opus inside a **WebM** container.
    ///
    /// The Gemini recognizer only decodes Opus-in-WebM (the format
    /// `MediaRecorder` produces); Ogg is accepted but never transcribed.
    pub fn encode_webm_opus(audio: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        use magnum_opus::{Application, Channels, Encoder};

        if audio.is_empty() {
            return Err(anyhow!("No audio to encode."));
        }

        // Opus is native at 48 kHz; resample so libopus never re-samples.
        let pcm48 = Self::resample(audio, sample_rate, 48_000);
        let samples: Vec<i16> = pcm48
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        let mut enc = Encoder::new(48_000, Channels::Mono, Application::Voip)
            .map_err(|e| anyhow!("Failed to create Opus encoder: {}", e))?;
        enc.set_bitrate(magnum_opus::Bitrate::Bits(32_000))
            .map_err(|e| anyhow!("Failed to set Opus bitrate: {}", e))?;

        const FRAME: usize = 960; // 20 ms at 48 kHz
        const FRAMES_PER_CLUSTER: usize = 50; // 1 s per cluster

        // OpusHead goes in CodecPrivate.
        let mut head = Vec::with_capacity(19);
        head.extend_from_slice(b"OpusHead");
        head.push(1); // version
        head.push(1); // channels (mono)
        head.extend_from_slice(&312u16.to_le_bytes()); // pre-skip
        head.extend_from_slice(&48_000u32.to_le_bytes()); // input sample rate
        head.extend_from_slice(&0i16.to_le_bytes()); // output gain
        head.push(0); // channel mapping family

        let total_frames = samples.len().div_ceil(FRAME);
        let duration_ms = samples.len() as f64 * 1000.0 / 48_000.0;

        let mut audio_elem = Vec::new();
        Self::ebml_elem(&mut audio_elem, &[0xB5], &48_000f64.to_be_bytes());
        Self::ebml_elem(&mut audio_elem, &[0x9F], &[1]);

        let mut track = Vec::new();
        Self::ebml_elem(&mut track, &[0xD7], &[1]); // TrackNumber
        Self::ebml_elem(&mut track, &[0x73, 0xC5], &[1]); // TrackUID
        Self::ebml_elem(&mut track, &[0x83], &[2]); // TrackType: audio
        Self::ebml_elem(&mut track, &[0x86], b"A_OPUS"); // CodecID
        Self::ebml_elem(&mut track, &[0x63, 0xA2], &head); // CodecPrivate
        let default_dur = Self::ebml_uint(20_000_000); // 20 ms in ns
        Self::ebml_elem(&mut track, &[0x23, 0xE3, 0x83], &default_dur);
        Self::ebml_elem(&mut track, &[0xE1], &audio_elem); // Audio

        let mut tracks = Vec::new();
        Self::ebml_elem(&mut tracks, &[0xAE], &track);

        let mut info = Vec::new();
        Self::ebml_elem(&mut info, &[0x2A, 0xD7, 0xB1], &Self::ebml_uint(1_000_000));
        Self::ebml_elem(&mut info, &[0x4D, 0x80], b"QuillScribe"); // MuxingApp
        Self::ebml_elem(&mut info, &[0x57, 0x41], b"QuillScribe"); // WritingApp
        Self::ebml_elem(&mut info, &[0x44, 0x89], &duration_ms.to_be_bytes());

        let mut seg = Vec::new();
        Self::ebml_elem(&mut seg, &[0x15, 0x49, 0xA9, 0x66], &info);
        Self::ebml_elem(&mut seg, &[0x16, 0x54, 0xAE, 0x6B], &tracks);

        let mut blocks: Vec<u8> = Vec::new();
        let mut cluster_ms: u64 = 0;
        for idx in 0..total_frames {
            let start = idx * FRAME;
            let end = (start + FRAME).min(samples.len());
            let mut frame = samples[start..end].to_vec();
            frame.resize(FRAME, 0); // libopus needs a full frame

            let packet = enc
                .encode_vec(&frame, 4000)
                .map_err(|e| anyhow!("Opus encode failed: {}", e))?;

            let rel = ((idx % FRAMES_PER_CLUSTER) * 20) as i16;
            let mut block = Vec::with_capacity(packet.len() + 4);
            block.push(0x81); // track number 1 (VINT)
            block.extend_from_slice(&rel.to_be_bytes());
            block.push(0x80); // flags: keyframe
            block.extend_from_slice(&packet);
            Self::ebml_elem(&mut blocks, &[0xA3], &block);

            if (idx + 1) % FRAMES_PER_CLUSTER == 0 || idx + 1 == total_frames {
                let mut cl = Vec::new();
                Self::ebml_elem(&mut cl, &[0xE7], &Self::ebml_uint(cluster_ms));
                cl.extend_from_slice(&blocks);
                Self::ebml_elem(&mut seg, &[0x1F, 0x43, 0xB6, 0x75], &cl);
                blocks.clear();
                cluster_ms += FRAMES_PER_CLUSTER as u64 * 20;
            }
        }

        let mut header = Vec::new();
        Self::ebml_elem(&mut header, &[0x42, 0x86], &[1]); // EBMLVersion
        Self::ebml_elem(&mut header, &[0x42, 0xF7], &[1]); // EBMLReadVersion
        Self::ebml_elem(&mut header, &[0x42, 0xF2], &[4]); // MaxIDLength
        Self::ebml_elem(&mut header, &[0x42, 0xF3], &[8]); // MaxSizeLength
        Self::ebml_elem(&mut header, &[0x42, 0x82], b"webm"); // DocType
        Self::ebml_elem(&mut header, &[0x42, 0x87], &[4]); // DocTypeVersion
        Self::ebml_elem(&mut header, &[0x42, 0x85], &[2]); // DocTypeReadVersion

        let mut out = Vec::new();
        Self::ebml_elem(&mut out, &[0x1A, 0x45, 0xDF, 0xA3], &header);
        Self::ebml_elem(&mut out, &[0x18, 0x53, 0x80, 0x67], &seg);
        Ok(out)
    }

    /// Wrap an audio slice in the WebChannel chunk envelope.
    ///
    /// Captured from the live client: `ext_field(293101) { field1: bytes }`.
    pub fn build_audio_chunk(payload: &[u8]) -> Vec<u8> {
        let mut inner = Vec::with_capacity(payload.len() + 8);
        inner.push(0x0a); // field 1: audio bytes
        Self::write_varint(&mut inner, payload.len() as u64);
        inner.extend_from_slice(payload);

        let mut chunk = Vec::with_capacity(inner.len() + 8);
        chunk.extend_from_slice(&[0xea, 0x8e, 0x8f, 0x01]); // extension field tag
        Self::write_varint(&mut chunk, inner.len() as u64);
        chunk.extend_from_slice(&inner);
        chunk
    }

    fn write_varint(buf: &mut Vec<u8>, mut n: u64) {
        while n >= 0x80 {
            buf.push(((n & 0x7f) | 0x80) as u8);
            n >>= 7;
        }
        buf.push((n & 0x7f) as u8);
    }

    /// Pull the newest non-empty transcript out of a WebChannel response line.
    ///
    /// The payload is a length-delimited protobuf; transcript text lives in
    /// field 3 as `{ field1: varint, field3: { field3: string } }`. We walk the
    /// wire format properly rather than scanning for printable bytes, because a
    /// raw scan swallows the length prefix as leading punctuation.
    #[allow(dead_code)]
    pub fn parse_webchannel_transcript(msg: &str) -> Option<String> {
        use base64::Engine as _;
        let engine = base64::engine::general_purpose::STANDARD;
        let mut best: Option<String> = None;
        for cap in msg.split('"').filter(|s| s.len() >= 12) {
            let Ok(raw) = engine.decode(cap.as_bytes()) else {
                continue;
            };
            for text in Self::extract_proto_strings(&raw) {
                if text.len() >= 2
                    && !text.contains("recognizer")
                    && !text.contains("frontend")
                    && text != "Web"
                    && text != "en-US"
                {
                    best = Some(text);
                }
            }
        }
        best
    }

    /// Recursively collect length-delimited string fields from a protobuf blob.
    ///
    /// Note: the *tag* is itself a base-128 varint and can be multi-byte (the
    /// WebChannel envelope uses a 4-byte tag), so it must not be read as a
    /// single octet.
    #[allow(dead_code)]
    fn extract_proto_strings(bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0usize;

        // Reads a base-128 varint, returning (value, next_index).
        fn read_varint(bytes: &[u8], mut i: usize) -> Option<(u64, usize)> {
            let mut val: u64 = 0;
            let mut shift = 0u32;
            while i < bytes.len() {
                let b = bytes[i];
                i += 1;
                val |= ((b & 0x7f) as u64) << shift;
                if b & 0x80 == 0 {
                    return Some((val, i));
                }
                shift += 7;
                if shift > 63 {
                    return None;
                }
            }
            None
        }

        while i < bytes.len() {
            let Some((tag, next)) = read_varint(bytes, i) else {
                break;
            };
            i = next;
            match tag & 0x07 {
                2 => {
                    let Some((len, next)) = read_varint(bytes, i) else {
                        break;
                    };
                    i = next;
                    let len = len as usize;
                    if i + len > bytes.len() {
                        break;
                    }
                    let slice = &bytes[i..i + len];
                    i += len;
                    // Transcript text is printable text. A slice containing
                    // control bytes is a nested protobuf message (its first byte
                    // is a field tag), so descend instead of emitting it.
                    match std::str::from_utf8(slice) {
                        Ok(s) if !s.is_empty() && !s.chars().any(|c| c.is_control()) => {
                            out.push(s.to_string())
                        }
                        _ => out.extend(Self::extract_proto_strings(slice)),
                    }
                }
                0 => {
                    let Some((_, next)) = read_varint(bytes, i) else {
                        break;
                    };
                    i = next;
                }
                1 => i += 8,
                5 => i += 4,
                _ => break,
            }
        }
        out
    }

    pub(super) async fn transcribe_gemini(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        let key = if self.gemini_api_key.trim().is_empty() {
            Self::GEMINI_WEBSITE_KEY
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
