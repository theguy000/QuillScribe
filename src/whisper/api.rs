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
        if let Some(answer) = self
            .transcribe_elsewhere(audio_data.clone(), sample_rate)
            .await
        {
            return answer;
        }
        let key = self.active_api_key()?;
        let form = self.upload_form(&audio_data, sample_rate)?;
        let response = Self::http_client()
            .post(Self::api_url(&self.api_provider))
            .bearer_auth(&key)
            .multipart(form)
            .send()
            .await?;
        Self::checked_transcript(response, Self::provider_label(&self.api_provider)).await
    }

    /// The providers that do not upload to the generic endpoint, or `None` when
    /// this one does. Each is its own protocol: Gemini's WebChannel, and
    /// Deepgram's prerecorded upload, which is answered by `deepgram.rs`.
    async fn transcribe_elsewhere(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Option<Result<String>> {
        let result = match self.api_provider.as_str() {
            "gemini" => self.transcribe_gemini(audio_data, sample_rate).await,
            "deepgram" => {
                crate::deepgram::transcribe_recorded(&self.deepgram, audio_data, sample_rate).await
            }
            _ => return None,
        };
        Some(result)
    }

    /// The key the chosen provider authenticates with, or the error that says
    /// which provider is missing one.
    fn active_api_key(&self) -> Result<String> {
        let key = if self.api_provider == "groq" {
            &self.groq_api_key
        } else {
            &self.api_key
        };
        if key.is_empty() {
            return Err(anyhow!(
                "API key is not set for {}.",
                Self::provider_label(&self.api_provider),
            ));
        }
        Ok(key.clone())
    }

    /// The upload: the WAV, the model, and the flags Groq needs on top.
    fn upload_form(&self, audio_data: &[f32], sample_rate: u32) -> Result<multipart::Form> {
        let file_part = multipart::Part::bytes(Self::encode_wav(audio_data, sample_rate)?)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;
        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.api_model.clone());
        if !self.api_language.is_empty() {
            form = form.text("language", self.api_language.clone());
        }
        if self.api_provider == "groq" {
            form = form.text("temperature", "0");
            form = form.text("response_format", "json");
        }
        Ok(form)
    }

    /// The transcript, or the provider's own words for why there is none.
    async fn checked_transcript(
        response: reqwest::Response,
        provider_label: &str,
    ) -> Result<String> {
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
