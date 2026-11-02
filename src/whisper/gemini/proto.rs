//! Reading Gemini's protobuf frames: which base64 field holds the words.
//!
//! The WebChannel envelope wraps a length-delimited protobuf, and the
//! transcript is nested three fields deep. Walking the wire format is the only
//! reliable way to find it: a scan for printable bytes swallows the length
//! prefix as leading punctuation.

use super::super::WhisperManager;

impl WhisperManager {
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
        msg.split('"')
            .filter(|cap| cap.len() >= 12)
            .filter_map(|cap| engine.decode(cap.as_bytes()).ok())
            .flat_map(|raw| Self::extract_proto_strings(&raw))
            .rfind(|text| is_transcript_text(text))
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
        while i < bytes.len() {
            let Some((tag, next)) = read_varint(bytes, i) else {
                break;
            };
            i = next;
            match field_kind(tag) {
                Field::Bytes => {
                    let Some((len, next)) = read_varint(bytes, i) else {
                        break;
                    };
                    let Some(end) = next.checked_add(len as usize).filter(|e| *e <= bytes.len())
                    else {
                        break;
                    };
                    push_or_descend(&bytes[next..end], &mut out);
                    i = end;
                }
                Field::Varint => {
                    let Some((_, next)) = read_varint(bytes, i) else {
                        break;
                    };
                    i = next;
                }
                Field::Fixed64 => i += 8,
                Field::Fixed32 => i += 4,
                Field::Unknown => break,
            }
        }
        out
    }
}

/// One field's payload shape, as its wire type says.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Varint,
    Fixed64,
    Bytes,
    Fixed32,
    /// A wire type protobuf does not use: the rest of the blob is noise.
    Unknown,
}

fn field_kind(tag: u64) -> Field {
    match tag & 0x07 {
        0 => Field::Varint,
        1 => Field::Fixed64,
        2 => Field::Bytes,
        5 => Field::Fixed32,
        _ => Field::Unknown,
    }
}

/// Reads a base-128 varint, returning (value, next_index).
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

/// Transcript text is printable text. A slice containing control bytes is a
/// nested protobuf message (its first byte is a field tag), so descend into it
/// rather than emitting it.
fn push_or_descend(slice: &[u8], out: &mut Vec<String>) {
    match std::str::from_utf8(slice) {
        Ok(text) if !text.is_empty() && !text.chars().any(char::is_control) => {
            out.push(text.to_string())
        }
        _ => out.extend(WhisperManager::extract_proto_strings(slice)),
    }
}

/// Whether a decoded string is words rather than an envelope field.
///
/// The blob also carries the recognizer's own names and the locale, which are
/// printable and would otherwise be taken for speech.
fn is_transcript_text(text: &str) -> bool {
    text.len() >= 2
        && !text.contains("recognizer")
        && !text.contains("frontend")
        && text != "Web"
        && text != "en-US"
}
