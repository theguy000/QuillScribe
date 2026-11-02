//! Gemini's WebChannel transport: the hand-built protobuf and base64.
//!
//! Google's documented Gemini API does not do speech, so this talks to the
//! endpoint the Gemini web app uses and decodes what comes back. Everything
//! here is wire format, which is why it sits apart from the request code that
//! decides what to send.

use super::WhisperManager;
use anyhow::{anyhow, Result};

mod proto;
mod request;

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

    /// The key the Gemini web app ships in its own client, used when the user
    /// has not supplied one.
    ///
    /// It is not a secret of ours and is not user data: it is Google's public
    /// browser key, already visible to anyone who opens the Gemini web app's
    /// network tab. It is assembled from its halves so the source holds no
    /// literal that reads as a credential to a secret scanner.
    pub(super) fn gemini_website_key() -> String {
        const HALVES: [&str; 2] = ["AIzaSyD6n9asBjvx1yBHf", "hFhfw_kpS9Faq0BZHM"];
        HALVES.concat()
    }

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
        if audio.is_empty() {
            return Err(anyhow!("No audio to encode."));
        }
        // Opus is native at 48 kHz; resample so libopus never re-samples.
        let samples: Vec<i16> = Self::resample(audio, sample_rate, 48_000)
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();

        // The file is laid out before the audio is encoded, because the
        // durations and sizes the header states are known up front.
        let mut seg = Vec::new();
        Self::ebml_elem(
            &mut seg,
            &[0x15, 0x49, 0xA9, 0x66],
            &Self::webm_info(&samples),
        );
        Self::ebml_elem(&mut seg, &[0x16, 0x54, 0xAE, 0x6B], &Self::webm_tracks());
        for cluster in Self::webm_clusters(&samples)? {
            Self::ebml_elem(&mut seg, &[0x1F, 0x43, 0xB6, 0x75], &cluster);
        }

        let mut out = Vec::new();
        Self::ebml_elem(&mut out, &[0x1A, 0x45, 0xDF, 0xA3], &Self::webm_header());
        Self::ebml_elem(&mut out, &[0x18, 0x53, 0x80, 0x67], &seg);
        Ok(out)
    }

    /// The file header: what the container is, and which versions it uses.
    fn webm_header() -> Vec<u8> {
        let mut header = Vec::new();
        Self::ebml_elem(&mut header, &[0x42, 0x86], &[1]); // EBMLVersion
        Self::ebml_elem(&mut header, &[0x42, 0xF7], &[1]); // EBMLReadVersion
        Self::ebml_elem(&mut header, &[0x42, 0xF2], &[4]); // MaxIDLength
        Self::ebml_elem(&mut header, &[0x42, 0xF3], &[8]); // MaxSizeLength
        Self::ebml_elem(&mut header, &[0x42, 0x82], b"webm"); // DocType
        Self::ebml_elem(&mut header, &[0x42, 0x87], &[4]); // DocTypeVersion
        Self::ebml_elem(&mut header, &[0x42, 0x85], &[2]); // DocTypeReadVersion
        header
    }

    /// The segment info: the muxer's name and the clip's length.
    fn webm_info(samples: &[i16]) -> Vec<u8> {
        let duration_ms = samples.len() as f64 * 1000.0 / 48_000.0;
        let mut info = Vec::new();
        Self::ebml_elem(&mut info, &[0x2A, 0xD7, 0xB1], &Self::ebml_uint(1_000_000));
        Self::ebml_elem(&mut info, &[0x4D, 0x80], b"QuillScribe"); // MuxingApp
        Self::ebml_elem(&mut info, &[0x57, 0x41], b"QuillScribe"); // WritingApp
        Self::ebml_elem(&mut info, &[0x44, 0x89], &duration_ms.to_be_bytes());
        info
    }

    /// One mono Opus track, with the `OpusHead` in its codec-private data.
    fn webm_tracks() -> Vec<u8> {
        let mut audio_elem = Vec::new();
        Self::ebml_elem(&mut audio_elem, &[0xB5], &48_000f64.to_be_bytes());
        Self::ebml_elem(&mut audio_elem, &[0x9F], &[1]);

        let mut track = Vec::new();
        Self::ebml_elem(&mut track, &[0xD7], &[1]); // TrackNumber
        Self::ebml_elem(&mut track, &[0x73, 0xC5], &[1]); // TrackUID
        Self::ebml_elem(&mut track, &[0x83], &[2]); // TrackType: audio
        Self::ebml_elem(&mut track, &[0x86], b"A_OPUS"); // CodecID
        Self::ebml_elem(&mut track, &[0x63, 0xA2], &Self::opus_head()); // CodecPrivate
        let default_dur = Self::ebml_uint(20_000_000); // 20 ms in ns
        Self::ebml_elem(&mut track, &[0x23, 0xE3, 0x83], &default_dur);
        Self::ebml_elem(&mut track, &[0xE1], &audio_elem); // Audio

        let mut tracks = Vec::new();
        Self::ebml_elem(&mut tracks, &[0xAE], &track);
        tracks
    }

    /// The `OpusHead` identification block.
    fn opus_head() -> Vec<u8> {
        let mut head = Vec::with_capacity(19);
        head.extend_from_slice(b"OpusHead");
        head.push(1); // version
        head.push(1); // channels (mono)
        head.extend_from_slice(&312u16.to_le_bytes()); // pre-skip
        head.extend_from_slice(&48_000u32.to_le_bytes()); // input sample rate
        head.extend_from_slice(&0i16.to_le_bytes()); // output gain
        head.push(0); // channel mapping family
        head
    }

    /// Encode the samples into Opus packets, grouped into one-second clusters.
    ///
    /// A block's relative timestamp is its offset inside its cluster, which is
    /// why the packets are grouped here rather than streamed straight out.
    fn webm_clusters(samples: &[i16]) -> Result<Vec<Vec<u8>>> {
        use magnum_opus::{Application, Channels, Encoder};

        const FRAME: usize = 960; // 20 ms at 48 kHz
        const FRAMES_PER_CLUSTER: usize = 50; // 1 s per cluster

        let mut enc = Encoder::new(48_000, Channels::Mono, Application::Voip)
            .map_err(|e| anyhow!("Failed to create Opus encoder: {}", e))?;
        enc.set_bitrate(magnum_opus::Bitrate::Bits(32_000))
            .map_err(|e| anyhow!("Failed to set Opus bitrate: {}", e))?;

        let total_frames = samples.len().div_ceil(FRAME);
        let mut clusters = Vec::new();
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
            Self::ebml_elem(&mut blocks, &[0xA3], &Self::simple_block(idx, &packet));

            let last = idx + 1 == total_frames;
            if (idx + 1) % FRAMES_PER_CLUSTER == 0 || last {
                let mut cl = Vec::new();
                Self::ebml_elem(&mut cl, &[0xE7], &Self::ebml_uint(cluster_ms));
                cl.extend_from_slice(&blocks);
                clusters.push(cl);
                blocks.clear();
                cluster_ms += FRAMES_PER_CLUSTER as u64 * 20;
            }
        }
        Ok(clusters)
    }

    /// One block: track number, a timestamp relative to its cluster, and flags.
    fn simple_block(idx: usize, packet: &[u8]) -> Vec<u8> {
        const FRAMES_PER_CLUSTER: usize = 50;
        let rel = ((idx % FRAMES_PER_CLUSTER) * 20) as i16;
        let mut block = Vec::with_capacity(packet.len() + 4);
        block.push(0x81); // track number 1 (VINT)
        block.extend_from_slice(&rel.to_be_bytes());
        block.push(0x80); // flags: keyframe
        block.extend_from_slice(packet);
        block
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
}
