//! The transcription manager: settings, and the one entry point that picks a
//! provider.
//!
//! Each provider keeps its own file: `api.rs` for a hosted upload, `gemini.rs`
//! for the WebChannel transport, `local.rs` for whisper.cpp, and `deepgram.rs`
//! next door for the live session. The catalogue of models and languages is in
//! `catalog.rs`, so the dropdown can be built without any transport at all.

mod deepgram_languages;

use anyhow::{anyhow, Result};

use crate::config::DeepgramConfig;
use serde::{Deserialize, Serialize};

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
    /// Deepgram's own settings. It is the one provider with two execution
    /// shapes (recorded upload or live session), so it carries a whole config
    /// rather than a lone API key.
    deepgram: DeepgramConfig,
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
            deepgram: DeepgramConfig::default(),
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
            "openai" | "groq" | "gemini" | "deepgram" => {
                self.api_provider = provider.to_string();
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid API provider '{}'. Must be 'openai', 'groq', 'gemini', or 'deepgram'.",
                provider
            )),
        }
    }

    /// True when this provider is driven by a live session rather than a batch
    /// transcription. Deepgram is the only one.
    pub fn is_live_provider(provider: &str) -> bool {
        provider == "deepgram"
    }

    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    pub fn set_deepgram(&mut self, config: DeepgramConfig) {
        self.deepgram = config;
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
        match Self::match_language(&self.api_provider, lang) {
            Some(code) => {
                self.api_language = code;
                Ok(())
            }
            None => Err(anyhow!("Unsupported language '{}'.", lang)),
        }
    }

    /// Resolve anything a settings file or the UI might hold — a language code,
    /// a display label ("English"), or a legacy "code,Label" pair — onto a code
    /// this provider accepts, falling back to the provider's default.
    ///
    /// This exists because the UI used to offer labels the manager could never
    /// accept ("Auto-detect"), and the resulting error was discarded, so every
    /// provider silently ran English. Resolve before setting instead.
    pub fn resolve_language_or_default(provider: &str, raw: &str) -> String {
        Self::match_language(provider, raw).unwrap_or_else(|| {
            log::warn!(
                "Language '{}' is not supported by '{}'; using its default.",
                raw,
                provider
            );
            Self::default_language_for(provider)
        })
    }

    fn default_language_for(_provider: &str) -> String {
        "en".to_string()
    }

    /// Match a code or a display label against the provider's own language list.
    fn match_language(provider: &str, raw: &str) -> Option<String> {
        let raw = Self::extract_language_code(raw);
        let languages = Self::get_available_languages_for_provider(provider);

        if let Some((code, _)) = languages.iter().find(|(code, _)| code == raw) {
            return Some(code.clone());
        }
        languages
            .iter()
            .find(|(_, label)| label.eq_ignore_ascii_case(raw))
            .map(|(code, _)| code.clone())
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
}

mod api;
mod audio;
mod catalog;
mod encoding;
mod gemini;
mod local;
mod models;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
