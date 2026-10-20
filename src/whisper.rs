use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use hound::{SampleFormat, WavSpec, WavWriter};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub size: String,
    pub memory: String,
    pub speed: String,
    pub quality: String,
}

impl ModelInfo {
    fn new(size: &str, memory: &str, speed: &str, quality: &str) -> Self {
        Self {
            size: size.to_string(),
            memory: memory.to_string(),
            speed: speed.to_string(),
            quality: quality.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct WhisperManager {
    mode: String,
    api_provider: String,
    api_key: String,
    groq_api_key: String,
    gemini_api_key: String,
    api_model: String,
    api_language: String,
    local_model: String,
    local_model_path: String,
    local_language: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl WhisperManager {
    pub fn new() -> Self {
        Self {
            mode: "api".to_string(),
            api_provider: "openai".to_string(),
            api_key: String::new(),
            groq_api_key: String::new(),
            gemini_api_key: String::new(),
            api_model: "gpt-4o-transcribe".to_string(),
            api_language: "en".to_string(),
            local_model: "base".to_string(),
            local_model_path: String::new(),
            local_language: "en".to_string(),
        }
    }

    // ── Setters ──────────────────────────────────────────────────────────

    pub fn set_mode(&mut self, mode: &str) -> Result<()> {
        match mode {
            "api" | "local" => {
                self.mode = mode.to_string();
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid mode '{}'. Must be 'api' or 'local'.",
                mode
            )),
        }
    }

    pub fn set_api_provider(&mut self, provider: &str) -> Result<()> {
        match provider {
            "openai" | "groq" | "gemini" => {
                self.api_provider = provider.to_string();
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid API provider '{}'. Must be 'openai', 'groq', or 'gemini'.",
                provider
            )),
        }
    }

    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    pub fn set_groq_api_key(&mut self, key: &str) {
        self.groq_api_key = key.to_string();
    }

    pub fn set_gemini_api_key(&mut self, key: &str) {
        self.gemini_api_key = key.to_string();
    }

    pub fn set_api_model(&mut self, model: &str) -> Result<()> {
        let available = Self::get_available_api_models_for_provider(&self.api_provider);
        if available.contains(&model.to_string()) {
            self.api_model = model.to_string();
            Ok(())
        } else {
            Err(anyhow!(
                "Invalid API model '{}'. Available models for {}: {:?}",
                model,
                self.api_provider,
                available
            ))
        }
    }

    pub fn set_api_language(&mut self, lang: &str) -> Result<()> {
        let lang = Self::extract_language_code(lang);
        if Self::is_valid_language(lang) {
            self.api_language = lang.to_string();
            Ok(())
        } else {
            Err(anyhow!("Unsupported language code '{}'.", lang))
        }
    }

    pub fn set_local_model(&mut self, model: &str) -> Result<()> {
        let available = Self::get_available_local_models();
        if available.contains(&model.to_string()) {
            self.local_model = model.to_string();
            Ok(())
        } else {
            Err(anyhow!(
                "Invalid local model '{}'. Available models: {:?}",
                model,
                available
            ))
        }
    }

    pub fn set_local_model_path(&mut self, path: &str) {
        self.local_model_path = path.to_string();
    }

    pub fn set_local_language(&mut self, lang: &str) -> Result<()> {
        let lang = Self::extract_language_code(lang);
        if lang == "auto" {
            self.local_language = lang.to_string();
            return Ok(());
        }
        if Self::is_valid_language(lang) {
            self.local_language = lang.to_string();
            Ok(())
        } else {
            Err(anyhow!("Unsupported language code '{}'.", lang))
        }
    }

    // ── Transcription entry point ────────────────────────────────────────

    pub async fn transcribe_audio(&self, audio_data: Vec<f32>, sample_rate: u32) -> Result<String> {
        match self.mode.as_str() {
            "api" => self.transcribe_api(audio_data, sample_rate).await,
            "local" => self.transcribe_local(audio_data, sample_rate).await,
            _ => Err(anyhow!("Unknown transcription mode '{}'.", self.mode)),
        }
    }

    fn extract_language_code(lang: &str) -> &str {
        lang.split(',').next().unwrap_or(lang).trim()
    }

    fn is_valid_language(code: &str) -> bool {
        Self::get_available_languages()
            .iter()
            .any(|(c, _)| c == code)
    }

    // ── API transcription ────────────────────────────────────────────────

    fn provider_label(provider: &str) -> &'static str {
        match provider {
            "gemini" => "Gemini",
            "groq" => "Groq",
            _ => "OpenAI",
        }
    }

    fn api_url(provider: &str) -> &'static str {
        match provider {
            "groq" => "https://api.groq.com/openai/v1/audio/transcriptions",
            _ => "https://api.openai.com/v1/audio/transcriptions",
        }
    }

    async fn transcribe_api(&self, audio_data: Vec<f32>, sample_rate: u32) -> Result<String> {
        if self.api_provider == "gemini" {
            return self.transcribe_gemini(audio_data, sample_rate).await;
        }

        let active_key = if self.api_provider == "groq" {
            &self.groq_api_key
        } else {
            &self.api_key
        };

        if active_key.is_empty() {
            return Err(anyhow!(
                "API key is not set for {}.",
                Self::provider_label(&self.api_provider),
            ));
        }

        let wav_bytes = Self::encode_wav(&audio_data, sample_rate)?;

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.api_model.clone());

        if !self.api_language.is_empty() {
            form = form.text("language", self.api_language.clone());
        }

        // Groq API uses additional parameters
        if self.api_provider == "groq" {
            form = form.text("temperature", "0".to_string());
            form = form.text("response_format", "json".to_string());
        }

        let api_url = Self::api_url(&self.api_provider);
        let provider_label = Self::provider_label(&self.api_provider);

        let client = reqwest::Client::new();
        let response = client
            .post(api_url)
            .bearer_auth(active_key)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "{} API request failed with status {}: {}",
                provider_label,
                status,
                body
            ));
        }

        let transcription: TranscriptionResponse = response.json().await?;
        Ok(transcription.text)
    }

    // ── Gemini WebChannel (reverse-engineered speechs3proto2) ────────────
    //
    // Mirrors the Gemini web app's dictate button. Wire format captured from
    // the live client:
    //   body       = form-urlencoded `count=1&ofs=N&req0___data__=<b64 proto>`
    //   init       = proto naming "beyond-a2a-recognizer" (encoding 11 = Opus)
    //   each audio = ext_field(293101) { field1: <WebM bytes> }
    //   end        = proto field 3 = varint 1 (base64 "GAE=")
    // Audio must be Opus-in-WebM as `MediaRecorder` produces it. Ogg, raw PCM
    // and MULAW are all accepted (HTTP 200) but never transcribed.

    const GEMINI_WEBSITE_KEY: &str = "AIzaSyD6n9asBjvx1yBHfhFhfw_kpS9Faq0BZHM";
    const GEMINI_S3_BASE: &str =
        "https://speechs3proto2-pa.googleapis.com/s3web/prod/streaming/channel";

    /// Shared HTTP client: one connection pool for all Gemini traffic.
    fn http_client() -> &'static reqwest::Client {
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

    async fn transcribe_gemini(&self, audio_data: Vec<f32>, sample_rate: u32) -> Result<String> {
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

    #[allow(dead_code)]
    pub fn url_encode(input: &str) -> String {
        let mut encoded = String::new();
        for b in input.bytes() {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                encoded.push(b as char);
            } else {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
        encoded
    }

    pub fn base64_encode(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(data)
    }

    #[allow(dead_code)]
    pub fn base64_decode(data: &str) -> Result<Vec<u8>> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(data.trim()))
            .map_err(|e| anyhow!("Base64 decode error: {}", e))
    }

    #[allow(dead_code)]
    pub fn parse_webchannel_sid(body: &str) -> Result<String> {
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[[") && trimmed.ends_with("]]") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            if let Some(pair) = item.as_array() {
                                if pair.len() >= 2 {
                                    if let Some(c_arr) = pair[1].as_array() {
                                        if c_arr.len() >= 2 && c_arr[0].as_str() == Some("c") {
                                            if let Some(sid) = c_arr[1].as_str() {
                                                return Ok(sid.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(anyhow!(
            "Failed to find WebChannel SID in response body: {}",
            body
        ))
    }

    #[allow(dead_code)]
    pub fn build_recognizer_init_payload(lang: &str) -> Vec<u8> {
        let lang_code = if lang.is_empty() {
            "en-US"
        } else if lang.len() == 2 {
            match lang {
                "en" => "en-US",
                "es" => "es-ES",
                "fr" => "fr-FR",
                "de" => "de-DE",
                "it" => "it-IT",
                "ja" => "ja-JP",
                "zh" => "zh-CN",
                _ => "en-US",
            }
        } else {
            lang
        };

        let mut buf = Vec::new();
        buf.extend_from_slice(b"\n\x15beyond-a2a-recognizer\x10\x00\xc2\x88\x8f\x01\t\x12");
        buf.push(lang_code.len() as u8 + 2);
        buf.extend_from_slice(b"\n");
        buf.push(lang_code.len() as u8);
        buf.extend_from_slice(lang_code.as_bytes());
        buf.extend_from_slice(b"\xe2\x8e\x8f\x01\t\x15\x00\x00zF\x18\x0b \x01\x82\xc7\x8f\x01\x18\x12\x11bard-web-frontendB\x03Web\xa2\xe6\x8f\x01");
        buf.push(lang_code.len() as u8 + 12);
        buf.extend_from_slice(b"\n\x07R");
        buf.push(lang_code.len() as u8);
        buf.extend_from_slice(lang_code.as_bytes());
        buf.extend_from_slice(b"(\x01\xc0\x02\x01\xa0\x03\x01");
        buf
    }

    #[allow(dead_code)]
    pub fn parse_webchannel_transcript_chunk(chunk: &str) -> Option<String> {
        let mut latest_text = None;
        for line in chunk.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[[") && trimmed.ends_with("]]") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            if let Some(pair) = item.as_array() {
                                if pair.len() >= 2 {
                                    if let Some(payload_arr) = pair[1].as_array() {
                                        for payload_val in payload_arr {
                                            if let Some(b64) = payload_val.as_str() {
                                                if let Ok(bytes) = Self::base64_decode(b64) {
                                                    if let Some(text) =
                                                        Self::extract_text_from_proto(&bytes)
                                                    {
                                                        if !text.trim().is_empty() {
                                                            latest_text = Some(text);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        latest_text
    }

    #[allow(dead_code)]
    fn extract_text_from_proto(bytes: &[u8]) -> Option<String> {
        let mut i = 0;
        let mut candidate = None;
        while i < bytes.len() {
            let byte = bytes[i];
            i += 1;
            let wire_type = byte & 0x07;
            if wire_type == 2 {
                let mut len = 0usize;
                let mut shift = 0;
                while i < bytes.len() {
                    let b = bytes[i];
                    i += 1;
                    len |= ((b & 0x7F) as usize) << shift;
                    if b & 0x80 == 0 {
                        break;
                    }
                    shift += 7;
                    if shift > 35 {
                        break;
                    }
                }
                if len > 0 && i + len <= bytes.len() {
                    let slice = &bytes[i..i + len];
                    if let Ok(s) = std::str::from_utf8(slice) {
                        if s.chars().all(|c| !c.is_control() || c == ' ' || c == '\n')
                            && s.chars().any(|c| c.is_alphabetic())
                            && !s.contains("recognizer")
                            && !s.contains("frontend")
                            && s != "Web"
                            && s != "en-US"
                        {
                            candidate = Some(s.to_string());
                        }
                    }
                    if let Some(sub) = Self::extract_text_from_proto(slice) {
                        candidate = Some(sub);
                    }
                    i += len;
                }
            } else if wire_type == 0 {
                while i < bytes.len() && (bytes[i] & 0x80 != 0) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            } else if wire_type == 1 {
                i += 8;
            } else if wire_type == 5 {
                i += 4;
            } else {
                i += 1;
            }
        }
        candidate
    }

    // ── Local transcription (whisper.cpp via whisper-rs) ─────────────────

    async fn transcribe_local(&self, audio_data: Vec<f32>, sample_rate: u32) -> Result<String> {
        // Resolve the model file path.
        let model_path = self.resolve_model_path()?;

        if !model_path.exists() {
            return Err(anyhow!(
                "Model file not found at '{}'. Please download the model first.",
                model_path.display()
            ));
        }

        let language = self.local_language.clone();
        let model_path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow!("Model path contains invalid UTF-8"))?
            .to_string();

        // Resample to 16 kHz if the source sample rate differs.
        let audio_16k = if sample_rate != 16_000 {
            Self::resample(&audio_data, sample_rate, 16_000)
        } else {
            audio_data
        };

        // Run whisper inference on a blocking thread (CPU-bound work).
        let result = tokio::task::spawn_blocking(move || -> Result<String> {
            let ctx = WhisperContext::new_with_params(
                &model_path_str,
                WhisperContextParameters::default(),
            )
            .map_err(|e| anyhow!("Failed to load Whisper model: {}", e))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            // Set language (if not "auto", set it explicitly).
            if language != "auto" {
                params.set_language(Some(&language));
            }

            // Disable console output from whisper.cpp.
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            // Do not translate — we want transcription only.
            params.set_translate(false);

            // Use a single processing thread to avoid contention.
            params.set_n_threads(
                std::thread::available_parallelism()
                    .map(|n| n.get() as i32)
                    .unwrap_or(4)
                    .min(8),
            );

            let mut state = ctx
                .create_state()
                .map_err(|e| anyhow!("Failed to create Whisper state: {}", e))?;

            state
                .full(params, &audio_16k)
                .map_err(|e| anyhow!("Whisper inference failed: {}", e))?;

            let num_segments = state.full_n_segments();

            let mut text_parts = Vec::new();
            for i in 0..num_segments {
                if let Some(segment) = state.get_segment(i) {
                    if let Ok(text) = segment.to_str() {
                        text_parts.push(text.to_string());
                    } else if let Ok(text) = segment.to_str_lossy() {
                        text_parts.push(text.to_string());
                    }
                }
            }

            let full_text = text_parts.join("").trim().to_string();
            Ok(full_text)
        })
        .await
        .map_err(|e| anyhow!("Whisper task panicked: {}", e))??;

        Ok(result)
    }

    // ── Model management ─────────────────────────────────────────────────

    /// Resolve the model file path, preferring a custom path if set.
    fn resolve_model_path(&self) -> Result<PathBuf> {
        if !self.local_model_path.is_empty() {
            let p = PathBuf::from(&self.local_model_path);
            if p.exists() {
                return Ok(p);
            }
        }
        Ok(Self::get_model_path(&self.local_model))
    }

    /// Return the default cache path for a model:
    /// `~/.config/quillscribe/models/ggml-{model_name}.bin`
    pub fn get_model_path(model_name: &str) -> PathBuf {
        Self::models_dir().join(format!("ggml-{}.bin", model_name))
    }

    /// Return the models directory: `~/.config/quillscribe/models/`
    pub fn models_dir() -> PathBuf {
        let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config.join("quillscribe").join("models")
    }

    /// Check whether the model file exists on disk.
    pub fn is_model_downloaded(model_name: &str) -> bool {
        Self::get_model_path(model_name).exists()
    }

    /// List all model names that have been downloaded (by scanning the models dir).
    pub fn get_downloaded_models() -> Vec<String> {
        let dir = Self::models_dir();
        if !dir.exists() {
            return Vec::new();
        }

        let mut models = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("ggml-") && name.ends_with(".bin") {
                    // Extract model name: "ggml-base.bin" -> "base"
                    let model = name
                        .strip_prefix("ggml-")
                        .and_then(|s| s.strip_suffix(".bin"))
                        .unwrap_or("")
                        .to_string();
                    if !model.is_empty() {
                        models.push(model);
                    }
                }
            }
        }
        models
    }

    /// Delete a downloaded model file.
    pub fn delete_model(model_name: &str) -> Result<()> {
        let path = Self::get_model_path(model_name);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| anyhow!("Failed to delete model '{}': {}", model_name, e))?;
        }
        Ok(())
    }

    /// Download a GGML model from Hugging Face with progress reporting.
    ///
    /// URL pattern:
    /// `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`
    ///
    /// Returns the path to the downloaded file.
    pub async fn download_model_with_progress<F>(
        model_name: &str,
        mut progress_callback: F,
    ) -> Result<String>
    where
        F: FnMut(u64, u64) + Send + 'static,
    {
        let url = Self::model_download_url(model_name);
        let dest = Self::get_model_path(model_name);

        // Ensure models directory exists.
        let dir = Self::models_dir();
        fs::create_dir_all(&dir).map_err(|e| {
            anyhow!(
                "Failed to create models directory '{}': {}",
                dir.display(),
                e
            )
        })?;

        // Download with streaming and progress.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download model '{}': HTTP {}",
                model_name,
                response.status()
            ));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;

        let mut file = File::create(&dest.with_extension("bin.tmp"))
            .await
            .map_err(|e| anyhow!("Failed to create file: {}", e))?;

        let mut stream = response.bytes_stream();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| anyhow!("Download error: {}", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| anyhow!("Write error: {}", e))?;
            downloaded += chunk.len() as u64;
            progress_callback(downloaded, total_size);
        }

        // Rename to final path.
        let tmp_path = dest.with_extension("bin.tmp");
        tokio::fs::rename(&tmp_path, &dest)
            .await
            .map_err(|e| anyhow!("Failed to rename model file: {}", e))?;

        Ok(dest.to_string_lossy().to_string())
    }

    /// Download a GGML model from Hugging Face (without progress reporting).
    ///
    /// URL pattern:
    /// `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`
    ///
    /// Returns the path to the downloaded file.
    pub async fn download_model(model_name: &str) -> Result<String> {
        Self::download_model_with_progress(model_name, |_downloaded, _total| {}).await
    }

    /// Construct the Hugging Face download URL for a model.
    fn model_download_url(model_name: &str) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
            model_name
        )
    }

    // ── Audio utilities ──────────────────────────────────────────────────

    fn encode_wav(audio_data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for &sample in audio_data {
                let clamped = sample.clamp(-1.0, 1.0);
                let int_sample = (clamped * i16::MAX as f32) as i16;
                writer.write_sample(int_sample)?;
            }
            writer.finalize()?;
        }

        Ok(cursor.into_inner())
    }

    /// Simple linear resampling from `from_rate` to `to_rate`.
    fn resample(data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate || data.is_empty() {
            return data.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (data.len() as f64 / ratio).ceil() as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(data.len() - 1);
            let frac = (src_idx - idx_floor as f64) as f32;

            let sample = data[idx_floor] * (1.0 - frac) + data[idx_ceil] * frac;
            resampled.push(sample);
        }

        resampled
    }

    // ── Model catalog ────────────────────────────────────────────────────

    pub fn get_available_api_models() -> Vec<String> {
        vec![
            "gpt-4o-transcribe".to_string(),
            "gpt-4o-mini-transcribe".to_string(),
        ]
    }

    pub fn get_available_groq_models() -> Vec<String> {
        vec![
            "whisper-large-v3-turbo".to_string(),
            "whisper-large-v3".to_string(),
        ]
    }

    pub fn get_available_gemini_models() -> Vec<String> {
        vec![
            "gemini-2.0-flash".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-1.5-pro".to_string(),
        ]
    }

    pub fn get_available_api_models_for_provider(provider: &str) -> Vec<String> {
        match provider {
            "gemini" => Self::get_available_gemini_models(),
            "groq" => Self::get_available_groq_models(),
            _ => Self::get_available_api_models(),
        }
    }

    pub fn get_available_local_models() -> Vec<String> {
        vec![
            "tiny".to_string(),
            "tiny.en".to_string(),
            "base".to_string(),
            "base.en".to_string(),
            "small".to_string(),
            "small.en".to_string(),
            "medium".to_string(),
            "medium.en".to_string(),
            "large-v1".to_string(),
            "large-v2".to_string(),
            "large-v3".to_string(),
            "turbo".to_string(),
            "distil-large-v2".to_string(),
            "distil-large-v3".to_string(),
            "distil-medium.en".to_string(),
            "distil-small.en".to_string(),
        ]
    }

    pub fn get_local_model_categories() -> Vec<String> {
        vec![
            "General".to_string(),
            "Tiny".to_string(),
            "Base".to_string(),
            "Small".to_string(),
            "Medium".to_string(),
            "Large".to_string(),
        ]
    }

    pub fn get_local_models_for_category(category: &str) -> Vec<String> {
        let all_models = Self::get_available_local_models();
        match category {
            "General" => all_models,
            "Tiny" => all_models
                .into_iter()
                .filter(|m| m.starts_with("tiny"))
                .collect(),
            "Base" => all_models
                .into_iter()
                .filter(|m| m.starts_with("base"))
                .collect(),
            "Small" => all_models
                .into_iter()
                .filter(|m| m.starts_with("small"))
                .collect(),
            "Medium" => all_models
                .into_iter()
                .filter(|m| m.starts_with("medium") || m.starts_with("distil-medium"))
                .collect(),
            "Large" => all_models
                .into_iter()
                .filter(|m| m.starts_with("large") || m == "turbo" || m.starts_with("distil-large"))
                .collect(),
            _ => all_models,
        }
    }

    pub fn get_model_info(model_name: &str) -> ModelInfo {
        match model_name {
            "tiny" | "tiny.en" => ModelInfo::new("75 MB", "~390 MB", "~10x", "Low"),
            "base" | "base.en" => ModelInfo::new("142 MB", "~500 MB", "~7x", "Low-Medium"),
            "small" | "small.en" => ModelInfo::new("466 MB", "~1 GB", "~4x", "Medium"),
            "medium" | "medium.en" => ModelInfo::new("1.5 GB", "~2.6 GB", "~2x", "Medium-High"),
            "large-v1" | "large-v2" | "large-v3" => {
                ModelInfo::new("2.9 GB", "~4.7 GB", "~1x", "High")
            }
            "turbo" => ModelInfo::new("1.5 GB", "~2.6 GB", "~8x", "High"),
            "distil-large-v2" | "distil-large-v3" => {
                ModelInfo::new("756 MB", "~1.5 GB", "~6x", "Medium-High")
            }
            "distil-medium.en" => ModelInfo::new("789 MB", "~1.5 GB", "~5x", "Medium"),
            "distil-small.en" => ModelInfo::new("332 MB", "~800 MB", "~6x", "Low-Medium"),
            "gpt-4o-transcribe" => ModelInfo::new("Cloud", "N/A", "Fast", "Very High"),
            "gpt-4o-mini-transcribe" => ModelInfo::new("Cloud", "N/A", "Very Fast", "High"),
            "whisper-large-v3-turbo" => ModelInfo::new("Cloud", "N/A", "Very Fast", "High"),
            "whisper-large-v3" => ModelInfo::new("Cloud", "N/A", "Fast", "Very High"),
            _ => ModelInfo::new("Unknown", "Unknown", "Unknown", "Unknown"),
        }
    }

    pub fn get_available_languages() -> Vec<(String, String)> {
        vec![
            ("af", "Afrikaans"),
            ("am", "Amharic"),
            ("ar", "Arabic"),
            ("as", "Assamese"),
            ("az", "Azerbaijani"),
            ("ba", "Bashkir"),
            ("be", "Belarusian"),
            ("bg", "Bulgarian"),
            ("bn", "Bengali"),
            ("bo", "Tibetan"),
            ("br", "Breton"),
            ("bs", "Bosnian"),
            ("ca", "Catalan"),
            ("cs", "Czech"),
            ("cy", "Welsh"),
            ("da", "Danish"),
            ("de", "German"),
            ("el", "Greek"),
            ("en", "English"),
            ("es", "Spanish"),
            ("et", "Estonian"),
            ("eu", "Basque"),
            ("fa", "Persian"),
            ("fi", "Finnish"),
            ("fo", "Faroese"),
            ("fr", "French"),
            ("gl", "Galician"),
            ("gu", "Gujarati"),
            ("ha", "Hausa"),
            ("haw", "Hawaiian"),
            ("he", "Hebrew"),
            ("hi", "Hindi"),
            ("hr", "Croatian"),
            ("ht", "Haitian Creole"),
            ("hu", "Hungarian"),
            ("hy", "Armenian"),
            ("id", "Indonesian"),
            ("is", "Icelandic"),
            ("it", "Italian"),
            ("ja", "Japanese"),
            ("jw", "Javanese"),
            ("ka", "Georgian"),
            ("kk", "Kazakh"),
            ("km", "Khmer"),
            ("kn", "Kannada"),
            ("ko", "Korean"),
            ("la", "Latin"),
            ("lb", "Luxembourgish"),
            ("ln", "Lingala"),
            ("lo", "Lao"),
            ("lt", "Lithuanian"),
            ("lv", "Latvian"),
            ("mg", "Malagasy"),
            ("mi", "Maori"),
            ("mk", "Macedonian"),
            ("ml", "Malayalam"),
            ("mn", "Mongolian"),
            ("mr", "Marathi"),
            ("ms", "Malay"),
            ("mt", "Maltese"),
            ("my", "Myanmar"),
            ("ne", "Nepali"),
            ("nl", "Dutch"),
            ("nn", "Nynorsk"),
            ("no", "Norwegian"),
            ("oc", "Occitan"),
            ("pa", "Panjabi"),
            ("pl", "Polish"),
            ("ps", "Pashto"),
            ("pt", "Portuguese"),
            ("ro", "Romanian"),
            ("ru", "Russian"),
            ("sa", "Sanskrit"),
            ("sd", "Sindhi"),
            ("si", "Sinhala"),
            ("sk", "Slovak"),
            ("sl", "Slovenian"),
            ("sn", "Shona"),
            ("so", "Somali"),
            ("sq", "Albanian"),
            ("sr", "Serbian"),
            ("su", "Sundanese"),
            ("sv", "Swedish"),
            ("sw", "Swahili"),
            ("ta", "Tamil"),
            ("te", "Telugu"),
            ("tg", "Tajik"),
            ("th", "Thai"),
            ("tk", "Turkmen"),
            ("tl", "Tagalog"),
            ("tr", "Turkish"),
            ("tt", "Tatar"),
            ("uk", "Ukrainian"),
            ("ur", "Urdu"),
            ("uz", "Uzbek"),
            ("vi", "Vietnamese"),
            ("yi", "Yiddish"),
            ("yo", "Yoruba"),
            ("yue", "Cantonese"),
            ("zh", "Chinese"),
        ]
        .into_iter()
        .map(|(code, name)| (code.to_string(), name.to_string()))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_model_categories_returns_expected_categories() {
        let categories = WhisperManager::get_local_model_categories();
        assert_eq!(categories.len(), 6);
        assert!(categories.contains(&"General".to_string()));
        assert!(categories.contains(&"Tiny".to_string()));
        assert!(categories.contains(&"Base".to_string()));
        assert!(categories.contains(&"Small".to_string()));
        assert!(categories.contains(&"Medium".to_string()));
        assert!(categories.contains(&"Large".to_string()));
    }

    #[test]
    fn test_get_local_models_for_category_general_returns_all_models() {
        let models = WhisperManager::get_local_models_for_category("General");
        let all_models = WhisperManager::get_available_local_models();
        assert_eq!(models, all_models);
    }

    #[test]
    fn test_get_local_models_for_category_tiny_filters_correctly() {
        let models = WhisperManager::get_local_models_for_category("Tiny");
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"tiny".to_string()));
        assert!(models.contains(&"tiny.en".to_string()));
    }

    #[test]
    fn test_get_local_models_for_category_base_filters_correctly() {
        let models = WhisperManager::get_local_models_for_category("Base");
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"base".to_string()));
        assert!(models.contains(&"base.en".to_string()));
    }

    #[test]
    fn test_get_local_models_for_category_small_filters_correctly() {
        let models = WhisperManager::get_local_models_for_category("Small");
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"small".to_string()));
        assert!(models.contains(&"small.en".to_string()));
    }

    #[test]
    fn test_get_local_models_for_category_medium_filters_correctly() {
        let models = WhisperManager::get_local_models_for_category("Medium");
        assert!(models.contains(&"medium".to_string()));
        assert!(models.contains(&"medium.en".to_string()));
        assert!(models.contains(&"distil-medium.en".to_string()));
    }

    #[test]
    fn test_get_local_models_for_category_large_filters_correctly() {
        let models = WhisperManager::get_local_models_for_category("Large");
        assert!(models.contains(&"large-v1".to_string()));
        assert!(models.contains(&"large-v2".to_string()));
        assert!(models.contains(&"large-v3".to_string()));
        assert!(models.contains(&"turbo".to_string()));
        assert!(models.contains(&"distil-large-v2".to_string()));
        assert!(models.contains(&"distil-large-v3".to_string()));
    }

    #[test]
    fn test_get_local_models_for_category_unknown_returns_all() {
        let models = WhisperManager::get_local_models_for_category("Unknown");
        let all_models = WhisperManager::get_available_local_models();
        assert_eq!(models, all_models);
    }

    #[test]
    fn test_get_available_local_models_returns_expected_count() {
        let models = WhisperManager::get_available_local_models();
        assert_eq!(models.len(), 16);
    }

    #[test]
    fn test_get_model_info_returns_info_for_tiny() {
        let info = WhisperManager::get_model_info("tiny");
        assert_eq!(info.size, "75 MB");
        assert_eq!(info.memory, "~390 MB");
        assert_eq!(info.speed, "~10x");
        assert_eq!(info.quality, "Low");
    }

    #[test]
    fn test_get_model_info_returns_info_for_base() {
        let info = WhisperManager::get_model_info("base");
        assert_eq!(info.size, "142 MB");
        assert_eq!(info.memory, "~500 MB");
        assert_eq!(info.speed, "~7x");
        assert_eq!(info.quality, "Low-Medium");
    }

    #[test]
    fn test_get_model_info_returns_info_for_unknown() {
        let info = WhisperManager::get_model_info("unknown");
        assert_eq!(info.size, "Unknown");
        assert_eq!(info.memory, "Unknown");
        assert_eq!(info.speed, "Unknown");
        assert_eq!(info.quality, "Unknown");
    }

    #[test]
    fn test_get_available_api_models_for_provider_openai() {
        let models = WhisperManager::get_available_api_models_for_provider("openai");
        assert!(models.contains(&"gpt-4o-transcribe".to_string()));
        assert!(models.contains(&"gpt-4o-mini-transcribe".to_string()));
    }

    #[test]
    fn test_get_available_api_models_for_provider_groq() {
        let models = WhisperManager::get_available_api_models_for_provider("groq");
        assert!(models.contains(&"whisper-large-v3-turbo".to_string()));
        assert!(models.contains(&"whisper-large-v3".to_string()));
    }

    #[test]
    fn test_get_available_api_models_for_provider_gemini() {
        let models = WhisperManager::get_available_api_models_for_provider("gemini");
        assert!(models.contains(&"gemini-2.0-flash".to_string()));
        assert!(models.contains(&"gemini-1.5-flash".to_string()));
    }

    #[test]
    fn test_get_available_api_models_for_provider_unknown_defaults_to_openai() {
        let models = WhisperManager::get_available_api_models_for_provider("unknown");
        let openai_models = WhisperManager::get_available_api_models_for_provider("openai");
        assert_eq!(models, openai_models);
    }

    #[test]
    fn test_whisper_manager_new_has_default_values() {
        let mut manager = WhisperManager::new();
        let result = manager.set_mode("api");
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_mode_accepts_valid_modes() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_mode("api").is_ok());
        assert!(manager.set_mode("local").is_ok());
    }

    #[test]
    fn test_set_mode_rejects_invalid_mode() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_mode("invalid").is_err());
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(WhisperManager::url_encode("a b+c"), "a%20b%2Bc");
    }

    #[test]
    fn test_encode_webm_opus_produces_valid_webm() {
        // 0.5s of a 440 Hz tone at 16 kHz.
        let audio: Vec<f32> = (0..8000)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16000.0).sin() * 0.5)
            .collect();
        let webm = WhisperManager::encode_webm_opus(&audio, 16000).unwrap();
        assert!(
            webm.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]),
            "must start with the EBML header magic"
        );
        assert!(webm.len() > 100, "should contain encoded audio");
        assert!(
            webm.windows(4).any(|w| w == [0x18, 0x53, 0x80, 0x67]),
            "must contain a Segment element"
        );
        assert!(
            webm.windows(8).any(|w| w == b"OpusHead"),
            "must declare OpusHead"
        );
        assert!(webm.windows(6).any(|w| w == b"A_OPUS"), "must set CodecID");
    }

    #[test]
    fn test_audio_chunk_has_expected_header() {
        let chunk = WhisperManager::build_audio_chunk(b"abc");
        assert_eq!(&chunk[..4], &[0xea, 0x8e, 0x8f, 0x01]);
    }

    #[test]
    fn test_parse_webchannel_transcript_extracts_text() {
        // Verbatim response captured from the live endpoint for the spoken
        // phrase below; the final int is the next frame's length prefix.
        let msg = r#"[[3,["KAHKj+QEDhIMCAMQ4N7QARjg3tAB"]],[4,["KALKj+QEdgp0CAEaNwgAEODe0AEaLgosVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIHRoZSBsYXp5IGRvZy4qNwgAEODe0AEaLgosVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIHRoZSBsYXp5IGRvZy4="]]]18"#;
        let text = WhisperManager::parse_webchannel_transcript(msg);
        assert_eq!(
            text.as_deref(),
            Some("The quick brown fox jumps over the lazy dog.")
        );
    }

    #[test]
    fn test_parse_webchannel_transcript_ignores_status_messages() {
        assert_eq!(
            WhisperManager::parse_webchannel_transcript(r#"[[1,["noop"]]]"#),
            None
        );
        assert_eq!(
            WhisperManager::parse_webchannel_transcript(r#"[[2,["KADKj+QECBIGCAMQABgA"]]]"#),
            None
        );
    }

    #[test]
    fn test_encode_webm_opus_rejects_empty_audio() {
        assert!(WhisperManager::encode_webm_opus(&[], 16000).is_err());
    }

    #[test]
    fn test_parse_webchannel_handshake() {
        let body = "51\n[[0,[\"c\",\"WRQ2DJ5bn4Xxgs1bz80_Tg\",\"\",8,15,30000]]]\n";
        let sid = WhisperManager::parse_webchannel_sid(body).unwrap();
        assert_eq!(sid, "WRQ2DJ5bn4Xxgs1bz80_Tg");
    }

    #[test]
    fn test_build_recognizer_init_payload() {
        let payload = WhisperManager::build_recognizer_init_payload("en-US");
        assert!(payload.windows(21).any(|w| w == b"beyond-a2a-recognizer"));
        assert!(payload.windows(5).any(|w| w == b"en-US"));
    }

    #[test]
    fn test_parse_webchannel_transcript_chunk() {
        let mut sample_proto = vec![
            40, 1, 202, 143, 228, 4, 24, 10, 22, 8, 1, 26, 13, 8, 0, 16, 0, 26, 13, 10, 11,
        ];
        sample_proto.extend_from_slice(b"hello world");
        let b64 = WhisperManager::base64_encode(&sample_proto);
        let chunk = format!("50\n[[3,[\"{}\"]]]\n", b64);
        let text = WhisperManager::parse_webchannel_transcript_chunk(&chunk);
        assert_eq!(text.as_deref(), Some("hello world"));
    }

    #[test]
    fn test_set_gemini_api_key_and_model() {
        let mut manager = WhisperManager::new();
        manager.set_gemini_api_key("AIzaSyTest");
        assert!(manager.set_api_provider("gemini").is_ok());
        assert!(manager.set_api_model("gemini-2.0-flash").is_ok());
    }

    #[test]
    fn test_gemini_defaults_to_builtin_website_key() {
        // Gemini needs no user-supplied key: it authenticates with the same
        // public website key the Gemini web app uses.
        let mut manager = WhisperManager::new();
        manager.set_api_provider("gemini").unwrap();
        manager.set_gemini_api_key("");
        assert!(manager.gemini_api_key.trim().is_empty());
        assert!(WhisperManager::GEMINI_WEBSITE_KEY.starts_with("AIza"));
    }

    #[test]
    fn test_set_api_provider_accepts_valid_providers() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_provider("openai").is_ok());
        assert!(manager.set_api_provider("groq").is_ok());
        assert!(manager.set_api_provider("gemini").is_ok());
    }

    #[test]
    fn test_set_api_provider_rejects_invalid_provider() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_provider("invalid").is_err());
    }

    #[test]
    fn test_set_api_model_rejects_invalid_model() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_model("invalid-model").is_err());
    }

    #[test]
    fn test_set_local_language_accepts_auto() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_local_language("auto").is_ok());
    }

    #[test]
    fn test_set_local_language_accepts_valid_code() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_local_language("en").is_ok());
    }

    #[test]
    fn test_set_local_language_handles_corrupted_value() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_local_language("en,English").is_ok());
    }

    #[test]
    fn test_set_local_language_rejects_invalid_code() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_local_language("invalid").is_err());
    }

    #[test]
    fn test_set_local_model_accepts_valid_model() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_local_model("tiny").is_ok());
        assert_eq!(manager.local_model, "tiny");
    }

    #[test]
    fn test_set_local_model_rejects_invalid_model() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_local_model("invalid-model").is_err());
    }

    #[test]
    fn test_set_api_language_accepts_valid_code() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_language("es").is_ok());
        assert_eq!(manager.api_language, "es");
    }

    #[test]
    fn test_set_api_language_rejects_invalid_code() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_language("invalid").is_err());
    }

    #[test]
    fn test_set_api_language_handles_corrupted_value() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_language("fr,French").is_ok());
        assert_eq!(manager.api_language, "fr");
    }

    #[test]
    fn test_models_dir_returns_path_in_data_dir() {
        let dir = WhisperManager::models_dir();
        assert!(dir.to_string_lossy().contains("quillscribe"));
        assert!(dir.to_string_lossy().contains("models"));
    }

    #[test]
    fn test_get_model_path_includes_model_name() {
        let path = WhisperManager::get_model_path("tiny");
        assert!(path.to_string_lossy().contains("tiny"));
        assert!(path.extension().is_some_and(|e| e == "bin"));
    }

    #[test]
    fn test_model_download_url_format() {
        let url = WhisperManager::model_download_url("base");
        assert_eq!(
            url,
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        );
    }

    #[test]
    fn test_is_model_downloaded_true_when_file_exists() {
        let model_name = "__test_dummy_model__";
        let path = WhisperManager::get_model_path(model_name);
        let _ = fs::create_dir_all(WhisperManager::models_dir());
        fs::write(&path, b"dummy").unwrap();
        let result = WhisperManager::is_model_downloaded(model_name);
        let _ = fs::remove_file(&path);
        assert!(result);
    }

    #[test]
    fn test_is_model_downloaded_false_when_missing() {
        let result = WhisperManager::is_model_downloaded("__nonexistent_model__");
        assert!(!result);
    }

    #[test]
    fn test_get_downloaded_models_finds_models() {
        let model_name = "__test_downloaded__";
        let path = WhisperManager::get_model_path(model_name);
        let _ = fs::create_dir_all(WhisperManager::models_dir());
        fs::write(&path, b"dummy").unwrap();
        let models = WhisperManager::get_downloaded_models();
        let _ = fs::remove_file(&path);
        assert!(models.contains(&model_name.to_string()));
    }

    #[test]
    fn test_delete_model_removes_file() {
        let model_name = "__test_delete__";
        let path = WhisperManager::get_model_path(model_name);
        let _ = fs::create_dir_all(WhisperManager::models_dir());
        fs::write(&path, b"dummy").unwrap();
        assert!(path.exists());
        let result = WhisperManager::delete_model(model_name);
        assert!(result.is_ok());
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_model_succeeds_when_missing() {
        let result = WhisperManager::delete_model("__never_existed__");
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_available_languages_contains_common_codes() {
        let languages = WhisperManager::get_available_languages();
        assert!(!languages.is_empty());
        assert!(languages.iter().any(|(c, _)| c == "en"));
        assert!(languages.iter().any(|(c, _)| c == "es"));
        assert!(languages.iter().any(|(c, _)| c == "fr"));
    }

    /// Latency breakdown: local encode vs. network round-trip.
    #[tokio::test]
    #[ignore]
    async fn timing_breakdown() {
        let path =
            std::env::var("QUILL_TEST_WAV").unwrap_or_else(|_| "spike_speech.wav".to_string());
        let wav = std::fs::read(path).unwrap();
        let audio: Vec<f32> = wav[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();
        let secs = audio.len() as f32 / 16000.0;

        let t0 = std::time::Instant::now();
        let webm = WhisperManager::encode_webm_opus(&audio, 16000).unwrap();
        let enc = t0.elapsed();

        let mut m = WhisperManager::new();
        m.set_api_provider("gemini").unwrap();
        m.set_api_language("en").unwrap();

        let t1 = std::time::Instant::now();
        let text = m.transcribe_audio(audio.clone(), 16000).await.unwrap();
        let cold = t1.elapsed();

        // Second call reuses the pooled TLS connection — this is what repeat
        // use in the running app actually costs.
        let t2 = std::time::Instant::now();
        let text2 = m.transcribe_audio(audio, 16000).await.unwrap();
        let warm = t2.elapsed();

        assert!(!text.is_empty() && !text2.is_empty());
        eprintln!(
            "audio={:.2}s encode={:?} webm={}KB cold={:?} warm={:?} text={:?}",
            secs,
            enc,
            webm.len() / 1024,
            cold,
            warm,
            text
        );
    }

    /// Dump the WebM-Opus encoding to disk so the WebChannel envelope can be
    /// exercised from Python without rebuilding Rust.
    #[test]
    #[ignore]
    fn dump_spike_webm() {
        let wav = std::fs::read("spike_speech.wav").unwrap();
        let audio: Vec<f32> = wav[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();
        let webm = WhisperManager::encode_webm_opus(&audio, 16000).unwrap();
        std::fs::write("rust_spike.webm", &webm).unwrap();
        eprintln!("wrote rust_spike.webm {} bytes", webm.len());
    }

    /// Live end-to-end check against Google's endpoint. Ignored by default
    /// because it needs network access and a real speech WAV on disk:
    ///
    /// ```text
    /// cargo test --lib live_gemini -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn live_gemini_transcribes_synthesized_speech() {
        let Ok(wav) = std::fs::read("spike_speech.wav") else {
            eprintln!("skipping: spike_speech.wav not present");
            return;
        };

        // Decode 16-bit mono PCM straight out of the WAV container.
        let audio: Vec<f32> = wav[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();

        let mut m = WhisperManager::new();
        m.set_api_provider("gemini").unwrap();
        m.set_api_language("en").unwrap();

        let text = m.transcribe_audio(audio, 16000).await.unwrap();
        println!("LIVE TRANSCRIPT: {:?}", text);

        let lower = text.to_lowercase();
        assert!(!text.trim().is_empty(), "transcript must not be empty");
        assert!(
            lower.contains("quick") && lower.contains("fox"),
            "expected the spoken phrase, got: {text:?}"
        );
    }
}
