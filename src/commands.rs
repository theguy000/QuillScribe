use std::sync::Mutex;

use crate::audio::{AudioDevice, AudioManager};
use crate::config::{ConfigManager, Settings};
use crate::output::{OutputManager, OutputMode, PasteToolStatus};
use crate::sound::SoundManager;
use crate::statistics::{HistoryEntry, StatisticsManager};
use crate::whisper::{ModelInfo, WhisperManager};

#[allow(dead_code)]
pub struct AppState {
    pub config: Mutex<ConfigManager>,
    pub audio: Mutex<AudioManager>,
    pub whisper: Mutex<WhisperManager>,
    pub output: Mutex<OutputManager>,
    pub sound: SoundManager,
    pub statistics: StatisticsManager,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(ConfigManager::new().expect("Failed to initialize ConfigManager")),
            audio: Mutex::new(AudioManager::new()),
            whisper: Mutex::new(WhisperManager::new()),
            output: Mutex::new(OutputManager::new()),
            sound: SoundManager::new(),
            statistics: StatisticsManager::new(),
        }
    }
}

// ── Config ───────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn get_settings(state: &AppState) -> Result<Settings, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.get_settings())
}

#[allow(dead_code)]
pub fn save_settings(state: &AppState, mut settings: Settings) -> Result<(), String> {
    if let Some(code) = settings.whisper.api_language.split(',').next() {
        settings.whisper.api_language = code.trim().to_string();
    }

    let device_id = settings.audio.device_id.clone();
    let sounds_enabled = settings.audio.sounds_enabled;

    let config = state.config.lock().map_err(|e| e.to_string())?;
    config.set_settings(settings);
    config.save_settings().map_err(|e| e.to_string())?;
    drop(config);

    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio
        .set_input_device(device_id)
        .map_err(|e| e.to_string())?;
    drop(audio);

    state.sound.set_sounds_enabled(sounds_enabled);

    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let max_history = config.get_max_history_entries();
        drop(config);
        state.statistics.set_max_history_entries(max_history);
    }

    Ok(())
}

#[allow(dead_code)]
pub fn validate_api_key(key: String) -> bool {
    ConfigManager::validate_api_key(&key)
}

// ── Audio ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn get_audio_devices(state: &AppState) -> Result<Vec<AudioDevice>, String> {
    let blocklist = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.get_blocklist()
    };
    let audio = state.audio.lock().map_err(|e| e.to_string())?;
    Ok(audio.get_available_devices(blocklist))
}

#[allow(dead_code)]
pub fn set_audio_device(state: &AppState, device_id: Option<String>) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.set_input_device(device_id).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn start_monitoring(state: &AppState) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.start_monitoring(true).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn stop_monitoring(state: &AppState) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.stop_monitoring();
    Ok(())
}

#[allow(dead_code)]
pub fn start_recording(state: &AppState) -> Result<(), String> {
    state.sound.play_start_sound();
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.start_recording().map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn stop_recording(state: &AppState) -> Result<Option<String>, String> {
    let (audio_data, sample_rate) = {
        let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
        let data = audio.stop_recording().map_err(|e| e.to_string())?;
        match data {
            Some((d, sr)) => (d, sr),
            None => return Ok(None),
        }
    };

    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let whisper_cfg = config.get_whisper();
        let mut whisper = state.whisper.lock().map_err(|e| e.to_string())?;
        whisper
            .set_mode(&whisper_cfg.mode)
            .map_err(|e| e.to_string())?;
        whisper
            .set_api_provider(&whisper_cfg.api_provider)
            .map_err(|e| e.to_string())?;
        whisper.set_api_key(&whisper_cfg.api_key);
        whisper.set_groq_api_key(&whisper_cfg.groq_api_key);
        whisper
            .set_api_model(&whisper_cfg.api_model)
            .map_err(|e| e.to_string())?;
        whisper
            .set_api_language(&whisper_cfg.api_language)
            .map_err(|e| e.to_string())?;
        whisper
            .set_local_model(&whisper_cfg.local_model)
            .map_err(|e| e.to_string())?;
        whisper.set_local_model_path(&whisper_cfg.local_model_path);
        whisper
            .set_local_language(&whisper_cfg.api_language)
            .map_err(|e| e.to_string())?;
    }

    let whisper_mode = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.get_whisper_mode()
    };
    let whisper_clone = {
        let whisper = state.whisper.lock().map_err(|e| e.to_string())?;
        whisper.clone()
    };

    let start_time = std::time::Instant::now();
    let audio_duration = audio_data.len() as f64 / sample_rate as f64;

    let transcribe_result = whisper_clone
        .transcribe_audio(audio_data, sample_rate)
        .await;
    let transcription_time = start_time.elapsed().as_secs_f64();

    let transcribed_text = match transcribe_result {
        Ok(text) => {
            state.statistics.record_transcription_result(
                true,
                &whisper_mode,
                audio_duration,
                transcription_time,
                &text,
                1.0,
            );
            text
        }
        Err(e) => {
            state.statistics.record_transcription_result(
                false,
                &whisper_mode,
                audio_duration,
                transcription_time,
                "",
                0.0,
            );
            return Err(e.to_string());
        }
    };

    state.sound.play_stop_sound();

    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let output_cfg = config.get_output();
        let output = state.output.lock().map_err(|e| e.to_string())?;
        output
            .process_transcription(&transcribed_text, OutputMode::from(output_cfg.mode))
            .map_err(|e| e.to_string())?;
    }

    Ok(Some(transcribed_text))
}

#[allow(dead_code)]
pub fn get_audio_level(state: &AppState) -> Result<f32, String> {
    let audio = state.audio.lock().map_err(|e| e.to_string())?;
    Ok(audio.get_audio_level())
}

#[allow(dead_code)]
pub fn test_microphone(state: &AppState, device_id: Option<String>) -> Result<bool, String> {
    let audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.test_microphone(device_id).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn start_mic_test(state: &AppState, device_id: Option<String>) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio
        .set_input_device(device_id.clone())
        .map_err(|e| e.to_string())?;
    audio.start_monitoring(true).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn stop_mic_test(state: &AppState) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.stop_monitoring();
    let device_id = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.get_audio_device_id()
    };
    audio
        .set_input_device(device_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Whisper ──────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn get_available_models(state: &AppState) -> Result<Vec<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let whisper = config.get_whisper();
    let mode = whisper.mode.clone();
    let provider = whisper.api_provider.clone();

    let models = match mode.as_str() {
        "api" => WhisperManager::get_available_api_models_for_provider(&provider),
        "local" => WhisperManager::get_available_local_models(),
        _ => WhisperManager::get_available_api_models_for_provider(&provider),
    };

    Ok(models)
}

#[allow(dead_code)]
pub fn get_available_local_models() -> Vec<String> {
    WhisperManager::get_available_local_models()
}

#[allow(dead_code)]
pub fn get_model_info(model_name: String) -> ModelInfo {
    WhisperManager::get_model_info(&model_name)
}

#[allow(dead_code)]
pub fn get_available_languages() -> Vec<(String, String)> {
    WhisperManager::get_available_languages()
}

#[allow(dead_code)]
pub async fn download_model(model_name: String) -> Result<String, String> {
    let result = WhisperManager::download_model(&model_name)
        .await
        .map_err(|e| e.to_string())?;
    Ok(result)
}

#[allow(dead_code)]
pub fn is_model_downloaded(model_name: String) -> bool {
    WhisperManager::is_model_downloaded(&model_name)
}

#[allow(dead_code)]
pub fn delete_model(model_name: String) -> Result<(), String> {
    WhisperManager::delete_model(&model_name).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn get_downloaded_models() -> Vec<String> {
    WhisperManager::get_downloaded_models()
}

// ── Output ───────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn process_transcription(state: &AppState, text: String) -> Result<String, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let output_cfg = config.get_output();
    let output = state.output.lock().map_err(|e| e.to_string())?;
    output
        .process_transcription(&text, OutputMode::from(output_cfg.mode))
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn copy_to_clipboard(state: &AppState, text: String) -> Result<(), String> {
    let output = state.output.lock().map_err(|e| e.to_string())?;
    output.copy_to_clipboard(&text).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn test_clipboard(state: &AppState) -> Result<bool, String> {
    let output = state.output.lock().map_err(|e| e.to_string())?;
    output.test_clipboard().map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub fn check_paste_tool_status() -> PasteToolStatus {
    crate::output::check_paste_tool().clone()
}

#[allow(dead_code)]
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

#[allow(dead_code)]
pub fn has_compositor() -> bool {
    #[cfg(target_os = "linux")]
    {
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|s| s.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        if is_wayland {
            return true;
        }

        if std::env::var_os("KDE_FULL_SESSION").is_some()
            || std::env::var_os("GNOME_DESKTOP_SESSION_ID").is_some()
        {
            return true;
        }
        const COMPOSITED_DESKTOPS: &[&str] = &[
            "kde",
            "gnome",
            "unity",
            "cinnamon",
            "deepin",
            "pantheon",
            "enlightenment",
        ];
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            let d = desktop.to_lowercase();
            if COMPOSITED_DESKTOPS.iter().any(|de| d.contains(de)) {
                return true;
            }
        }

        for name in ["picom", "compton", "xcompmgr"] {
            if std::process::Command::new("pgrep")
                .args(["-x", name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

// ── Sound ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn play_start_sound(state: &AppState) {
    state.sound.play_start_sound();
}

#[allow(dead_code)]
pub fn play_stop_sound(state: &AppState) {
    state.sound.play_stop_sound();
}

#[allow(dead_code)]
pub fn set_sounds_enabled(state: &AppState, enabled: bool) {
    state.sound.set_sounds_enabled(enabled);
}

#[allow(dead_code)]
pub fn get_sounds_enabled(state: &AppState) -> bool {
    state.sound.is_sounds_enabled()
}

// ── Statistics ─────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn get_recent_history(state: &AppState, days: Option<i64>) -> Vec<HistoryEntry> {
    state.statistics.get_recent_history(days.unwrap_or(7))
}

// ── Window ─────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn set_always_on_top(state: &AppState, on_top: bool) -> Result<(), String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    config.set_always_on_top(on_top);
    let _ = config.save_settings();
    Ok(())
}

#[allow(dead_code)]
pub fn get_always_on_top(state: &AppState) -> Result<bool, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.get_always_on_top())
}
