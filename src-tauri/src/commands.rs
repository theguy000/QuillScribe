use log::warn;
use std::sync::Mutex;

use crate::audio::{AudioDevice, AudioManager};
use crate::config::{ConfigManager, Settings};
use crate::hotkey;
use crate::output::{OutputManager, OutputMode, PasteToolStatus};
use crate::sound::SoundManager;
use crate::statistics::{HistoryEntry, StatisticsManager};
use crate::whisper::{ModelInfo, WhisperManager};

pub struct AppState {
    pub config: Mutex<ConfigManager>,
    pub audio: Mutex<AudioManager>,
    pub whisper: Mutex<WhisperManager>,
    pub output: Mutex<OutputManager>,
    pub sound: SoundManager,
    pub statistics: StatisticsManager,
}

/// Creates the initial `AppState` with default managers.
pub fn app_state() -> AppState {
    AppState {
        config: Mutex::new(ConfigManager::new().expect("Failed to initialize ConfigManager")),
        audio: Mutex::new(AudioManager::new()),
        whisper: Mutex::new(WhisperManager::new()),
        output: Mutex::new(OutputManager::new()),
        sound: SoundManager::new(),
        statistics: StatisticsManager::new(),
    }
}

// ── Config commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: tauri::State<AppState>) -> Result<Settings, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.get_settings())
}

#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    mut settings: Settings,
) -> Result<(), String> {
    // Sanitize the language code: strip anything after a comma (e.g. "en,English" → "en").
    if let Some(code) = settings.whisper.api_language.split(',').next() {
        settings.whisper.api_language = code.trim().to_string();
    }

    // Extract values before moving settings into config.
    let device_id = settings.audio.device_id.clone();
    let sounds_enabled = settings.audio.sounds_enabled;

    let config = state.config.lock().map_err(|e| e.to_string())?;
    config.set_settings(settings);
    config.save_settings().map_err(|e| e.to_string())?;
    drop(config);

    // Apply the audio device to the AudioManager so it takes effect immediately.
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio
        .set_input_device(device_id)
        .map_err(|e| e.to_string())?;
    drop(audio);

    // Apply the sounds_enabled flag so it takes effect immediately.
    state.sound.set_sounds_enabled(sounds_enabled);

    // Apply the max history entries setting so it takes effect immediately.
    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let max_history = config.get_max_history_entries();
        drop(config);
        state.statistics.set_max_history_entries(max_history);
    }

    // Re-register the global hotkey so shortcut changes take effect immediately.
    hotkey::register_record_toggle(&app);

    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let custom_titlebar = config.get_custom_titlebar();
        let always_on_top = config.get_always_on_top();
        drop(config);

        crate::window::apply_custom_titlebar(&app, custom_titlebar);
        crate::window::apply_always_on_top(&app, always_on_top);
    }

    // Rebuild the tray menu so model / mode changes are reflected.
    if let Err(e) = crate::tray::rebuild_tray_menu(&app) {
        warn!("Failed to rebuild tray menu after settings change: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub fn validate_api_key(key: String) -> bool {
    ConfigManager::validate_api_key(&key)
}

// ── Audio commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_audio_devices(state: tauri::State<AppState>) -> Result<Vec<AudioDevice>, String> {
    let audio = state.audio.lock().map_err(|e| e.to_string())?;
    Ok(audio.get_available_devices())
}

#[tauri::command]
pub fn set_audio_device(
    state: tauri::State<AppState>,
    device_id: Option<String>,
) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.set_input_device(device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_monitoring(state: tauri::State<AppState>) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.start_monitoring().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_monitoring(state: tauri::State<AppState>) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.stop_monitoring();
    Ok(())
}

#[tauri::command]
pub fn start_recording(state: tauri::State<AppState>) -> Result<(), String> {
    state.sound.play_start_sound();
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.start_recording().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_recording(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    // 1. Get audio data from the audio manager.
    let (audio_data, sample_rate) = {
        let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
        let data = audio.stop_recording().map_err(|e| e.to_string())?;

        match data {
            Some((d, sr)) => (d, sr),
            None => return Ok(None),
        }
    };

    // 2. Apply whisper settings from config.
    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let whisper_cfg = config.get_whisper();

        let mut whisper = state.whisper.lock().map_err(|e| e.to_string())?;
        whisper
            .set_mode(&whisper_cfg.mode)
            .map_err(|e| e.to_string())?;
        whisper.set_api_key(&whisper_cfg.api_key);
        whisper
            .set_api_model(&whisper_cfg.api_model)
            .map_err(|e| e.to_string())?;
        whisper
            .set_api_language(&whisper_cfg.api_language)
            .map_err(|e| e.to_string())?;

        // Apply local mode settings.
        whisper
            .set_local_model(&whisper_cfg.local_model)
            .map_err(|e| e.to_string())?;
        whisper.set_local_model_path(&whisper_cfg.local_model_path);
        whisper
            .set_local_language(&whisper_cfg.api_language)
            .map_err(|e| e.to_string())?;
    }

    // 3. Transcribe (async) — clone the whisper manager so we can drop the lock before await.
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

    // 4. Play stop sound.
    state.sound.play_stop_sound();

    // 4. Process transcription through output manager.
    {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let output_cfg = config.get_output();

        let output = state.output.lock().map_err(|e| e.to_string())?;
        output
            .process_transcription(
                &transcribed_text,
                OutputMode::from(output_cfg.mode),
                output_cfg.silent_mode,
            )
            .map_err(|e| e.to_string())?;
    }

    // 5. Return the transcribed text.
    Ok(Some(transcribed_text))
}

#[tauri::command]
pub fn get_audio_level(state: tauri::State<AppState>) -> Result<f32, String> {
    let audio = state.audio.lock().map_err(|e| e.to_string())?;
    Ok(audio.get_audio_level())
}

#[tauri::command]
pub fn test_microphone(
    state: tauri::State<AppState>,
    device_id: Option<String>,
) -> Result<bool, String> {
    let audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.test_microphone(device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_mic_test(
    state: tauri::State<AppState>,
    device_id: Option<String>,
) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio
        .set_input_device(device_id.clone())
        .map_err(|e| e.to_string())?;
    audio.start_monitoring().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_mic_test(state: tauri::State<AppState>) -> Result<(), String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.stop_monitoring();
    // Restore the persisted audio device so the mic test doesn't leave
    // a stale `selected_device_id` when the user cancels settings.
    let device_id = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.get_audio_device_id()
    };
    audio
        .set_input_device(device_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Whisper commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_available_models(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let mode = config.get_whisper_mode();

    let models = match mode.as_str() {
        "api" => WhisperManager::get_available_api_models(),
        "local" => WhisperManager::get_available_local_models(),
        _ => WhisperManager::get_available_api_models(),
    };

    Ok(models)
}

#[tauri::command]
pub fn get_available_local_models() -> Vec<String> {
    WhisperManager::get_available_local_models()
}

#[tauri::command]
pub fn get_model_info(model_name: String) -> ModelInfo {
    WhisperManager::get_model_info(&model_name)
}

#[tauri::command]
pub fn get_available_languages() -> Vec<(String, String)> {
    WhisperManager::get_available_languages()
}

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, model_name: String) -> Result<String, String> {
    let result = WhisperManager::download_model(&model_name)
        .await
        .map_err(|e| e.to_string())?;
    // Rebuild tray menu so the newly downloaded model appears in the submenu.
    if let Err(e) = crate::tray::rebuild_tray_menu(&app) {
        warn!("Failed to rebuild tray menu after model download: {}", e);
    }
    Ok(result)
}

#[tauri::command]
pub fn is_model_downloaded(model_name: String) -> bool {
    WhisperManager::is_model_downloaded(&model_name)
}

#[tauri::command]
pub fn delete_model(app: tauri::AppHandle, model_name: String) -> Result<(), String> {
    WhisperManager::delete_model(&model_name).map_err(|e| e.to_string())?;
    // Rebuild tray menu so the deleted model is removed from the submenu.
    if let Err(e) = crate::tray::rebuild_tray_menu(&app) {
        warn!("Failed to rebuild tray menu after model delete: {}", e);
    }
    Ok(())
}

#[tauri::command]
pub fn get_downloaded_models() -> Vec<String> {
    WhisperManager::get_downloaded_models()
}

// ── Output commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn process_transcription(
    state: tauri::State<AppState>,
    text: String,
) -> Result<String, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let output_cfg = config.get_output();

    let output = state.output.lock().map_err(|e| e.to_string())?;
    output
        .process_transcription(
            &text,
            OutputMode::from(output_cfg.mode),
            output_cfg.silent_mode,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn copy_to_clipboard(state: tauri::State<AppState>, text: String) -> Result<(), String> {
    let output = state.output.lock().map_err(|e| e.to_string())?;
    output.copy_to_clipboard(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_clipboard(state: tauri::State<AppState>) -> Result<bool, String> {
    let output = state.output.lock().map_err(|e| e.to_string())?;
    output.test_clipboard().map_err(|e| e.to_string())
}

/// Check which paste tool is available on Linux (xdotool or ydotool).
/// On Windows this returns a dummy status since paste tools are not applicable.
/// The result is cached after the first call for the lifetime of the process.
#[tauri::command]
pub fn check_paste_tool_status() -> PasteToolStatus {
    crate::output::check_paste_tool().clone()
}

/// Returns true if the current platform is Linux.
#[tauri::command]
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Returns true when the OS supports transparent windows.
///
/// - Windows / macOS / Wayland → always true (compositor is guaranteed).
/// - Linux + X11 → true only if a compositor is running (`_NET_WM_CM_S0` atom is owned).
#[tauri::command]
pub fn has_compositor() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check the XDG_SESSION_TYPE env var first — Wayland always composites.
        if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
            if session.eq_ignore_ascii_case("wayland") {
                return true;
            }
        }

        // X11: check if a compositor owns the _NET_WM_CM_S0 selection atom.
        // We shell out to xprop because pulling in x11-rb just for this is overkill.
        match std::process::Command::new("xprop")
            .args(["-root", "-notype", "_NET_WM_CM_S0"])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // If a compositor is running the output contains a window id, not "not found".
                !stdout.contains("not found")
            }
            Err(_) => {
                // xprop not available — assume no compositor.
                false
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

// ── Sound commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn play_start_sound(state: tauri::State<AppState>) {
    state.sound.play_start_sound();
}

#[tauri::command]
pub fn play_stop_sound(state: tauri::State<AppState>) {
    state.sound.play_stop_sound();
}

#[tauri::command]
pub fn set_sounds_enabled(state: tauri::State<AppState>, enabled: bool) {
    state.sound.set_sounds_enabled(enabled);
}

#[tauri::command]
pub fn get_sounds_enabled(state: tauri::State<AppState>) -> bool {
    state.sound.is_sounds_enabled()
}

// ── Statistics commands ──────────────────────────────────────────────────────

#[tauri::command]
pub fn get_recent_history(state: tauri::State<AppState>, days: Option<i64>) -> Vec<HistoryEntry> {
    state.statistics.get_recent_history(days.unwrap_or(7))
}

// ── Window commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn set_always_on_top(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    on_top: bool,
) -> Result<(), String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    crate::window::set_always_on_top(&app, &config, on_top);
    Ok(())
}

#[tauri::command]
pub fn get_always_on_top(state: tauri::State<AppState>) -> Result<bool, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.get_always_on_top())
}

#[tauri::command]
pub fn set_tray_theme(app: tauri::AppHandle, theme: String) {
    crate::tray::set_tray_theme(&app, &theme);
}

#[tauri::command]
pub fn set_taskbar_icon_theme(app: tauri::AppHandle, theme: String) {
    crate::tray::set_window_icon_theme(&app, &theme);
}

#[tauri::command]
pub fn rebuild_tray_menu(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::rebuild_tray_menu(&app).map_err(|e| e.to_string())
}
