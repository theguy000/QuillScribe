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
#[path = "whisper/tests.rs"]
mod tests;
