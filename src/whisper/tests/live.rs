//! Runs against the real Gemini endpoint, so both are ignored by default.
//!
//! ```text
//! cargo test --lib live_gemini timing_breakdown -- --ignored --nocapture
//! ```

use crate::whisper::*;

/// Latency breakdown: local encode vs. network round-trip.
#[tokio::test]
#[ignore]
async fn timing_breakdown() {
    let path = std::env::var("QUILL_TEST_WAV").unwrap_or_else(|_| "spike_speech.wav".to_string());
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
