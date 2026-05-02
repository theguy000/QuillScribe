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

use log::warn;
use std::sync::{Arc, Mutex};

use commands::AppState;

slint::include_modules!();

/// Shared application state for Slint callbacks.
struct SharedAppState {
    state: AppState,
    app_weak: Mutex<Option<slint::Weak<App>>>,
}

impl SharedAppState {
    fn with_ui<F>(&self, f: F)
    where
        F: FnOnce(App),
    {
        if let Ok(lock) = self.app_weak.lock() {
            if let Some(weak) = lock.as_ref() {
                if let Some(app) = weak.upgrade() {
                    f(app);
                }
            }
        }
    }
}

pub fn run() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("SLINT_BACKEND", "winit-femtovg");
    }

    // Initialize logging
    env_logger::init();

    // Create a Tokio runtime for async tasks (Slint's event loop is not a Tokio runtime)
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let app_state = AppState::new();
    let initial_theme = {
        let config = app_state.config.lock().unwrap();
        config.get_settings().ui.theme.clone()
    };

    // Set up audio device from config
    {
        let config = app_state.config.lock().unwrap();
        let device_id = config.get_audio_device_id();
        let _custom_titlebar = config.get_custom_titlebar();
        let _always_on_top = config.get_always_on_top();
        let sounds_enabled = config.get_sounds_enabled();
        let max_history = config.get_max_history_entries();
        drop(config);

        app_state.sound.set_sounds_enabled(sounds_enabled);
        app_state.statistics.set_max_history_entries(max_history);

        if let Ok(mut audio) = app_state.audio.lock() {
            if let Err(e) = audio.set_input_device(device_id) {
                log::warn!("Failed to set audio device from config: {}", e);
            }
        }
    }

    let shared = Arc::new(SharedAppState {
        state: app_state,
        app_weak: Mutex::new(None),
    });

    let app = App::new().unwrap();

    // Store weak handle for callbacks that need to update UI
    {
        let mut lock = shared.app_weak.lock().unwrap();
        *lock = Some(app.as_weak());
    }

    // Apply initial theme
    let _theme = initial_theme.clone();
    let _ = slint::invoke_from_event_loop(move || {
        // Theme is set via the theme-colors property
    });

    // ── Wire Slint callbacks ───────────────────────────────────────────

    let shared_toggle = Arc::clone(&shared);
    app.on_toggle_recording(move || {
        let s = Arc::clone(&shared_toggle);
        let ui = s.app_weak.lock().unwrap().clone();

        // Read current state on the UI thread (callback runs on Slint event loop)
        let is_recording = {
            if let Some(app) = ui.as_ref().and_then(|w| w.upgrade()) {
                app.get_is_recording()
            } else {
                false
            }
        };

        if !is_recording {
            // Start recording — synchronous, on UI thread
            s.state.sound.play_start_sound();
            if let Ok(mut audio) = s.state.audio.lock() {
                if let Err(e) = audio.start_recording() {
                    log::error!("Failed to start recording: {}", e);
                    return;
                }
            }
            if let Some(app) = ui.as_ref().and_then(|w| w.upgrade()) {
                app.set_is_recording(true);
                app.set_status_message("Recording...".into());
            }
        } else {
            // Stop recording — synchronous UI update on UI thread
            if let Some(app) = ui.as_ref().and_then(|w| w.upgrade()) {
                app.set_is_recording(false);
                app.set_is_transcribing(true);
                app.set_status_message("Transcribing...".into());
            }

            let (audio_data, sample_rate) = {
                let mut audio = s.state.audio.lock().unwrap();
                match audio.stop_recording() {
                    Ok(Some((data, sr))) => (data, sr),
                    Ok(None) => {
                        if let Some(app) = ui.as_ref().and_then(|w| w.upgrade()) {
                            app.set_is_transcribing(false);
                            app.set_status_message("No speech detected".into());
                        }
                        return;
                    }
                    Err(e) => {
                        log::error!("Failed to stop recording: {}", e);
                        if let Some(app) = ui.as_ref().and_then(|w| w.upgrade()) {
                            app.set_is_transcribing(false);
                            app.set_status_message(format!("Error: {}", e).into());
                        }
                        return;
                    }
                }
            };

            // Apply whisper settings
            let whisper_cfg = {
                let config = s.state.config.lock().unwrap();
                config.get_whisper()
            };

            {
                let mut whisper = s.state.whisper.lock().unwrap();
                let _ = whisper.set_mode(&whisper_cfg.mode);
                let _ = whisper.set_api_provider(&whisper_cfg.api_provider);
                whisper.set_api_key(&whisper_cfg.api_key);
                whisper.set_groq_api_key(&whisper_cfg.groq_api_key);
                let _ = whisper.set_api_model(&whisper_cfg.api_model);
                let _ = whisper.set_api_language(&whisper_cfg.api_language);
                let _ = whisper.set_local_model(&whisper_cfg.local_model);
                whisper.set_local_model_path(&whisper_cfg.local_model_path);
            }

            let whisper_clone = {
                let whisper = s.state.whisper.lock().unwrap();
                whisper.clone()
            };

            let audio_duration = audio_data.len() as f64 / sample_rate as f64;
            let start_time = std::time::Instant::now();

            // Spawn async transcription on Tokio runtime
            let s_async = Arc::clone(&s);
            let ui_async = ui.clone();
            rt.spawn(async move {
                let result = whisper_clone.transcribe_audio(audio_data, sample_rate).await;
                let transcription_time = start_time.elapsed().as_secs_f64();
                let whisper_mode = {
                    let config = s_async.state.config.lock().unwrap();
                    config.get_whisper_mode()
                };

                match result {
                    Ok(text) => {
                        s_async.state.statistics.record_transcription_result(
                            true,
                            &whisper_mode,
                            audio_duration,
                            transcription_time,
                            &text,
                            1.0,
                        );

                        s_async.state.sound.play_stop_sound();

                        // Process output
                        {
                            let config = s_async.state.config.lock().unwrap();
                            let output_cfg = config.get_output();
                            let output = s_async.state.output.lock().unwrap();
                            let _ = output.process_transcription(&text, output::OutputMode::from(output_cfg.mode));
                        }

                        // Update UI from the Slint event loop thread
                        let text_for_ui = text.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = ui_async.as_ref().and_then(|w| w.upgrade()) {
                                app.set_is_transcribing(false);
                                app.set_transcription_text(text_for_ui.into());
                                app.set_status_message("Transcription complete".into());
                            }
                        });
                    }
                    Err(e) => {
                        s_async.state.statistics.record_transcription_result(
                            false,
                            &whisper_mode,
                            audio_duration,
                            transcription_time,
                            "",
                            0.0,
                        );
                        let err_msg = format!("Error: {}", e);
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = ui_async.as_ref().and_then(|w| w.upgrade()) {
                                app.set_is_transcribing(false);
                                app.set_status_message(err_msg.into());
                            }
                        });
                    }
                }
            });
        }
    });

    let shared_nav = Arc::clone(&shared);
    app.on_request_navigate(move |panel: slint::SharedString| {
        // Update active panel so the UI switches views
        shared_nav.with_ui(|app| {
            app.set_active_panel(panel.clone());
        });
        let panel_str = panel.to_string();
        if panel_str == "history" {
            let _history = shared_nav.state.statistics.get_recent_history(7);
            // TODO: feed history into UI
        }
    });

    let shared_copy = Arc::clone(&shared);
    app.on_copy_to_clipboard(move |text: slint::SharedString| {
        let output = shared_copy.state.output.lock().unwrap();
        let _ = output.copy_to_clipboard(&text.to_string());
    });

    let shared_settings = Arc::clone(&shared);
    app.on_save_settings(move |settings_json: slint::SharedString| {
        // TODO: Parse settings JSON and save
        let _ = settings_json;
        let _ = shared_settings;
    });

    let shared_load = Arc::clone(&shared);
    app.on_load_history(move || {
        let _history = shared_load.state.statistics.get_recent_history(30);
        // TODO: feed history into UI
    });

    let shared_update = Arc::clone(&shared);
    app.on_check_for_update(move || {
        let _ = shared_update;
        // TODO: implement self_update check
    });

    let shared_install = Arc::clone(&shared);
    app.on_install_update(move || {
        let _ = shared_install;
        // TODO: implement self_update install
    });

    // Initialize GTK on Linux (required by tray-icon for menu support)
    #[cfg(target_os = "linux")]
    {
        if let Err(e) = gtk::init() {
            warn!("Failed to initialize GTK: {}", e);
        }
    }

    // Set up system tray
    if let Err(e) = tray::setup_tray(&initial_theme, &app.as_weak()) {
        warn!("Failed to set up system tray: {}", e);
    }

    // Set up global hotkey
    hotkey::register_record_toggle(&app.as_weak());

    // Audio level polling timer
    let shared_audio = Arc::clone(&shared);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(50),
        move || {
            let level = {
                if let Ok(audio) = shared_audio.state.audio.lock() {
                    audio.get_audio_level()
                } else {
                    0.0
                }
            };
            shared_audio.with_ui(|app| {
                app.set_audio_level(level);
            });
        },
    );

    // Run the Slint event loop
    app.run().unwrap();

    // Clean up tray
    tray::cleanup_tray();
}
