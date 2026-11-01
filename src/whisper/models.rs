//! Downloading, finding and removing the local models.
//!
//! The catalogue in `catalog.rs` decides what the dropdown offers; this file
//! only moves those files on and off the disk.

use super::WhisperManager;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;

use std::fs;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

impl WhisperManager {
    /// Resolve the model file path, preferring a custom path if set.
    pub(super) fn resolve_model_path(&self) -> Result<PathBuf> {
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
    pub(super) fn model_download_url(model_name: &str) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
            model_name
        )
    }
}
