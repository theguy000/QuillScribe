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

const THEME_MAP: &[(&str, &str)] = &[
    ("white", "White"),
    ("warm_gray", "Warm Gray"),
    ("soft_beige", "Soft Beige"),
    ("blue_gray", "Blue Gray"),
    ("warm_taupe", "Warm Taupe"),
    ("soft_sage", "Soft Sage"),
    ("dark_charcoal", "Dark Charcoal"),
    ("dark_blue", "Dark Blue"),
    ("dark_purple", "Dark Purple"),
    ("dark_forest", "Dark Forest"),
    ("dark_burgundy", "Dark Burgundy"),
    ("obsidian", "Obsidian"),
];

fn theme_key_to_display(key: &str) -> &str {
    THEME_MAP.iter().find(|(k, _)| *k == key).map(|(_, d)| *d).unwrap_or("White")
}

fn theme_display_to_key(display: &str) -> &str {
    THEME_MAP.iter().find(|(_, d)| *d == display).map(|(k, _)| *k).unwrap_or("white")
}

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

/// Helper: save a single-field config change.
/// Reads current settings, applies `mutate`, writes back.
fn save_config_field<F>(shared: &SharedAppState, mutate: F)
where
    F: FnOnce(&mut config::Settings),
{
    let config = shared.state.config.lock().unwrap();
    let mut settings = config.get_settings();
    mutate(&mut settings);
    config.set_settings(settings);
    if let Err(e) = config.save_settings() {
        log::error!("Failed to save settings: {}", e);
    }
}

/// Helper: refresh the blocklist model and audio device list in the UI.
/// Also checks if the currently configured device is still visible (not blocklisted)
/// and falls back to default if it isn't.
fn refresh_blocklist_and_devices(shared: &SharedAppState) {
    let (blocklist, current_device_id) = {
        let config = shared.state.config.lock().unwrap();
        (config.get_blocklist(), config.get_audio_device_id())
    };

    let devices = if let Ok(audio) = shared.state.audio.lock() {
        audio.get_available_devices(blocklist.clone())
    } else {
        Vec::new()
    };

    // If the saved device is blocklisted, clear it and fall back to default
    if let Some(ref dev_id) = current_device_id {
        if !devices.iter().any(|d| &d.id == dev_id) {
            save_config_field(shared, |s| {
                s.audio.device_id = None;
            });
            if let Ok(mut audio) = shared.state.audio.lock() {
                if let Err(e) = audio.set_input_device(None) {
                    log::warn!("Failed to reset blocklisted device: {}", e);
                }
            }
        }
    }

    // Resolve selected display name before consuming devices
    let selected_name = match &current_device_id {
        Some(dev_id) => devices
            .iter()
            .find(|d| &d.id == dev_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "Default".to_string()),
        None => "Default".to_string(),
    };

    let blocklist_items: Vec<slint::SharedString> = blocklist
        .iter()
        .map(|s| slint::SharedString::from(s.as_str()))
        .collect();
    let blocklist_model = std::rc::Rc::new(slint::VecModel::from(blocklist_items));

    let mut names: Vec<slint::SharedString> = vec!["Default".into()];
    names.extend(
        devices
            .into_iter()
            .map(|d| slint::SharedString::from(d.name.as_str())),
    );
    let device_model = std::rc::Rc::new(slint::VecModel::from(names));

    shared.with_ui(|app| {
        app.set_settings_audio_blocklist(slint::ModelRc::from(blocklist_model));
        app.set_settings_audio_devices(slint::ModelRc::from(device_model));
        app.set_settings_audio_device_id(selected_name.into());
    });
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
    let rt_handle = rt.handle().clone();

    let app_state = AppState::new();
    let initial_theme = {
        let config = app_state.config.lock().unwrap();
        config.get_settings().ui.theme.clone()
    };

    // Set up audio device from config
    {
        let config = app_state.config.lock().unwrap();
        let device_id = config.get_audio_device_id();
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

    // ── Initialize UI from persisted config ──────────────────────────────
    {
        let config = shared.state.config.lock().unwrap();
        let s = config.get_settings();

        // Audio
        if let Some(ref dev_id) = s.audio.device_id {
            let dev_name = if let Ok(audio) = shared.state.audio.lock() {
                audio.get_available_devices(s.audio.blocklist.clone())
                    .into_iter()
                    .find(|d| &d.id == dev_id)
                    .map(|d| d.name)
                    .unwrap_or_else(|| dev_id.clone())
            } else {
                dev_id.clone()
            };
            app.set_settings_audio_device_id(dev_name.into());
        } else {
            app.set_settings_audio_device_id("Default".into());
        }
        app.set_settings_sounds_enabled(s.audio.sounds_enabled);
        
        let blocklist_items: Vec<slint::SharedString> = s.audio.blocklist.iter()
            .map(|s| slint::SharedString::from(s.as_str()))
            .collect();
        let blocklist_model = std::rc::Rc::new(slint::VecModel::from(blocklist_items));
        app.set_settings_audio_blocklist(slint::ModelRc::from(blocklist_model));

        if let Ok(audio) = shared.state.audio.lock() {
            let devices = audio.get_available_devices(s.audio.blocklist.clone());
            let mut names: Vec<slint::SharedString> = vec!["Default".into()];
            names.extend(
                devices
                    .into_iter()
                    .map(|d| slint::SharedString::from(d.name.as_str()))
            );
            let model = std::rc::Rc::new(slint::VecModel::from(names));
            app.set_settings_audio_devices(slint::ModelRc::from(model.clone()));
        }

        // Whisper / Engine
        app.set_settings_whisper_mode(s.whisper.mode.clone().into());
        app.set_settings_api_provider(s.whisper.api_provider.clone().into());
        app.set_settings_api_key(s.whisper.api_key.clone().into());
        app.set_settings_groq_api_key(s.whisper.groq_api_key.clone().into());
        app.set_settings_api_model(s.whisper.api_model.clone().into());
        app.set_settings_api_language(s.whisper.api_language.clone().into());
        app.set_settings_local_model(s.whisper.local_model.clone().into());

        // UI
        let theme_display = theme_key_to_display(&s.ui.theme);
        app.set_settings_theme(theme_display.into());
        app.set_current_theme(s.ui.theme.clone().into());
        app.set_settings_custom_titlebar(s.ui.custom_titlebar);
        app.set_settings_always_on_top(s.ui.always_on_top);
        app.set_settings_overlay_mode(s.ui.overlay_mode.clone().into());
        app.set_settings_overlay_style(s.ui.overlay_style.clone().into());
        app.set_settings_overlay_opacity(s.ui.overlay_opacity as f32);
        app.set_settings_max_history_entries(s.advanced.max_history_entries as i32);

        // Output
        app.set_settings_output_mode(s.output.mode as i32);

        // Keyboard
        app.set_settings_record_toggle_shortcut(s.shortcuts.record_toggle.clone().into());

        // Platform
        app.set_settings_is_linux(cfg!(target_os = "linux"));

        drop(config);
    }

    // ── Wire core Slint callbacks ────────────────────────────────────────

    let shared_toggle = Arc::clone(&shared);
    let rt_handle_toggle = rt_handle.clone();
    app.on_toggle_recording(move || {
        let rt_handle = rt_handle_toggle.clone();
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
            rt_handle.spawn(async move {
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

                                // Trigger completion burst animation
                                app.set_show_burst(true);
                                let weak = app.as_weak();
                                slint::Timer::single_shot(
                                    std::time::Duration::from_millis(800),
                                    move || {
                                        if let Some(app) = weak.upgrade() {
                                            app.set_show_burst(false);
                                        }
                                    },
                                );
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

    let shared_load = Arc::clone(&shared);
    app.on_load_history(move || {
        let history = shared_load.state.statistics.get_recent_history(30);
        let count = history.len() as i32;
        shared_load.with_ui(|app| {
            app.set_session_count(count);
        });
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

    // ── Settings callbacks ───────────────────────────────────────────────

    // Audio
    let shared_cb = Arc::clone(&shared);
    app.on_settings_audio_device_changed(move |device_name: slint::SharedString| {
        let dev_name = device_name.to_string();
        let dev_id = if dev_name.is_empty() || dev_name == "Default" {
            None
        } else {
            if let Ok(audio) = shared_cb.state.audio.lock() {
                let blocklist = {
                    let config = shared_cb.state.config.lock().unwrap();
                    config.get_blocklist()
                };
                audio.get_available_devices(blocklist)
                    .into_iter()
                    .find(|d| d.name == dev_name)
                    .map(|d| d.id)
            } else {
                Some(dev_name)
            }
        };
        save_config_field(&shared_cb, |s| {
            s.audio.device_id = dev_id.clone();
        });
        if let Ok(mut audio) = shared_cb.state.audio.lock() {
            if let Err(e) = audio.set_input_device(dev_id) {
                log::warn!("Failed to set audio device: {}", e);
            }
        }
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_refresh_audio_devices(move || {
        refresh_blocklist_and_devices(&shared_cb);
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_start_mic_test(move || {
        shared_cb.with_ui(|app| app.set_settings_testing_mic(true));
        if let Ok(mut audio) = shared_cb.state.audio.lock() {
            if let Err(e) = audio.start_monitoring(true) {
                log::warn!("Failed to start mic test: {}", e);
                shared_cb.with_ui(|app| app.set_settings_testing_mic(false));
            }
        }
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_stop_mic_test(move || {
        shared_cb.with_ui(|app| app.set_settings_testing_mic(false));
        if let Ok(mut audio) = shared_cb.state.audio.lock() {
            audio.stop_monitoring();
        }
    });

    app.on_settings_manage_blocklist(move || {
        // Popup is handled entirely in Slint; this callback is a no-op.
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_add_blocklist_item(move |item: slint::SharedString| {
        let item_str = item.to_string();
        if item_str.is_empty() { return; }
        
        save_config_field(&shared_cb, |s| {
            if !s.audio.blocklist.contains(&item_str) {
                s.audio.blocklist.push(item_str);
            }
        });
        
        refresh_blocklist_and_devices(&shared_cb);
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_remove_blocklist_item(move |idx: i32| {
        if idx < 0 { return; }
        
        save_config_field(&shared_cb, |s| {
            if (idx as usize) < s.audio.blocklist.len() {
                s.audio.blocklist.remove(idx as usize);
            }
        });
        
        refresh_blocklist_and_devices(&shared_cb);
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_sounds_enabled_changed(move |enabled: bool| {
        save_config_field(&shared_cb, |s| {
            s.audio.sounds_enabled = enabled;
        });
        shared_cb.state.sound.set_sounds_enabled(enabled);
    });

    // Whisper / Engine
    let shared_cb = Arc::clone(&shared);
    app.on_settings_mode_changed(move |mode: slint::SharedString| {
        let mode_str = mode.to_string();
        shared_cb.with_ui(|app| {
            app.set_settings_whisper_mode(mode.clone());
        });
        save_config_field(&shared_cb, |s| {
            s.whisper.mode = mode_str;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_provider_changed(move |provider: slint::SharedString| {
        let provider_str = provider.to_string();
        // Update the available models list for this provider
        let models = whisper::WhisperManager::get_available_api_models_for_provider(&provider_str);
        let model_strings: Vec<slint::SharedString> = models
            .iter()
            .map(|m| slint::SharedString::from(m.as_str()))
            .collect();
        let model_rc = std::rc::Rc::new(slint::VecModel::from(model_strings));
        let first_model = models.first().cloned().unwrap_or_default();
        shared_cb.with_ui(|app| {
            app.set_settings_api_models(slint::ModelRc::from(model_rc.clone()));
            app.set_settings_api_model(first_model.clone().into());
        });
        save_config_field(&shared_cb, |s| {
            s.whisper.api_provider = provider_str;
            s.whisper.api_model = first_model;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_api_key_changed(move |key: slint::SharedString| {
        let key_str = key.to_string();
        save_config_field(&shared_cb, |s| {
            s.whisper.api_key = key_str;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_api_model_changed(move |model: slint::SharedString| {
        let model_str = model.to_string();
        save_config_field(&shared_cb, |s| {
            s.whisper.api_model = model_str;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_language_changed(move |lang: slint::SharedString| {
        let lang_str = lang.to_string();
        save_config_field(&shared_cb, |s| {
            s.whisper.api_language = lang_str;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_local_model_changed(move |model: slint::SharedString| {
        let model_str = model.to_string();
        // Check download status and fetch model info, then update UI together
        let downloaded = whisper::WhisperManager::is_model_downloaded(&model_str);
        let info = whisper::WhisperManager::get_model_info(&model_str);
        shared_cb.with_ui(|app| {
            app.set_settings_model_downloaded(downloaded);
            app.set_settings_model_size(info.size.into());
            app.set_settings_model_memory(info.memory.into());
            app.set_settings_model_speed(info.speed.into());
            app.set_settings_model_quality(info.quality.into());
        });
        save_config_field(&shared_cb, |s| {
            s.whisper.local_model = model_str;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_category_changed(move |_category: slint::SharedString| {
        // Category is a UI-only grouping filter — no config change needed
        let _ = shared_cb;
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_toggle_api_key_visibility(move || {
        shared_cb.with_ui(|app| {
            let current = app.get_settings_show_api_key();
            app.set_settings_show_api_key(!current);
        });
    });

    // Download model (async)
    let shared_cb = Arc::clone(&shared);
    app.on_settings_download_model({
        let rt_handle = rt_handle.clone();
        move || {
            let s = Arc::clone(&shared_cb);
            s.with_ui(|app| {
                app.set_settings_downloading_model(true);
                app.set_settings_download_error("".into());
            });
            let model_name = {
                let config = s.state.config.lock().unwrap();
                config.get_whisper().local_model
            };
            let s2 = Arc::clone(&s);
            rt_handle.spawn(async move {
                match whisper::WhisperManager::download_model(&model_name).await {
                    Ok(_) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            s2.with_ui(|app| {
                                app.set_settings_downloading_model(false);
                                app.set_settings_model_downloaded(true);
                            });
                        });
                    }
                    Err(e) => {
                        let err = format!("{}", e);
                        let _ = slint::invoke_from_event_loop(move || {
                            s2.with_ui(|app| {
                                app.set_settings_downloading_model(false);
                                app.set_settings_download_error(err.into());
                            });
                        });
                    }
                }
            });
        }
    });

    // Delete model
    let shared_cb = Arc::clone(&shared);
    app.on_settings_delete_model(move || {
        let model_name = {
            let config = shared_cb.state.config.lock().unwrap();
            config.get_whisper().local_model
        };
        shared_cb.with_ui(|app| {
            app.set_settings_deleting_model(true);
        });
        match whisper::WhisperManager::delete_model(&model_name) {
            Ok(_) => {
                shared_cb.with_ui(|app| {
                    app.set_settings_deleting_model(false);
                    app.set_settings_model_downloaded(false);
                });
            }
            Err(e) => {
                log::error!("Failed to delete model: {}", e);
                shared_cb.with_ui(|app| {
                    app.set_settings_deleting_model(false);
                });
            }
        }
    });

    // UI settings
    let shared_cb = Arc::clone(&shared);
    app.on_settings_theme_changed(move |theme: slint::SharedString| {
        let display_str = theme.to_string();
        let key = theme_display_to_key(&display_str).to_string();
        save_config_field(&shared_cb, |s| {
            s.ui.theme = key.clone();
        });
        shared_cb.with_ui(|app| {
            app.set_current_theme(key.into());
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_overlay_mode_changed(move |mode: slint::SharedString| {
        let mode_str = mode.to_string();
        save_config_field(&shared_cb, |s| {
            s.ui.overlay_mode = mode_str;
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_overlay_style_changed(move |style: slint::SharedString| {
        let style_str = style.to_string();
        save_config_field(&shared_cb, |s| {
            s.ui.overlay_style = style_str;
        });
    });

    // Output
    let shared_cb = Arc::clone(&shared);
    app.on_settings_output_mode_changed(move |mode: i32| {
        shared_cb.with_ui(|app| {
            app.set_settings_output_mode(mode);
        });
        save_config_field(&shared_cb, |s| {
            s.output.mode = mode as u8;
        });
    });

    // Keyboard
    let shared_cb = Arc::clone(&shared);
    app.on_settings_start_recording_shortcut(move || {
        let _ = shared_cb;
        // TODO: implement shortcut recording
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_stop_recording_shortcut(move || {
        let _ = shared_cb;
        // TODO: implement shortcut recording stop + save
    });

    // Window management callbacks
    let app_weak_drag = app.as_weak();
    app.on_drag_window(move || {
        if let Some(app) = app_weak_drag.upgrade() {
            window::drag_window(&app);
        }
    });

    let app_weak_min = app.as_weak();
    app.on_minimize_window(move || {
        if let Some(app) = app_weak_min.upgrade() {
            window::minimize_window(&app);
        }
    });

    let app_weak_close = app.as_weak();
    app.on_close_window(move || {
        if let Some(app) = app_weak_close.upgrade() {
            window::hide_to_tray(&app);
        }
    });

    let app_weak_icon = app.as_weak();
    app.on_set_window_icon_theme(move |theme: slint::SharedString| {
        if let Some(app) = app_weak_icon.upgrade() {
            window::set_window_icon_theme(&app, &theme.to_string());
        }
    });

    // Initialize GTK on Linux (required by tray-icon/libappindicator before building the tray)
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

    // Set initial window (taskbar) icon to match theme
    window::set_window_icon_theme(&app, &initial_theme);

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

    // On Linux, pump the GLib main context on the main thread so that
    // libappindicator's D-Bus registration (from tray-icon) completes.
    #[cfg(target_os = "linux")]
    {
        let glib_timer = slint::Timer::default();
        glib_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(50),
            move || {
                let ctx = glib::MainContext::default();
                for _ in 0..10 {
                    if !ctx.iteration(false) {
                        break;
                    }
                }
            },
        );
        // Timer must outlive this block — forget prevents it from being dropped.
        std::mem::forget(glib_timer);
    }

    // Intercept native window close (Alt+F4, etc.) — hide to tray instead of quitting
    let app_weak_native_close = app.as_weak();
    app.window().on_close_requested(move || {
        if let Some(app) = app_weak_native_close.upgrade() {
            app.window().hide().ok();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });

    // Run the Slint event loop — use run_event_loop_until_quit so the app
    // stays alive in the tray even after the window is hidden.
    // (app.run() would exit as soon as the last window is hidden.)
    app.window().show().ok();
    slint::run_event_loop_until_quit().unwrap();

    // Clean up tray
    tray::cleanup_tray();
}
