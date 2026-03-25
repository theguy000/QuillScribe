mod audio;
mod commands;
mod config;
mod hotkey;
mod output;
mod sound;
mod statistics;
mod tray;
mod whisper;
mod window;

use commands::{
    app_state, copy_to_clipboard, export_statistics, get_accuracy_rate, get_all_time_stats,
    get_always_on_top, get_audio_devices, get_audio_level, get_available_languages,
    get_available_local_models, get_available_models, get_daily_usage, get_model_info,
    get_recent_history, get_session_stats, get_settings, get_sounds_enabled, play_start_sound,
    play_stop_sound, process_transcription, reset_statistics, save_settings, set_always_on_top,
    set_audio_device, set_sounds_enabled, start_monitoring, start_recording, stop_monitoring,
    stop_recording, test_clipboard, test_microphone, validate_api_key,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(app_state())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Set up system tray
            let handle = app.handle().clone();
            if let Err(e) = tray::setup_tray(&handle) {
                log::warn!("Failed to set up system tray: {}", e);
            }

            // Restore window position and apply settings
            let state: tauri::State<commands::AppState> = app.state();
            if let Ok(config) = state.config.lock() {
                window::restore_window_position(&handle, &config);
                window::apply_always_on_top(&handle, &config);

                // Initialize sound enabled state from config
                state.sound.set_sounds_enabled(config.get_sounds_enabled());
            }

            // Record session start for statistics
            state.statistics.record_session_start();

            // Register global hotkeys
            hotkey::register_record_toggle(&handle);

            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // Save window position before closing
                    let app = window.app_handle();
                    let state = app.state::<commands::AppState>();
                    {
                        let config = state.config.lock();
                        if let Ok(config) = config {
                            window::save_window_position(app, &config);

                            // If minimize_on_close is enabled, minimize instead of closing
                            if config.get_minimize_on_close() {
                                api.prevent_close();
                                let _ = window.hide();
                            }
                        }
                    };

                    // Record session end on actual close
                    state.statistics.record_session_end();
                }
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    // Auto-save position on move/resize
                    let app = window.app_handle();
                    let state = app.state::<commands::AppState>();
                    let config = state.config.lock();
                    if let Ok(config) = config {
                        window::save_window_position(app, &config);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Config
            get_settings,
            save_settings,
            validate_api_key,
            // Audio
            get_audio_devices,
            set_audio_device,
            start_monitoring,
            stop_monitoring,
            start_recording,
            stop_recording,
            get_audio_level,
            test_microphone,
            // Whisper
            get_available_models,
            get_available_local_models,
            get_model_info,
            get_available_languages,
            // Output
            process_transcription,
            copy_to_clipboard,
            test_clipboard,
            // Sound
            play_start_sound,
            play_stop_sound,
            set_sounds_enabled,
            get_sounds_enabled,
            // Statistics
            get_all_time_stats,
            get_session_stats,
            get_recent_history,
            get_accuracy_rate,
            get_daily_usage,
            reset_statistics,
            export_statistics,
            // Window
            set_always_on_top,
            get_always_on_top,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
