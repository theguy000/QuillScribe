//! The encodings the Gemini WebChannel needs: URL, base64, and a session id.
//!
//! Small and dependency-free on purpose: these are the few pieces of the wire
//! format that do not need the request itself, so they can be read and tested
//! without the transport around them.

use super::WhisperManager;
use anyhow::{anyhow, Result};

impl WhisperManager {
    pub fn url_encode(input: &str) -> String {
        let mut encoded = String::with_capacity(input.len());
        for byte in input.bytes() {
            match unescaped_url_char(byte) {
                Some(c) => encoded.push(c),
                None => encoded.push_str(&format!("%{:02X}", byte)),
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
        body.lines()
            .filter_map(webchannel_frames)
            .flatten()
            .find_map(channel_sid)
            .ok_or_else(|| anyhow!("Failed to find WebChannel SID in response body: {}", body))
    }

    #[allow(dead_code)]
    pub fn build_recognizer_init_payload(lang: &str) -> Vec<u8> {
        let lang_code = recognizer_language(lang);
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
        let mut latest = None;
        for payload in chunk.lines().filter_map(webchannel_frames).flatten() {
            if let Some(text) = Self::transcript_of(&payload) {
                latest = Some(text);
            }
        }
        latest
    }

    /// The transcript one frame payload carries, if it carries one.
    #[allow(dead_code)]
    fn transcript_of(payload: &serde_json::Value) -> Option<String> {
        let bytes = Self::base64_decode(payload.as_str()?).ok()?;
        let text = Self::extract_text_from_proto(&bytes)?;
        (!text.trim().is_empty()).then_some(text)
    }

    #[allow(dead_code)]
    fn extract_text_from_proto(bytes: &[u8]) -> Option<String> {
        let mut candidate = None;
        for field in ProtoFields::new(bytes) {
            if let Some(text) = proto_text(field) {
                candidate = Some(text);
            }
            if let Some(nested) = Self::extract_text_from_proto(field) {
                candidate = Some(nested);
            }
        }
        candidate
    }
}

/// The strings one `[[...]]` line lists, or `None` when it is not a frame.
///
/// A frame is `[n, payload]`. `n` is the frame's own length in characters, so
/// it says nothing about the payload. The handshake's payload names a channel
/// and holds numbers beside its strings; a transcript frame lists base64
/// messages and nothing else.
fn webchannel_frames(line: &str) -> Option<Vec<serde_json::Value>> {
    let trimmed = line.trim();
    if !(trimmed.starts_with("[[") && trimmed.ends_with("]]")) {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    frames_of(&parsed)
}

/// The payloads a parsed frame list holds.
fn frames_of(parsed: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    let frames = parsed.as_array()?;
    let mut out = Vec::new();
    for frame in frames.iter() {
        let pair = frame.as_array()?;
        let payload = pair.get(1)?.as_array()?;
        out.extend(frame_payloads(payload)?);
    }
    Some(out)
}

/// The payloads one frame carries.
///
/// A frame of base64 messages carries each message on its own. A frame that
/// names a channel carries the list itself, numbers and all.
fn frame_payloads(payload: &[serde_json::Value]) -> Option<Vec<serde_json::Value>> {
    let messages: Vec<&str> = payload.iter().filter_map(|value| value.as_str()).collect();
    let all_messages = messages.len() == payload.len() && !messages.is_empty();
    match all_messages {
        true => Some(messages.into_iter().map(text_value).collect()),
        false => Some(vec![serde_json::Value::Array(payload.to_vec())]),
    }
}

/// A payload string as the value the decoder reads.
fn text_value(text: &str) -> serde_json::Value {
    serde_json::Value::String(text.to_string())
}

/// The `"c"` channel id inside a frame, when this value is one.
fn channel_sid(payload: serde_json::Value) -> Option<String> {
    let values = payload.as_array()?;
    let (tag, sid) = (values.first()?.as_str()?, values.get(1)?.as_str()?);
    (tag == "c").then(|| sid.to_string())
}

/// The full locale a short code means, since the recognizer wants one.
fn recognizer_language(lang: &str) -> &str {
    match lang {
        "" => "en-US",
        "en" => "en-US",
        "es" => "es-ES",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "it" => "it-IT",
        "ja" => "ja-JP",
        "zh" => "zh-CN",
        _ => "en-US",
    }
}

/// Whether a decoded field is transcript text rather than an envelope name.
fn proto_text(slice: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(slice).ok()?;
    text_is_transcript(text).then(|| text.to_string())
}

/// The test itself: readable letters, and not one of the envelope names.
fn text_is_transcript(text: &str) -> bool {
    readable(text) && text.chars().any(char::is_alphabetic) && !envelope_name(text)
}

/// A string with no control characters other than a newline or a space.
fn readable(text: &str) -> bool {
    text.chars()
        .all(|c| !c.is_control() || c == ' ' || c == '\n')
}

/// The fixed names the recognizer envelope carries, in any order.
fn envelope_name(text: &str) -> bool {
    const NAMES: [&str; 4] = ["recognizer", "frontend", "Web", "en-US"];
    NAMES.iter().any(|name| text.contains(name))
}

/// A protobuf blob walked one field at a time.
///
/// A step yields the bytes of a length-delimited field, which is where the
/// transcript lives. Varints and fixed-width values are stepped over.
struct ProtoFields<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> ProtoFields<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// The value of the base-128 varint at `self.at`, advancing past it.
    fn varint(&mut self) -> Option<usize> {
        let mut value = 0usize;
        let mut shift = 0u32;
        while let Some(&byte) = self.bytes.get(self.at) {
            self.at += 1;
            value |= usize::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
            shift += 7;
            if shift > 35 {
                return None;
            }
        }
        None
    }

    /// The tag byte just consumed, or `None` at the end of the blob.
    fn tag(&mut self) -> Option<u8> {
        let tag = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(tag)
    }

    /// The bytes of the length-delimited field whose length follows.
    ///
    /// A field whose declared length runs past the end of the blob is not
    /// part of this message: responses are cut off mid-message, and the
    /// numbers at the cut belong to the outer envelope. Such a field is
    /// skipped, leaving the walk to read the rest of the blob in place.
    fn length_delimited(&mut self) -> Option<&'a [u8]> {
        let len = self.varint()?;
        let end = self.at.saturating_add(len);
        if end > self.bytes.len() {
            return None;
        }
        let slice = &self.bytes[self.at..end];
        self.at = end;
        (!slice.is_empty()).then_some(slice)
    }

    /// Advance past a varint value, however many bytes it takes.
    fn skip_varint(&mut self) {
        while self.bytes.get(self.at).is_some_and(|b| b & 0x80 != 0) {
            self.at += 1;
        }
        self.at = self.at.saturating_add(1);
    }

    /// Advance past a fixed-width value of `width` bytes.
    fn skip_fixed(&mut self, width: usize) {
        self.at = self.at.saturating_add(width);
    }
}

impl<'a> Iterator for ProtoFields<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        loop {
            match self.tag()? & 0x07 {
                // A field that does not fit inside the blob is skipped, and
                // the walk carries on from the bytes it left behind.
                2 => {
                    if let Some(field) = self.length_delimited() {
                        return Some(field);
                    }
                }
                0 => self.skip_varint(),
                1 => self.skip_fixed(8),
                5 => self.skip_fixed(4),
                _ => self.skip_fixed(1),
            }
        }
    }
}

/// The characters RFC 3986 leaves alone in a query value, as themselves.
fn unescaped_url_char(byte: u8) -> Option<char> {
    const ALWAYS_SAFE: &[u8] = b"-_.~";
    let safe = byte.is_ascii_alphanumeric() || ALWAYS_SAFE.contains(&byte);
    safe.then_some(byte as char)
}
