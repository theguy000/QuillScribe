use anyhow::{anyhow, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

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
        }
    }

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

    pub async fn transcribe_audio(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        match self.mode.as_str() {
            "api" => self.transcribe_api(audio_data, sample_rate).await,
            "local" => Err(anyhow!("Local mode not yet implemented.")),
            _ => Err(anyhow!("Unknown transcription mode '{}'.", self.mode)),
        }
    }

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
