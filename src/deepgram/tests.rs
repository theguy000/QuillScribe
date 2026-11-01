//! The Deepgram frames and sessions, checked against captured shapes.

use super::*;

fn cfg() -> DeepgramConfig {
    DeepgramConfig {
        api_key: "not-a-real-deepgram-key".to_string(),
        ..DeepgramConfig::default()
    }
}

// â”€â”€ URL building â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_build_listen_url_enables_the_normal_things() {
    let url = build_listen_url(&cfg());

    assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
    assert!(url.contains("model=nova-3"));
    assert!(url.contains("language=en"));
    assert!(url.contains("encoding=linear16"));
    assert!(url.contains("sample_rate=16000"));
    assert!(url.contains("channels=1"));
    assert!(url.contains("smart_format=true"));
    assert!(url.contains("punctuate=true"));
    // `dictation` is deliberately absent by default: it replaces automatic
    // punctuation, it does not add to it.
    assert!(!url.contains("dictation"), "URL was: {}", url);
    assert!(url.contains("interim_results=true"));
    assert!(url.contains("endpointing=300"));
    assert!(url.contains("utterance_end_ms=1000"));
}

#[test]
fn test_build_recorded_url_is_a_batch_request() {
    let url = build_recorded_url(&cfg());

    assert!(url.starts_with("https://api.deepgram.com/v1/listen?"));
    assert!(url.contains("model=nova-3"));
    assert!(url.contains("language=en"));
    assert!(url.contains("smart_format=true"));
    assert!(url.contains("punctuate=true"));
    // The audio is a complete WAV, so raw-PCM hints must be absentâ€¦
    assert!(!url.contains("encoding"), "URL was: {}", url);
    assert!(!url.contains("sample_rate"));
    assert!(!url.contains("channels"));
    // â€¦and so must live-only parameters.
    assert!(!url.contains("interim_results"));
    assert!(!url.contains("utterance_end_ms"));
    assert!(!url.contains("endpointing"));
    assert!(!url.contains("vad_events"));
}

#[test]
fn test_parse_recorded_transcript_extracts_the_text() {
    let body = r#"{"metadata":{"request_id":"x"},
        "results":{"channels":[{"alternatives":[{"transcript":" The quick brown fox. ",
        "confidence":0.99,"words":[]}]}]}}"#;
    assert_eq!(
        parse_recorded_transcript(body).unwrap(),
        "The quick brown fox."
    );
}

#[test]
fn test_parse_recorded_transcript_rejects_an_empty_result() {
    let body = r#"{"results":{"channels":[{"alternatives":[{"transcript":""}]}]}}"#;
    assert!(parse_recorded_transcript(body).is_err());
    assert!(parse_recorded_transcript("not json").is_err());
}

#[test]
fn test_build_listen_url_never_sends_diarization() {
    // Diarization was deliberately left out of this provider.
    let url = build_listen_url(&cfg());
    assert!(!url.contains("diarize"), "URL was: {}", url);
}

#[test]
fn test_build_listen_url_forces_punctuate_when_dictation_is_on() {
    let mut config = cfg();
    config.punctuate = false;
    config.dictation = true;

    let url = build_listen_url(&config);
    assert!(url.contains("punctuate=true"), "URL was: {}", url);
    assert!(url.contains("dictation=true"));
}

#[test]
fn test_build_listen_url_sends_dictation_only_when_enabled() {
    let mut config = cfg();
    config.dictation = true;
    assert!(build_listen_url(&config).contains("dictation=true"));

    config.dictation = false;
    assert!(!build_listen_url(&config).contains("dictation"));
}

#[test]
fn test_build_listen_url_omits_disabled_flags() {
    let mut config = cfg();
    config.smart_format = false;
    config.punctuate = false;
    config.dictation = false;
    config.interim_results = false;

    let url = build_listen_url(&config);
    assert!(!url.contains("smart_format"));
    assert!(!url.contains("punctuate"));
    assert!(!url.contains("dictation"));
    assert!(!url.contains("interim_results"));
    // utterance end only means something with interim results
    assert!(!url.contains("utterance_end_ms"));
    assert!(!url.contains("vad_events"));
}

#[test]
fn test_build_listen_url_clamps_utterance_end_to_the_supported_range() {
    let mut config = cfg();
    config.utterance_end_ms = 200; // below Deepgram's 1000 ms minimum

    let url = build_listen_url(&config);
    assert!(url.contains("utterance_end_ms=1000"), "URL was: {}", url);

    config.utterance_end_ms = 90_000; // above the 5000 ms maximum
    let url = build_listen_url(&config);
    assert!(url.contains("utterance_end_ms=5000"), "URL was: {}", url);
}

#[test]
fn test_build_listen_url_encodes_model_and_language() {
    let mut config = cfg();
    config.model = "nova-3 custom".to_string();
    config.language = "en-US".to_string();

    let url = build_listen_url(&config);
    assert!(url.contains("model=nova-3%20custom"), "URL was: {}", url);
    assert!(url.contains("language=en-US"));
}

// â”€â”€ Message parsing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_parse_interim_result() {
    // Note: no "type" field, exactly as Deepgram sends it.
    let raw = r#"{"channel_index":[0,1],"duration":1.039875,"start":0,"is_final":false,
        "channel":{"alternatives":[{"transcript":"another big","confidence":0.96,
        "words":[]}]}}"#;
    assert_eq!(
        parse_server_message(raw),
        ServerMessage::Interim("another big".to_string())
    );
}

#[test]
fn test_parse_final_result() {
    let raw = r#"{"type":"Results","is_final":true,"speech_final":true,
        "channel":{"alternatives":[{"transcript":" hello there. ","confidence":1.0}]}}"#;
    assert_eq!(
        parse_server_message(raw),
        ServerMessage::Final {
            text: "hello there.".to_string(),
            from_finalize: false,
        }
    );
}

#[test]
fn test_parse_final_from_finalize() {
    let raw = r#"{"type":"Results","is_final":true,"from_finalize":true,
        "channel":{"alternatives":[{"transcript":" tail words"}]}}"#;
    match parse_server_message(raw) {
        ServerMessage::Final {
            text,
            from_finalize,
        } => {
            assert_eq!(text, "tail words");
            assert!(from_finalize, "from_finalize must be preserved");
        }
        other => panic!("expected a final, got {:?}", other),
    }
}

#[test]
fn test_parse_speech_final_without_is_final_is_not_a_commit() {
    // Deepgram's own docs show is_final:false together with speech_final:true.
    let raw = r#"{"type":"Results","is_final":false,"speech_final":true,
        "channel":{"alternatives":[{"transcript":"half a thought"}]}}"#;
    assert_eq!(
        parse_server_message(raw),
        ServerMessage::Interim("half a thought".to_string())
    );
}

#[test]
fn test_parse_utterance_end_is_a_turn_boundary() {
    let raw = r#"{"type":"UtteranceEnd","channel":[0,1],"last_word_end":2.395}"#;
    assert_eq!(parse_server_message(raw), ServerMessage::TurnBoundary);
}

fn final_frame(text: &str) -> String {
    format!(
        r#"{{"type":"Results","is_final":true,
            "channel":{{"alternatives":[{{"transcript":"{}"}}]}}}}"#,
        text
    )
}

/// Collect the finals a sink is given, in order.
fn collecting_sink() -> (ProgressSink, Arc<std::sync::Mutex<Vec<String>>>) {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink: ProgressSink = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event: Progress| {
            if let Progress::Final(text) = event {
                seen.lock().unwrap().push(text);
            }
        })
    };
    (sink, seen)
}

#[test]
fn test_a_replayed_final_is_not_reported_twice() {
    let (sink, seen) = collecting_sink();
    let mut state = SessionState::new();

    apply_message(&final_frame("one"), &mut state, &sink);
    apply_message(&final_frame("two"), &mut state, &sink);

    // The socket dropped: the next connection re-transcribes the whole
    // utterance from the beginning and reports the same sentences again.
    state.finals.clear();
    apply_message(&final_frame("one"), &mut state, &sink);
    apply_message(&final_frame("two"), &mut state, &sink);

    assert_eq!(*seen.lock().unwrap(), ["one", "two"]);
    assert_eq!(state.transcript(), "one two");
}

#[test]
fn test_a_final_from_new_audio_after_a_replay_is_still_reported() {
    let (sink, seen) = collecting_sink();
    let mut state = SessionState::new();

    apply_message(&final_frame("one"), &mut state, &sink);
    apply_message(&final_frame("two"), &mut state, &sink);

    state.finals.clear();
    apply_message(&final_frame("one"), &mut state, &sink);
    // The audio spoken after the reconnect commits its own sentence.
    apply_message(&final_frame("three"), &mut state, &sink);

    assert_eq!(*seen.lock().unwrap(), ["one", "two", "three"]);
}

#[test]
fn test_a_blank_final_does_not_desync_the_replay_guard() {
    let (sink, seen) = collecting_sink();
    let mut state = SessionState::new();

    // Deepgram emits whitespace-only finals on silence.
    apply_message(&final_frame("one"), &mut state, &sink);
    apply_message(&final_frame("   "), &mut state, &sink);
    apply_message(&final_frame("two"), &mut state, &sink);

    state.finals.clear();
    apply_message(&final_frame("one"), &mut state, &sink);
    apply_message(&final_frame("   "), &mut state, &sink);
    apply_message(&final_frame("two"), &mut state, &sink);

    assert_eq!(*seen.lock().unwrap(), ["one", "two"]);
    assert_eq!(state.transcript(), "one two");
}

#[test]
fn test_parse_metadata() {
    let raw = r#"{"type":"Metadata","request_id":"abc","duration":0.0,"channels":0}"#;
    assert_eq!(parse_server_message(raw), ServerMessage::Metadata);
}

#[test]
fn test_parse_error_carries_a_message() {
    let raw = r#"{"type":"Error","description":"NET-0001"}"#;
    assert_eq!(
        parse_server_message(raw),
        ServerMessage::Error("NET-0001".to_string())
    );
}

#[test]
fn test_parse_garbage_is_ignored() {
    assert_eq!(parse_server_message("not json"), ServerMessage::Ignored);
    assert_eq!(
        parse_server_message("{}"),
        ServerMessage::Interim(String::new())
    );
}

// â”€â”€ Transcript assembly â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_session_state_joins_finals_once() {
    let mut state = SessionState::new();
    state.commit(" hello there. ");
    state.commit("");
    state.commit("how are you?");
    assert_eq!(state.transcript(), "hello there. how are you?");
}

#[test]
fn test_session_state_clears_finals_on_reconnect() {
    let mut state = SessionState::new();
    state.commit("first attempt");
    state.finals.clear(); // what run_connection does on reconnect
    state.commit("replayed properly");
    assert_eq!(state.transcript(), "replayed properly");
}

#[test]
fn test_session_state_accumulates_pcm_for_replay() {
    let mut state = SessionState::new();
    let chunk = vec![0.0f32; 100];
    state.push_samples(&chunk);
    state.push_samples(&chunk);
    assert_eq!(state.pcm.len(), 200);
}

// â”€â”€ Live network test â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// The key the app itself would use: `settings.json` first, because that is
/// where Settings â†’ Engine writes it, then an environment override.
fn live_api_key() -> Option<String> {
    let from_env = std::env::var("DEEPGRAM_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    if from_env.is_some() {
        return from_env;
    }

    let path = crate::config::ConfigManager::config_dir()
        .ok()?
        .join("settings.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get("deepgram")?
        .get("api_key")?
        .as_str()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// End-to-end check of the recorded (prerecorded) path.
///
/// ```text
/// cargo test --lib live_deepgram_recorded -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn live_deepgram_recorded_transcribes_synthesized_speech() {
    let _ = env_logger::builder().is_test(true).try_init();

    let Some(api_key) = live_api_key() else {
        eprintln!("skipping: no Deepgram key (Settings → Engine or DEEPGRAM_API_KEY)");
        return;
    };
    let fixture = std::path::Path::new("spike_speech.wav");
    if !fixture.exists() {
        eprintln!("skipping: spike_speech.wav is missing");
        return;
    }

    let reader = hound::WavReader::open(fixture).expect("open fixture");
    let spec = reader.spec();
    let samples: Vec<f32> = reader
        .into_samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / i16::MAX as f32)
        .collect();

    let config = DeepgramConfig {
        api_key,
        ..DeepgramConfig::default()
    };
    assert!(!config.is_live(), "recorded must be the default mode");

    let transcript = transcribe_recorded(&config, samples, spec.sample_rate)
        .await
        .expect("recorded transcription");
    println!("recorded transcript: {}", transcript);
    assert!(
        transcript.trim_end().ends_with('.'),
        "expected punctuation from the recorded endpoint, got {:?}",
        transcript
    );
}
/// End-to-end check against the real service.
///
/// The key is taken from the app's own settings first, so a key pasted into
/// Settings â†’ Engine â†’ Deepgram is enough; `DEEPGRAM_API_KEY` is only an
/// override for CI. Also needs the gitignored `spike_speech.wav` fixture.
///
/// ```text
/// cargo test --lib live_deepgram -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn live_deepgram_transcribes_synthesized_speech() {
    // `RUST_LOG=debug` dumps every raw frame, which is how you tell
    // "Deepgram didn't punctuate" apart from "we dropped the punctuation".
    let _ = env_logger::builder().is_test(true).try_init();

    let Some(api_key) = live_api_key() else {
        eprintln!(
            "skipping: no Deepgram key. Paste one into Settings â†’ Engine â†’ Deepgram \
             (written to settings.json) or set DEEPGRAM_API_KEY."
        );
        return;
    };
    let fixture = std::path::Path::new("spike_speech.wav");
    if !fixture.exists() {
        eprintln!("skipping: spike_speech.wav is missing");
        return;
    }

    let reader = hound::WavReader::open(fixture).expect("open fixture");
    let spec = reader.spec();
    let samples: Vec<f32> = reader
        .into_samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / i16::MAX as f32)
        .collect();
    if samples.is_empty() {
        eprintln!("skipping: fixture contains no samples");
        return;
    }

    let config = DeepgramConfig {
        api_key,
        ..DeepgramConfig::default()
    };
    let (audio_tx, audio_rx) = mpsc::unbounded_channel();
    let progress: ProgressSink = Arc::new(|event: Progress| {
        if let Progress::Interim(text) = event {
            println!("interim: {}", text);
        }
    });

    let session = LiveSession::start(
        tokio::runtime::Handle::current(),
        config,
        audio_rx,
        progress,
    );

    // Feed the clip at roughly realtime: Deepgram accepts at most 1.25x.
    let block = (spec.sample_rate as usize) / 10;
    for chunk in samples.chunks(block.max(1)) {
        audio_tx
            .send(AudioChunk {
                samples: chunk.to_vec(),
                sample_rate: spec.sample_rate,
            })
            .expect("session alive");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    drop(audio_tx);

    let transcript = session.finish().await.expect("live transcription");
    println!("transcript: {}", transcript);
    assert!(
        transcript.split_whitespace().count() >= 2,
        "expected a real transcript, got {:?}",
        transcript
    );
    // Regression guard for the measured `dictation` behaviour: turning spoken
    // punctuation on silently removes automatic punctuation, so if this ever
    // comes back unpunctuated the defaults have drifted.
    assert!(
        transcript.trim_end().ends_with('.'),
        "expected Deepgram to punctuate the sentence, got {:?}",
        transcript
    );
}
