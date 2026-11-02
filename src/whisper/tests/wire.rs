//! The pure encoding helpers, and what they must produce on the wire.

use crate::whisper::*;
use std::fs;

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
    let key = WhisperManager::gemini_website_key();
    assert!(
        key.starts_with("AIza"),
        "the website key is a Google browser key"
    );
    assert_eq!(key.len(), 39, "it is the key the web client ships, whole");
}

#[test]
fn test_set_api_provider_accepts_valid_providers() {
    let mut manager = WhisperManager::new();
    assert!(manager.set_api_provider("openai").is_ok());
    assert!(manager.set_api_provider("groq").is_ok());
    assert!(manager.set_api_provider("gemini").is_ok());
    assert!(manager.set_api_provider("deepgram").is_ok());
}

#[test]
fn test_deepgram_is_the_only_live_provider() {
    assert!(WhisperManager::is_live_provider("deepgram"));
    assert!(!WhisperManager::is_live_provider("openai"));
    assert!(!WhisperManager::is_live_provider("groq"));
    assert!(!WhisperManager::is_live_provider("gemini"));
}

#[test]
fn test_get_available_api_models_for_provider_deepgram() {
    let models = WhisperManager::get_available_api_models_for_provider("deepgram");
    assert_eq!(models.first().map(String::as_str), Some("nova-3"));
    assert!(models.contains(&"nova-2".to_string()));
    assert!(!models.iter().any(|m| m.starts_with("whisper")));
}

#[test]
fn test_deepgram_accepts_the_multilingual_option() {
    // `multi` is Deepgram-only; it is not in the shared OpenAI code list.
    assert!(WhisperManager::get_available_languages()
        .iter()
        .all(|(code, _)| code != "multi"));

    let mut manager = WhisperManager::new();
    manager.set_api_provider("deepgram").unwrap();
    assert!(manager.set_api_language("multi").is_ok());
}

#[test]
fn test_resolve_language_accepts_a_display_label() {
    assert_eq!(
        WhisperManager::resolve_language_or_default("openai", "Spanish"),
        "es"
    );
    assert_eq!(
        WhisperManager::resolve_language_or_default("deepgram", "English"),
        "en"
    );
}

#[test]
fn test_resolve_language_repairs_the_legacy_auto_detect_value() {
    // settings.json used to hold the dropdown's label, which the manager
    // rejected with an error that was then discarded. It must now resolve.
    assert_eq!(
        WhisperManager::resolve_language_or_default("openai", "Auto-detect"),
        "en"
    );
    assert_eq!(
        WhisperManager::resolve_language_or_default("deepgram", ""),
        "en"
    );
}

#[test]
fn test_language_label_for_round_trips_a_stored_code() {
    let labels = WhisperManager::language_labels_for_provider("deepgram");
    assert!(labels.contains(&"Multilingual (code-switching)".to_string()));
    assert_eq!(
        WhisperManager::language_label_for("deepgram", "multi"),
        "Multilingual (code-switching)"
    );
    assert_eq!(
        WhisperManager::language_label_for("deepgram", "fr"),
        "French"
    );
}

#[test]
fn test_deepgram_language_catalogue_is_comprehensive() {
    let languages = WhisperManager::get_available_deepgram_languages();
    assert!(
        languages.len() >= 100,
        "Deepgram publishes far more than {} languages",
        languages.len()
    );

    // Codes that only appear in the Nova-3 multilingual list.
    for code in ["multi", "xh", "zu", "gl", "en-CA"] {
        assert!(
            languages.iter().any(|(c, _)| c == code),
            "missing Deepgram language '{}'",
            code
        );
    }
    // Regional variants, including the script-tagged Chinese ones.
    for code in ["en-GB", "en-US", "zh-HK", "zh-Hans", "zh-Hant", "es-419"] {
        assert!(
            languages.iter().any(|(c, _)| c == code),
            "missing Deepgram regional language '{}'",
            code
        );
    }
}

#[test]
fn test_deepgram_language_labels_are_unique() {
    // The dropdown selects by label string, so a duplicate label would make
    // one of the two codes unreachable.
    let mut labels: Vec<String> = WhisperManager::get_available_deepgram_languages()
        .into_iter()
        .map(|(_, label)| label)
        .collect();
    let total = labels.len();
    labels.sort();
    labels.dedup();
    assert_eq!(total, labels.len(), "duplicate language labels");
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
