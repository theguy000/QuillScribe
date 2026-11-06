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

/// The live session against the real endpoint, fed at the pace a microphone
/// delivers audio. Ignored by default:
///
///     cargo test --lib live_gemini_stream -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn live_gemini_stream_transcribes_synthesized_speech() {
    use crate::audio::AudioChunk;
    use crate::live_provider::ProgressSink;
    use crate::whisper::gemini::live::LiveSession;
    use std::sync::Arc;

    let Ok(wav) = speech_clip() else {
        eprintln!("skipping: spike_speech.wav not present");
        return;
    };

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let progress: ProgressSink = Arc::new(|_| {});
    let session = LiveSession::start(
        tokio::runtime::Handle::current(),
        live_gemini_manager(),
        rx,
        progress,
    );

    // 100 ms blocks, the way the recorder delivers them, so the session sees the
    // shape it sees during a real utterance.
    for block in wav.chunks(1_600) {
        let _ = tx.send(AudioChunk {
            samples: block.to_vec(),
            sample_rate: 16_000,
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    drop(tx);

    let text = session.finish().await.unwrap();
    println!("LIVE STREAM TRANSCRIPT: {:?}", text);

    let lower = text.to_lowercase();
    assert!(!text.trim().is_empty(), "transcript must not be empty");
    assert!(
        lower.contains("quick") && lower.contains("fox"),
        "expected the spoken phrase, got: {text:?}"
    );
}

/// What the live session costs after the user stops, next to what the batch
/// shape costs for the same audio. This is the measurement the live session
/// exists for: the batch shape only starts working once the user stops, so its
/// cost grows with the utterance. Ignored, and about two minutes long.
///
///     cargo test --lib timing_live_vs_batch -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn timing_live_vs_batch() {
    use crate::audio::AudioChunk;
    use crate::live_provider::ProgressSink;
    use crate::whisper::gemini::live::LiveSession;
    use std::sync::Arc;

    let Ok(clip) = speech_clip() else {
        eprintln!("skipping: spike_speech.wav not present");
        return;
    };
    let mut audio = Vec::new();
    while audio.len() < 60 * 16_000 {
        audio.extend_from_slice(&clip);
    }
    let seconds = audio.len() as f32 / 16_000.0;

    // What the app did before the live session existed, and still does when one
    // fails.
    let mut manager = live_gemini_manager();
    manager.set_api_language("en").unwrap();
    let start = std::time::Instant::now();
    let batch_text = manager
        .transcribe_audio(audio.clone(), 16_000)
        .await
        .unwrap();
    let batch = start.elapsed();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let progress: ProgressSink = Arc::new(|_| {});
    let session = LiveSession::start(
        tokio::runtime::Handle::current(),
        live_gemini_manager(),
        rx,
        progress,
    );
    for block in audio.chunks(1_600) {
        let _ = tx.send(AudioChunk {
            samples: block.to_vec(),
            sample_rate: 16_000,
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let stop = std::time::Instant::now();
    drop(tx);
    let live_text = session.finish().await.unwrap();
    let live = stop.elapsed();

    eprintln!(
        "audio={:.1}s  batch_after_stop={:?}  live_after_stop={:?}  batch_words={}  live_words={}",
        seconds,
        batch,
        live,
        batch_text.split_whitespace().count(),
        live_text.split_whitespace().count(),
    );
    assert!(!live_text.trim().is_empty());
}

/// A manager pointed at Gemini with no user key, so the session falls back to
/// the one the Gemini web app ships.
fn live_gemini_manager() -> WhisperManager {
    let mut manager = WhisperManager::new();
    manager.set_api_provider("gemini").unwrap();
    manager.set_api_language("en").unwrap();
    manager
}

/// The speech fixture, decoded from 16-bit mono PCM out of its WAV container.
fn speech_clip() -> std::io::Result<Vec<f32>> {
    let wav = std::fs::read("spike_speech.wav")?;
    Ok(wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect())
}
