//! The encodings the Gemini WebChannel needs: URL, base64, and a session id.
//!
//! Small and dependency-free on purpose: these are the few pieces of the wire
//! format that do not need the request itself, so they can be read and tested
//! without the transport around them.

use super::WhisperManager;
use anyhow::{anyhow, Result};

impl WhisperManager {
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
}
