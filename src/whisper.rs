use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use hound::{SampleFormat, WavSpec, WavWriter};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

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
    api_model: String,
    api_language: String,
    local_model: String,
    local_model_path: String,
    local_language: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl WhisperManager {
    pub fn new() -> Self {
        Self {
            mode: "api".to_string(),
            api_provider: "openai".to_string(),
            api_key: String::new(),
            groq_api_key: String::new(),
            api_model: "gpt-4o-transcribe".to_string(),
            api_language: "en".to_string(),
            local_model: "base".to_string(),
            local_model_path: String::new(),
            local_language: "en".to_string(),
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
            "openai" | "groq" => {
                self.api_provider = provider.to_string();
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid API provider '{}'. Must be 'openai' or 'groq'.",
                provider
            )),
        }
    }

    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    pub fn set_groq_api_key(&mut self, key: &str) {
        self.groq_api_key = key.to_string();
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
        let lang = Self::extract_language_code(lang);
        if Self::is_valid_language(lang) {
            self.api_language = lang.to_string();
            Ok(())
        } else {
            Err(anyhow!("Unsupported language code '{}'.", lang))
        }
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

    // ── API transcription ────────────────────────────────────────────────

    fn provider_label(provider: &str) -> &'static str {
        match provider {
            "groq" => "Groq",
            _ => "OpenAI",
        }
    }

    fn api_url(provider: &str) -> &'static str {
        match provider {
            "groq" => "https://api.groq.com/openai/v1/audio/transcriptions",
            _ => "https://api.openai.com/v1/audio/transcriptions",
        }
    }

    async fn transcribe_api(&self, audio_data: Vec<f32>, sample_rate: u32) -> Result<String> {
        let active_key = if self.api_provider == "groq" {
            &self.groq_api_key
        } else {
            &self.api_key
        };

        if active_key.is_empty() {
            return Err(anyhow!(
                "API key is not set for {}.",
                Self::provider_label(&self.api_provider),
            ));
        }

        let wav_bytes = Self::encode_wav(&audio_data, sample_rate)?;

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.api_model.clone());

        if !self.api_language.is_empty() {
            form = form.text("language", self.api_language.clone());
        }

        // Groq API uses additional parameters
        if self.api_provider == "groq" {
            form = form.text("temperature", "0".to_string());
            form = form.text("response_format", "json".to_string());
        }

        let api_url = Self::api_url(&self.api_provider);
        let provider_label = Self::provider_label(&self.api_provider);

        let client = reqwest::Client::new();
        let response = client
            .post(api_url)
            .bearer_auth(active_key)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "{} API request failed with status {}: {}",
                provider_label,
                status,
                body
            ));
        }

        let transcription: TranscriptionResponse = response.json().await?;
        Ok(transcription.text)
    }

    // ── Local transcription (whisper.cpp via whisper-rs) ─────────────────

    async fn transcribe_local(&self, audio_data: Vec<f32>, sample_rate: u32) -> Result<String> {
        // Resolve the model file path.
        let model_path = self.resolve_model_path()?;

        if !model_path.exists() {
            return Err(anyhow!(
                "Model file not found at '{}'. Please download the model first.",
                model_path.display()
            ));
        }

        let language = self.local_language.clone();
        let model_path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow!("Model path contains invalid UTF-8"))?
            .to_string();

        // Resample to 16 kHz if the source sample rate differs.
        let audio_16k = if sample_rate != 16_000 {
            Self::resample(&audio_data, sample_rate, 16_000)
        } else {
            audio_data
        };

        // Run whisper inference on a blocking thread (CPU-bound work).
        let result = tokio::task::spawn_blocking(move || -> Result<String> {
            let ctx = WhisperContext::new_with_params(
                &model_path_str,
                WhisperContextParameters::default(),
            )
            .map_err(|e| anyhow!("Failed to load Whisper model: {}", e))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            // Set language (if not "auto", set it explicitly).
            if language != "auto" {
                params.set_language(Some(&language));
            }

            // Disable console output from whisper.cpp.
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            // Do not translate — we want transcription only.
            params.set_translate(false);

            // Use a single processing thread to avoid contention.
            params.set_n_threads(
                std::thread::available_parallelism()
                    .map(|n| n.get() as i32)
                    .unwrap_or(4)
                    .min(8),
            );

            let mut state = ctx
                .create_state()
                .map_err(|e| anyhow!("Failed to create Whisper state: {}", e))?;

            state
                .full(params, &audio_16k)
                .map_err(|e| anyhow!("Whisper inference failed: {}", e))?;

            let num_segments = state.full_n_segments();

            let mut text_parts = Vec::new();
            for i in 0..num_segments {
                if let Some(segment) = state.get_segment(i) {
                    if let Ok(text) = segment.to_str() {
                        text_parts.push(text.to_string());
                    } else if let Ok(text) = segment.to_str_lossy() {
                        text_parts.push(text.to_string());
                    }
                }
            }

            let full_text = text_parts.join("").trim().to_string();
            Ok(full_text)
        })
        .await
        .map_err(|e| anyhow!("Whisper task panicked: {}", e))??;

        Ok(result)
    }

    // ── Model management ─────────────────────────────────────────────────

    /// Resolve the model file path, preferring a custom path if set.
    fn resolve_model_path(&self) -> Result<PathBuf> {
        if !self.local_model_path.is_empty() {
            let p = PathBuf::from(&self.local_model_path);
            if p.exists() {
                return Ok(p);
            }
        }
        Ok(Self::get_model_path(&self.local_model))
    }

    /// Return the default cache path for a model:
    /// `~/.config/quillscribe/models/ggml-{model_name}.bin`
    pub fn get_model_path(model_name: &str) -> PathBuf {
        Self::models_dir().join(format!("ggml-{}.bin", model_name))
    }

    /// Return the models directory: `~/.config/quillscribe/models/`
    pub fn models_dir() -> PathBuf {
        let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config.join("quillscribe").join("models")
    }

    /// Check whether the model file exists on disk.
    pub fn is_model_downloaded(model_name: &str) -> bool {
        Self::get_model_path(model_name).exists()
    }

    /// List all model names that have been downloaded (by scanning the models dir).
    pub fn get_downloaded_models() -> Vec<String> {
        let dir = Self::models_dir();
        if !dir.exists() {
            return Vec::new();
        }

        let mut models = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("ggml-") && name.ends_with(".bin") {
                    // Extract model name: "ggml-base.bin" -> "base"
                    let model = name
                        .strip_prefix("ggml-")
                        .and_then(|s| s.strip_suffix(".bin"))
                        .unwrap_or("")
                        .to_string();
                    if !model.is_empty() {
                        models.push(model);
                    }
                }
            }
        }
        models
    }

    /// Delete a downloaded model file.
    pub fn delete_model(model_name: &str) -> Result<()> {
        let path = Self::get_model_path(model_name);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| anyhow!("Failed to delete model '{}': {}", model_name, e))?;
        }
        Ok(())
    }

    /// Download a GGML model from Hugging Face with progress reporting.
    ///
    /// URL pattern:
    /// `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`
    ///
    /// Returns the path to the downloaded file.
    pub async fn download_model_with_progress<F>(
        model_name: &str,
        mut progress_callback: F,
    ) -> Result<String>
    where
        F: FnMut(u64, u64) + Send + 'static,
    {
        let url = Self::model_download_url(model_name);
        let dest = Self::get_model_path(model_name);

        // Ensure models directory exists.
        let dir = Self::models_dir();
        fs::create_dir_all(&dir).map_err(|e| {
            anyhow!(
                "Failed to create models directory '{}': {}",
                dir.display(),
                e
            )
        })?;

        // Download with streaming and progress.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;

        let response = client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download model '{}': HTTP {}",
                model_name,
                response.status()
            ));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;

        let mut file = File::create(&dest.with_extension("bin.tmp"))
            .await
            .map_err(|e| anyhow!("Failed to create file: {}", e))?;

        let mut stream = response.bytes_stream();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| anyhow!("Download error: {}", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| anyhow!("Write error: {}", e))?;
            downloaded += chunk.len() as u64;
            progress_callback(downloaded, total_size);
        }

        // Rename to final path.
        let tmp_path = dest.with_extension("bin.tmp");
        tokio::fs::rename(&tmp_path, &dest)
            .await
            .map_err(|e| anyhow!("Failed to rename model file: {}", e))?;

        Ok(dest.to_string_lossy().to_string())
    }

    /// Download a GGML model from Hugging Face (without progress reporting).
    ///
    /// URL pattern:
    /// `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`
    ///
    /// Returns the path to the downloaded file.
    pub async fn download_model(model_name: &str) -> Result<String> {
        Self::download_model_with_progress(model_name, |_downloaded, _total| {}).await
    }

    /// Construct the Hugging Face download URL for a model.
    fn model_download_url(model_name: &str) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
            model_name
        )
    }

    // ── Audio utilities ──────────────────────────────────────────────────

    fn encode_wav(audio_data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for &sample in audio_data {
                let clamped = sample.clamp(-1.0, 1.0);
                let int_sample = (clamped * i16::MAX as f32) as i16;
                writer.write_sample(int_sample)?;
            }
            writer.finalize()?;
        }

        Ok(cursor.into_inner())
    }

    /// Simple linear resampling from `from_rate` to `to_rate`.
    fn resample(data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate || data.is_empty() {
            return data.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (data.len() as f64 / ratio).ceil() as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(data.len() - 1);
            let frac = (src_idx - idx_floor as f64) as f32;

            let sample = data[idx_floor] * (1.0 - frac) + data[idx_ceil] * frac;
            resampled.push(sample);
        }

        resampled
    }

    // ── Model catalog ────────────────────────────────────────────────────

    pub fn get_available_api_models() -> Vec<String> {
        vec![
            "gpt-4o-transcribe".to_string(),
            "gpt-4o-mini-transcribe".to_string(),
        ]
    }

    pub fn get_available_groq_models() -> Vec<String> {
        vec![
            "whisper-large-v3-turbo".to_string(),
            "whisper-large-v3".to_string(),
        ]
    }

    pub fn get_available_api_models_for_provider(provider: &str) -> Vec<String> {
        match provider {
            "groq" => Self::get_available_groq_models(),
            _ => Self::get_available_api_models(),
        }
    }

    pub fn get_available_local_models() -> Vec<String> {
        vec![
            "tiny".to_string(),
            "tiny.en".to_string(),
            "base".to_string(),
            "base.en".to_string(),
            "small".to_string(),
            "small.en".to_string(),
            "medium".to_string(),
            "medium.en".to_string(),
            "large-v1".to_string(),
            "large-v2".to_string(),
            "large-v3".to_string(),
            "turbo".to_string(),
            "distil-large-v2".to_string(),
            "distil-large-v3".to_string(),
            "distil-medium.en".to_string(),
            "distil-small.en".to_string(),
        ]
    }

    pub fn get_local_model_categories() -> Vec<String> {
        vec![
            "General".to_string(),
            "Tiny".to_string(),
            "Base".to_string(),
            "Small".to_string(),
            "Medium".to_string(),
            "Large".to_string(),
        ]
    }

    pub fn get_local_models_for_category(category: &str) -> Vec<String> {
        let all_models = Self::get_available_local_models();
        match category {
            "General" => all_models,
            "Tiny" => all_models
                .into_iter()
                .filter(|m| m.starts_with("tiny"))
                .collect(),
            "Base" => all_models
                .into_iter()
                .filter(|m| m.starts_with("base"))
                .collect(),
            "Small" => all_models
                .into_iter()
                .filter(|m| m.starts_with("small"))
                .collect(),
            "Medium" => all_models
                .into_iter()
                .filter(|m| m.starts_with("medium") || m.starts_with("distil-medium"))
                .collect(),
            "Large" => all_models
                .into_iter()
                .filter(|m| m.starts_with("large") || m == "turbo" || m.starts_with("distil-large"))
                .collect(),
            _ => all_models,
        }
    }

    pub fn get_model_info(model_name: &str) -> ModelInfo {
        match model_name {
            "tiny" | "tiny.en" => ModelInfo::new("75 MB", "~390 MB", "~10x", "Low"),
            "base" | "base.en" => ModelInfo::new("142 MB", "~500 MB", "~7x", "Low-Medium"),
            "small" | "small.en" => ModelInfo::new("466 MB", "~1 GB", "~4x", "Medium"),
            "medium" | "medium.en" => ModelInfo::new("1.5 GB", "~2.6 GB", "~2x", "Medium-High"),
            "large-v1" | "large-v2" | "large-v3" => {
                ModelInfo::new("2.9 GB", "~4.7 GB", "~1x", "High")
            }
            "turbo" => ModelInfo::new("1.5 GB", "~2.6 GB", "~8x", "High"),
            "distil-large-v2" | "distil-large-v3" => {
                ModelInfo::new("756 MB", "~1.5 GB", "~6x", "Medium-High")
            }
            "distil-medium.en" => ModelInfo::new("789 MB", "~1.5 GB", "~5x", "Medium"),
            "distil-small.en" => ModelInfo::new("332 MB", "~800 MB", "~6x", "Low-Medium"),
            "gpt-4o-transcribe" => ModelInfo::new("Cloud", "N/A", "Fast", "Very High"),
            "gpt-4o-mini-transcribe" => ModelInfo::new("Cloud", "N/A", "Very Fast", "High"),
            "whisper-large-v3-turbo" => ModelInfo::new("Cloud", "N/A", "Very Fast", "High"),
            "whisper-large-v3" => ModelInfo::new("Cloud", "N/A", "Fast", "Very High"),
            _ => ModelInfo::new("Unknown", "Unknown", "Unknown", "Unknown"),
        }
    }

    pub fn get_available_languages() -> Vec<(String, String)> {
        vec![
            ("af", "Afrikaans"),
            ("am", "Amharic"),
            ("ar", "Arabic"),
            ("as", "Assamese"),
            ("az", "Azerbaijani"),
            ("ba", "Bashkir"),
            ("be", "Belarusian"),
            ("bg", "Bulgarian"),
            ("bn", "Bengali"),
            ("bo", "Tibetan"),
            ("br", "Breton"),
            ("bs", "Bosnian"),
            ("ca", "Catalan"),
            ("cs", "Czech"),
            ("cy", "Welsh"),
            ("da", "Danish"),
            ("de", "German"),
            ("el", "Greek"),
            ("en", "English"),
            ("es", "Spanish"),
            ("et", "Estonian"),
            ("eu", "Basque"),
            ("fa", "Persian"),
            ("fi", "Finnish"),
            ("fo", "Faroese"),
            ("fr", "French"),
            ("gl", "Galician"),
            ("gu", "Gujarati"),
            ("ha", "Hausa"),
            ("haw", "Hawaiian"),
            ("he", "Hebrew"),
            ("hi", "Hindi"),
            ("hr", "Croatian"),
            ("ht", "Haitian Creole"),
            ("hu", "Hungarian"),
            ("hy", "Armenian"),
            ("id", "Indonesian"),
            ("is", "Icelandic"),
            ("it", "Italian"),
            ("ja", "Japanese"),
            ("jw", "Javanese"),
            ("ka", "Georgian"),
            ("kk", "Kazakh"),
            ("km", "Khmer"),
            ("kn", "Kannada"),
            ("ko", "Korean"),
            ("la", "Latin"),
            ("lb", "Luxembourgish"),
            ("ln", "Lingala"),
            ("lo", "Lao"),
            ("lt", "Lithuanian"),
            ("lv", "Latvian"),
            ("mg", "Malagasy"),
            ("mi", "Maori"),
            ("mk", "Macedonian"),
            ("ml", "Malayalam"),
            ("mn", "Mongolian"),
            ("mr", "Marathi"),
            ("ms", "Malay"),
            ("mt", "Maltese"),
            ("my", "Myanmar"),
            ("ne", "Nepali"),
            ("nl", "Dutch"),
            ("nn", "Nynorsk"),
            ("no", "Norwegian"),
            ("oc", "Occitan"),
            ("pa", "Panjabi"),
            ("pl", "Polish"),
            ("ps", "Pashto"),
            ("pt", "Portuguese"),
            ("ro", "Romanian"),
            ("ru", "Russian"),
            ("sa", "Sanskrit"),
            ("sd", "Sindhi"),
            ("si", "Sinhala"),
            ("sk", "Slovak"),
            ("sl", "Slovenian"),
            ("sn", "Shona"),
            ("so", "Somali"),
            ("sq", "Albanian"),
            ("sr", "Serbian"),
            ("su", "Sundanese"),
            ("sv", "Swedish"),
            ("sw", "Swahili"),
            ("ta", "Tamil"),
            ("te", "Telugu"),
            ("tg", "Tajik"),
            ("th", "Thai"),
            ("tk", "Turkmen"),
            ("tl", "Tagalog"),
            ("tr", "Turkish"),
            ("tt", "Tatar"),
            ("uk", "Ukrainian"),
            ("ur", "Urdu"),
            ("uz", "Uzbek"),
            ("vi", "Vietnamese"),
            ("yi", "Yiddish"),
            ("yo", "Yoruba"),
            ("yue", "Cantonese"),
            ("zh", "Chinese"),
        ]
        .into_iter()
        .map(|(code, name)| (code.to_string(), name.to_string()))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_set_api_provider_accepts_valid_providers() {
        let mut manager = WhisperManager::new();
        assert!(manager.set_api_provider("openai").is_ok());
        assert!(manager.set_api_provider("groq").is_ok());
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
}
