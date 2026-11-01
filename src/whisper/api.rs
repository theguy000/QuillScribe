//! Talking to a hosted provider: the request, and reading what comes back.
//!
//! One path for every provider that takes an upload and answers with JSON; the
//! provider only chooses the URL, the header and the body shape.

use super::WhisperManager;
use anyhow::{anyhow, Result};
use reqwest::multipart;
use serde::Deserialize;

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl WhisperManager {
    fn provider_label(provider: &str) -> &'static str {
        match provider {
            "gemini" => "Gemini",
            "groq" => "Groq",
            "deepgram" => "Deepgram",
            _ => "OpenAI",
        }
    }

    fn api_url(provider: &str) -> &'static str {
        match provider {
            "groq" => "https://api.groq.com/openai/v1/audio/transcriptions",
            _ => "https://api.openai.com/v1/audio/transcriptions",
        }
    }

    pub(super) async fn transcribe_api(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        if self.api_provider == "gemini" {
            return self.transcribe_gemini(audio_data, sample_rate).await;
        }

        // Deepgram's recorded mode: upload the finished utterance. Its live
        // mode never reaches here — it is driven by a session started at
        // record-start instead.
        if self.api_provider == "deepgram" {
            return crate::deepgram::transcribe_recorded(&self.deepgram, audio_data, sample_rate)
                .await;
        }

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
}
