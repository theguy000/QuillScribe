//! Local transcription through whisper.cpp.
//!
//! The model runs in a blocking task, because decoding an utterance holds one
//! thread for its whole length and the caller is a runtime that has other work
//! to do.

use super::WhisperManager;
use anyhow::{anyhow, Result};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

impl WhisperManager {
    pub(super) async fn transcribe_local(
        &self,
        audio_data: Vec<f32>,
        sample_rate: u32,
    ) -> Result<String> {
        let model_path = self.local_model_file()?;
        // Resample to 16 kHz if the source sample rate differs.
        let audio_16k = if sample_rate != 16_000 {
            Self::resample(&audio_data, sample_rate, 16_000)
        } else {
            audio_data
        };
        let language = self.local_language.clone();

        // Run whisper inference on a blocking thread (CPU-bound work).
        tokio::task::spawn_blocking(move || run_whisper(&model_path, &audio_16k, &language))
            .await
            .map_err(|e| anyhow!("Whisper task panicked: {}", e))?
    }

    /// The model file this manager is set to use, or the error that says which
    /// file is missing.
    fn local_model_file(&self) -> Result<String> {
        let path = self.resolve_model_path()?;
        if !path.exists() {
            return Err(anyhow!(
                "Model file not found at '{}'. Please download the model first.",
                path.display()
            ));
        }
        path.to_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Model path contains invalid UTF-8"))
    }
}

/// Load the model, transcribe in one go, and join the segments.
///
/// Blocking by nature: this holds one thread for the whole utterance, which is
/// why it runs inside `spawn_blocking` rather than on the runtime.
fn run_whisper(model_path: &str, audio: &[f32], language: &str) -> Result<String> {
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| anyhow!("Failed to load Whisper model: {}", e))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // Set language (if not "auto", set it explicitly).
    if language != "auto" {
        params.set_language(Some(language));
    }
    // Disable console output from whisper.cpp.
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // Do not translate: we want transcription only.
    params.set_translate(false);
    // Bounded parallelism: more threads contend without finishing sooner.
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
        .full(params, audio)
        .map_err(|e| anyhow!("Whisper inference failed: {}", e))?;
    Ok(join_segments(&state))
}

/// Every decoded segment, joined with no separator (whisper.cpp already emits
/// each segment's leading space).
fn join_segments(state: &whisper_rs::WhisperState) -> String {
    let mut text_parts = Vec::new();
    for i in 0..state.full_n_segments() {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(text) = segment.to_str() {
                text_parts.push(text.to_string());
            } else if let Ok(text) = segment.to_str_lossy() {
                text_parts.push(text.to_string());
            }
        }
    }
    text_parts.join("").trim().to_string()
}
