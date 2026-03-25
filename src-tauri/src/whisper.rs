use anyhow::{anyhow, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub size: String,
    pub memory: String,
    pub speed: String,
    pub quality: String,
}

#[derive(Clone)]
pub struct WhisperManager {
    mode: String,
    api_key: String,
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
            api_key: String::new(),
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
            _ => Err(anyhow!("Invalid mode '{}'. Must be 'api' or 'local'.", mode)),
        }
    }

    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    pub fn set_api_model(&mut self, model: &str) -> Result<()> {
        let available = Self::get_available_api_models();
        if available.contains(&model.to_string()) {
            self.api_model = model.to_string();
            Ok(())
        } else {
            Err(anyhow!(
                "Invalid API model '{}'. Available models: {:?}",
                model,
                available
            ))
        }
    }

    pub fn set_api_language(&mut self, lang: &str) -> Result<()> {
        let languages = Self::get_available_languages();
        if languages.iter().any(|(code, _)| code == lang) {
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
        // "auto" is valid for local mode (whisper.cpp auto-detects language)
        if lang == "auto" {
            self.local_language = lang.to_string();
            return Ok(());
        }
        let languages = Self::get_available_languages();
        if languages.iter().any(|(code, _)| code == lang) {
            self.local_language = lang.to_string();
            Ok(())
        } else {
            Err(anyhow!("Unsupported language code '{}'.", lang))
        }
    }

    // ── Transcription entry point ────────────────────────────────────────

    pub async fn transcribe_audio(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        match self.mode.as_str() {
            "api" => self.transcribe_api(audio_data, sample_rate).await,
            "local" => self.transcribe_local(audio_data, sample_rate).await,
            _ => Err(anyhow!("Unknown transcription mode '{}'.", self.mode)),
        }
    }

    // ── API transcription ────────────────────────────────────────────────

    async fn transcribe_api(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow!("API key is not set."));
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

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "OpenAI API request failed with status {}: {}",
                status,
                body
            ));
        }

        let transcription: TranscriptionResponse = response.json().await?;
        Ok(transcription.text)
    }

    // ── Local transcription (whisper.cpp via whisper-rs) ─────────────────

    async fn transcribe_local(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
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
            params.set_n_threads(std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(4)
                .min(8));

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
            fs::remove_file(&path).map_err(|e| {
                anyhow!("Failed to delete model '{}': {}", model_name, e)
            })?;
        }
        Ok(())
    }

    /// Download a GGML model from Hugging Face.
    ///
    /// URL pattern:
    /// `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`
    ///
    /// Returns the path to the downloaded file.
    pub async fn download_model(model_name: &str) -> Result<String> {
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

        // Download with streaming.
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

        let bytes = response.bytes().await?;

        // Write to a temporary file first, then rename for atomicity.
        let tmp_path = dest.with_extension("bin.tmp");
        fs::write(&tmp_path, &bytes).map_err(|e| {
            anyhow!("Failed to write model file: {}", e)
        })?;
        fs::rename(&tmp_path, &dest).map_err(|e| {
            anyhow!("Failed to rename model file: {}", e)
        })?;

        Ok(dest.to_string_lossy().to_string())
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

    pub fn get_model_info(model_name: &str) -> ModelInfo {
        match model_name {
            "tiny" | "tiny.en" => ModelInfo {
                size: "75 MB".to_string(),
                memory: "~390 MB".to_string(),
                speed: "~10x".to_string(),
                quality: "Low".to_string(),
            },
            "base" | "base.en" => ModelInfo {
                size: "142 MB".to_string(),
                memory: "~500 MB".to_string(),
                speed: "~7x".to_string(),
                quality: "Low-Medium".to_string(),
            },
            "small" | "small.en" => ModelInfo {
                size: "466 MB".to_string(),
                memory: "~1 GB".to_string(),
                speed: "~4x".to_string(),
                quality: "Medium".to_string(),
            },
            "medium" | "medium.en" => ModelInfo {
                size: "1.5 GB".to_string(),
                memory: "~2.6 GB".to_string(),
                speed: "~2x".to_string(),
                quality: "Medium-High".to_string(),
            },
            "large-v1" | "large-v2" | "large-v3" => ModelInfo {
                size: "2.9 GB".to_string(),
                memory: "~4.7 GB".to_string(),
                speed: "~1x".to_string(),
                quality: "High".to_string(),
            },
            "turbo" => ModelInfo {
                size: "1.5 GB".to_string(),
                memory: "~2.6 GB".to_string(),
                speed: "~8x".to_string(),
                quality: "High".to_string(),
            },
            "distil-large-v2" | "distil-large-v3" => ModelInfo {
                size: "756 MB".to_string(),
                memory: "~1.5 GB".to_string(),
                speed: "~6x".to_string(),
                quality: "Medium-High".to_string(),
            },
            "distil-medium.en" => ModelInfo {
                size: "789 MB".to_string(),
                memory: "~1.5 GB".to_string(),
                speed: "~5x".to_string(),
                quality: "Medium".to_string(),
            },
            "distil-small.en" => ModelInfo {
                size: "332 MB".to_string(),
                memory: "~800 MB".to_string(),
                speed: "~6x".to_string(),
                quality: "Low-Medium".to_string(),
            },
            "gpt-4o-transcribe" => ModelInfo {
                size: "Cloud".to_string(),
                memory: "N/A".to_string(),
                speed: "Fast".to_string(),
                quality: "Very High".to_string(),
            },
            "gpt-4o-mini-transcribe" => ModelInfo {
                size: "Cloud".to_string(),
                memory: "N/A".to_string(),
                speed: "Very Fast".to_string(),
                quality: "High".to_string(),
            },
            _ => ModelInfo {
                size: "Unknown".to_string(),
                memory: "Unknown".to_string(),
                speed: "Unknown".to_string(),
                quality: "Unknown".to_string(),
            },
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
