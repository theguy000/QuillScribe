mod audio;
mod commands;
mod config;
mod hotkey;
mod output;
mod sound;
mod statistics;
mod tray;
mod updater;
mod whisper;
mod window;

use log::warn;
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use crate::audio::AudioDevice;
use slint::winit_030::WinitWindowAccessor;

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

const MIN_HISTORY_ENTRIES: usize = 1;
const MAX_HISTORY_ENTRIES: usize = 1000;
const HISTORY_SAVE_DEBOUNCE_MS: u64 = 500;

fn clamp_max_history_entries(value: i32) -> usize {
    (value
        .max(MIN_HISTORY_ENTRIES as i32)
        .min(MAX_HISTORY_ENTRIES as i32)) as usize
}

fn clamp_max_history_entries_usize(value: usize) -> usize {
    value.clamp(MIN_HISTORY_ENTRIES, MAX_HISTORY_ENTRIES)
}

fn theme_key_to_display(key: &str) -> &str {
    THEME_MAP
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, d)| *d)
        .unwrap_or("White")
}

fn theme_display_to_key(display: &str) -> &str {
    THEME_MAP
        .iter()
        .find(|(_, d)| *d == display)
        .map(|(k, _)| *k)
        .unwrap_or("white")
}

fn device_matches_blocklist(device_id: &str, blocklist: &[String]) -> bool {
    blocklist
        .iter()
        .any(|pattern| device_id.contains(pattern.as_str()))
}

fn matching_blocklist_patterns(device_id: &str, blocklist: &[String]) -> Vec<String> {
    blocklist
        .iter()
        .filter(|pattern| device_id.contains(pattern.as_str()))
        .cloned()
        .collect()
}

fn visible_audio_devices(all_devices: &[AudioDevice], blocklist: &[String]) -> Vec<AudioDevice> {
    all_devices
        .iter()
        .filter(|device| !device_matches_blocklist(&device.id, blocklist))
        .cloned()
        .collect()
}

fn all_audio_devices(shared: &SharedAppState) -> Vec<AudioDevice> {
    if let Ok(audio) = shared.state.audio.lock() {
        audio.get_available_devices(Vec::new())
    } else {
        Vec::new()
    }
}

fn blocklist_model(blocklist: &[String]) -> Rc<slint::VecModel<slint::SharedString>> {
    let blocklist_items: Vec<slint::SharedString> = blocklist
        .iter()
        .map(|item| slint::SharedString::from(item.as_str()))
        .collect();

    Rc::new(slint::VecModel::from(blocklist_items))
}

fn blocklist_device_model(
    all_devices: &[AudioDevice],
    blocklist: &[String],
) -> Rc<slint::VecModel<BlocklistDeviceEntry>> {
    let blocklist_device_items: Vec<BlocklistDeviceEntry> = all_devices
        .iter()
        .map(|device| {
            let matching_patterns = matching_blocklist_patterns(&device.id, blocklist);
            BlocklistDeviceEntry {
                name: device.name.as_str().into(),
                id: device.id.as_str().into(),
                blocked: !matching_patterns.is_empty(),
                blocked_by: matching_patterns.join(", ").as_str().into(),
            }
        })
        .collect();

    Rc::new(slint::VecModel::from(blocklist_device_items))
}

fn unblock_device_from_blocklist(
    target_id: &str,
    all_devices: &[AudioDevice],
    blocklist: &[String],
) -> Vec<String> {
    let mut updated_blocklist: Vec<String> = blocklist
        .iter()
        .filter(|pattern| !target_id.contains(pattern.as_str()))
        .cloned()
        .collect();

    for device in all_devices {
        if device.id == target_id {
            continue;
        }

        let was_blocked = device_matches_blocklist(&device.id, blocklist);
        let is_still_blocked = device_matches_blocklist(&device.id, &updated_blocklist);
        if was_blocked && !is_still_blocked && !updated_blocklist.contains(&device.id) {
            updated_blocklist.push(device.id.clone());
        }
    }

    updated_blocklist
}

use commands::AppState;

slint::include_modules!();

const HISTORY_DAYS: i64 = 30;
const HISTORY_SUMMARY_CHARS: usize = 140;
const OVERLAY_MINIMAL_WIDTH: f32 = 120.0;
const OVERLAY_MINIMAL_HEIGHT: f32 = 32.0;
const OVERLAY_FULL_WIDTH: f32 = 240.0;
const OVERLAY_FULL_HEIGHT: f32 = 48.0;

fn format_history_timestamp(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%b %-d, %-I:%M %p")
                .to_string()
        })
        .unwrap_or_else(|_| timestamp.to_string())
}

fn format_history_duration(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "-".to_string();
    }

    if seconds < 60.0 {
        return format!("{seconds:.1}s");
    }

    let total_seconds = seconds.round() as i64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let remaining_seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{remaining_seconds:02}")
    } else {
        format!("{minutes}:{remaining_seconds:02}")
    }
}

fn summarize_history_text(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "No transcription text".to_string();
    }

    let mut chars = normalized.chars();
    let summary: String = chars.by_ref().take(HISTORY_SUMMARY_CHARS).collect();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

fn overlay_mode_is_full(mode: &str) -> bool {
    mode == "full" || mode == "Full (bars, timer, stop button)"
}

fn recording_overlay_size(is_full: bool) -> (f32, f32) {
    if is_full {
        (OVERLAY_FULL_WIDTH, OVERLAY_FULL_HEIGHT)
    } else {
        (OVERLAY_MINIMAL_WIDTH, OVERLAY_MINIMAL_HEIGHT)
    }
}

fn show_recording_overlay(overlay: &RecordingOverlay, shared: &SharedAppState, elapsed_secs: i32) {
    let ui = {
        let config = shared.state.config.lock().unwrap();
        config.get_settings().ui
    };

    let is_full = overlay_mode_is_full(&ui.overlay_mode);
    overlay.set_current_theme(ui.theme.into());
    overlay.set_mode(ui.overlay_mode.into());
    overlay.set_composited_desktop(window::has_compositor());
    overlay.set_elapsed_seconds(elapsed_secs);
    overlay.set_timer_running(is_full);
    let (width, height) = recording_overlay_size(is_full);
    overlay
        .window()
        .set_size(slint::LogicalSize::new(width, height));

    window::apply_overlay_topmost(overlay.window());
    overlay.window().show().ok();
    window::apply_overlay_topmost(overlay.window());
    position_recording_overlay(overlay, is_full);
    window::harden_recording_overlay(overlay.window(), is_full, width, height);
}

fn position_recording_overlay(overlay: &RecordingOverlay, is_full: bool) {
    let (width, height) = recording_overlay_size(is_full);
    overlay.window().with_winit_window(|winit_win| {
        use slint::winit_030::winit::dpi::{LogicalPosition, LogicalSize};

        let _ = winit_win.request_inner_size(LogicalSize::new(width, height));
        let width = f64::from(width);
        let height = f64::from(height);
        let monitor = winit_win
            .current_monitor()
            .or_else(|| winit_win.primary_monitor())
            .or_else(|| winit_win.available_monitors().next());

        if let Some(monitor) = monitor {
            let scale = monitor.scale_factor();
            let position = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            let x = position.x + ((size.width - width) / 2.0).round();
            let y = position.y + (size.height - height - 60.0).round();
            winit_win.set_outer_position(LogicalPosition::new(x, y));
        }
    });
}

fn hide_recording_overlay(overlay: &RecordingOverlay) {
    overlay.set_timer_running(false);
    overlay.set_audio_level(0.0);
    overlay.window().hide().ok();
}

fn format_history_word_count(text: &str) -> String {
    let word_count = text.split_whitespace().count();
    format!(
        "{word_count} {}",
        if word_count == 1 { "word" } else { "words" }
    )
}

fn make_history_entries(entries: Vec<statistics::HistoryEntry>) -> Vec<HistoryListEntry> {
    entries
        .into_iter()
        .rev()
        .map(|entry| HistoryListEntry {
            timestamp: format_history_timestamp(&entry.timestamp).into(),
            mode: entry.mode.into(),
            status: if entry.success { "Success" } else { "Failed" }.into(),
            success: entry.success,
            duration_label: format_history_duration(entry.duration_secs).into(),
            transcription_label: format_history_duration(entry.transcription_time_secs).into(),
            text_length_label: format_history_word_count(&entry.text).into(),
            summary: summarize_history_text(&entry.text).into(),
            text: entry.text.into(),
        })
        .collect()
}

fn make_history_model(entries: Vec<HistoryListEntry>) -> slint::ModelRc<HistoryListEntry> {
    let model = std::rc::Rc::new(slint::VecModel::from(entries));
    slint::ModelRc::from(model)
}

fn get_recent_history_entries(state: &AppState, days: i64) -> (i32, Vec<HistoryListEntry>) {
    let history = state.statistics.get_recent_history(days);
    (history.len() as i32, make_history_entries(history))
}

fn apply_history_ui(app: &App, count: i32, entries: Vec<HistoryListEntry>) {
    app.set_session_count(count);
    app.set_history_entries(make_history_model(entries));
    app.set_history_loading(false);
}

fn refresh_history_ui(app: &App, state: &AppState, days: i64) {
    app.set_history_loading(true);
    app.set_history_error("".into());
    app.set_history_expanded_index(-1);

    let (count, entries) = get_recent_history_entries(state, days);
    apply_history_ui(app, count, entries);
}

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

    fn with_ui_ret<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(App) -> R,
    {
        if let Ok(lock) = self.app_weak.lock() {
            if let Some(weak) = lock.as_ref() {
                if let Some(app) = weak.upgrade() {
                    return Some(f(app));
                }
            }
        }
        None
    }
}

fn refresh_history_ui_async(
    shared: Arc<SharedAppState>,
    rt_handle: tokio::runtime::Handle,
    days: i64,
) {
    shared.with_ui(|app| {
        app.set_history_loading(true);
        app.set_history_error("".into());
        app.set_history_expanded_index(-1);
    });

    rt_handle.spawn(async move {
        let (count, entries) = get_recent_history_entries(&shared.state, days);

        let _ = slint::invoke_from_event_loop(move || {
            shared.with_ui(|app| {
                apply_history_ui(&app, count, entries);
            });
        });
    });
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

fn flush_pending_max_history_entries(
    shared: &SharedAppState,
    pending: &Mutex<Option<usize>>,
    generation: &AtomicU64,
) {
    let Some(value) = pending.lock().unwrap().take() else {
        return;
    };

    generation.fetch_add(1, Ordering::SeqCst);
    let clamped = clamp_max_history_entries_usize(value);
    save_config_field(shared, |s| {
        s.advanced.max_history_entries = clamped;
    });
}

/// Helper: refresh the blocklist model and audio device list in the UI.
/// Also checks if the currently configured device is still visible (not blocklisted)
/// and falls back to default if it isn't.
fn refresh_blocklist_and_devices(shared: &SharedAppState) {
    let (blocklist, current_device_id) = {
        let config = shared.state.config.lock().unwrap();
        (config.get_blocklist(), config.get_audio_device_id())
    };

    let all_devices = all_audio_devices(shared);
    let devices = visible_audio_devices(&all_devices, &blocklist);

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

    let mut names: Vec<slint::SharedString> = vec!["Default".into()];
    names.extend(
        devices
            .into_iter()
            .map(|d| slint::SharedString::from(d.name.as_str())),
    );
    let device_model = Rc::new(slint::VecModel::from(names));

    shared.with_ui(|app| {
        app.set_settings_audio_blocklist(slint::ModelRc::from(blocklist_model(&blocklist)));
        app.set_settings_audio_blocklist_devices(slint::ModelRc::from(blocklist_device_model(
            &all_devices,
            &blocklist,
        )));
        app.set_settings_audio_devices(slint::ModelRc::from(device_model));
        app.set_settings_audio_device_id(selected_name.into());
    });
}

pub fn run() {
    // Let Slint auto-detect the best renderer (femtovg with GPU if available)
    // Previously tried winit-skia-vulkan but the feature wasn't compiled in.

    // Initialize logging
    env_logger::init();

    // Create a Tokio runtime for async tasks (Slint's event loop is not a Tokio runtime)
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let rt_handle = rt.handle().clone();

    // Suppress ALSA error spam on Linux by installing a no-op global handler.
    // Also prevent JACK from trying to auto-start its server.
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("JACK_NO_START_SERVER", "1");

        extern "C" {
            fn quillscribe_silence_alsa_errors(
                file: *const libc::c_char,
                line: libc::c_int,
                function: *const libc::c_char,
                err: libc::c_int,
                fmt: *const libc::c_char,
                ...
            );
        }

        unsafe {
            let f: unsafe extern "C" fn(
                *const libc::c_char,
                libc::c_int,
                *const libc::c_char,
                libc::c_int,
                *const libc::c_char,
                ...
            ) = quillscribe_silence_alsa_errors;
            let _ = alsa_sys::snd_lib_error_set_handler(Some(f));
        }
    }

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
        let max_history = clamp_max_history_entries_usize(config.get_max_history_entries());
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
    let recording_overlay = RecordingOverlay::new().unwrap();
    window::apply_overlay_topmost(recording_overlay.window());
    recording_overlay.window().hide().ok();

    // Store weak handle for callbacks that need to update UI
    {
        let mut lock = shared.app_weak.lock().unwrap();
        *lock = Some(app.as_weak());
    }

    // ── Initialize UI from persisted config ──────────────────────────────
    let s = {
        let config = shared.state.config.lock().unwrap();
        config.get_settings()
    };
    let is_overlay_full = overlay_mode_is_full(&s.ui.overlay_mode);
    let (overlay_width, overlay_height) = recording_overlay_size(is_overlay_full);
    window::harden_recording_overlay(
        recording_overlay.window(),
        is_overlay_full,
        overlay_width,
        overlay_height,
    );

    // Audio devices, blocklist, and device selection
    refresh_blocklist_and_devices(&shared);
    app.set_settings_sounds_enabled(s.audio.sounds_enabled);

    // Whisper / Engine
    app.set_settings_whisper_mode(s.whisper.mode.clone().into());
    app.set_settings_api_provider(s.whisper.api_provider.clone().into());
    app.set_settings_api_key(s.whisper.api_key.clone().into());
    app.set_settings_groq_api_key(s.whisper.groq_api_key.clone().into());
    app.set_settings_api_model(s.whisper.api_model.clone().into());
    app.set_settings_api_language(s.whisper.api_language.clone().into());
    app.set_settings_local_model(s.whisper.local_model.clone().into());

    // Initialize local model categories and models
    let categories = whisper::WhisperManager::get_local_model_categories();
    let category_strings: Vec<slint::SharedString> = categories.iter().map(|c| c.into()).collect();
    let category_rc = std::rc::Rc::new(slint::VecModel::from(category_strings));
    app.set_settings_local_model_categories(slint::ModelRc::from(category_rc));

    let initial_category = "General";
    let models = whisper::WhisperManager::get_local_models_for_category(initial_category);
    let model_strings: Vec<slint::SharedString> = models.iter().map(|m| m.into()).collect();
    let model_rc = std::rc::Rc::new(slint::VecModel::from(model_strings));
    app.set_settings_local_models(slint::ModelRc::from(model_rc));
    app.set_settings_local_model_category(initial_category.into());

    let max_history_entries = clamp_max_history_entries_usize(s.advanced.max_history_entries);
    if max_history_entries != s.advanced.max_history_entries {
        save_config_field(&shared, |settings| {
            settings.advanced.max_history_entries = max_history_entries;
        });
        shared
            .state
            .statistics
            .set_max_history_entries(max_history_entries);
    }

    // UI
    let theme_display = theme_key_to_display(&s.ui.theme);
    app.set_settings_theme(theme_display.into());
    app.set_current_theme(s.ui.theme.clone().into());
    app.set_settings_custom_titlebar(s.ui.custom_titlebar);
    app.set_settings_always_on_top(s.ui.always_on_top);
    app.set_settings_overlay_mode(s.ui.overlay_mode.clone().into());
    app.set_settings_max_history_entries(max_history_entries as i32);
    app.set_settings_app_version(env!("CARGO_PKG_VERSION").into());
    app.set_settings_is_linux(cfg!(target_os = "linux"));
    app.set_settings_update_can_install(false);
    app.set_settings_update_install_message("".into());
    window::apply_custom_titlebar(&app, s.ui.custom_titlebar);
    window::apply_always_on_top(&app, s.ui.always_on_top);
    refresh_history_ui(&app, &shared.state, HISTORY_DAYS);

    // ── Wire core Slint callbacks ────────────────────────────────────────

    let app_weak_overlay_stop = app.as_weak();
    recording_overlay.on_stop_recording(move || {
        if let Some(app) = app_weak_overlay_stop.upgrade() {
            if app.get_is_recording() {
                app.invoke_toggle_recording();
            }
        }
    });

    let overlay_weak_drag = recording_overlay.as_weak();
    recording_overlay.on_drag_overlay(move || {
        if let Some(overlay) = overlay_weak_drag.upgrade() {
            overlay.window().with_winit_window(|winit_win| {
                if let Err(e) = winit_win.drag_window() {
                    log::warn!("overlay drag_window failed: {}", e);
                }
            });
        }
    });

    let shared_toggle = Arc::clone(&shared);
    let rt_handle_toggle = rt_handle.clone();
    let overlay_toggle = recording_overlay.as_weak();
    app.on_toggle_recording(move || {
        let rt_handle = rt_handle_toggle.clone();
        let s = Arc::clone(&shared_toggle);

        // Read current state on the UI thread (callback runs on Slint event loop)
        let is_recording = s.with_ui_ret(|app| app.get_is_recording()).unwrap_or(false);

        if !is_recording {
            // Start recording synchronously on the UI thread before presenting it as active.
            let start_result = s
                .state
                .audio
                .lock()
                .map_err(|e| format!("Audio device lock failed: {}", e))
                .and_then(|mut audio| audio.start_recording().map_err(|e| e.to_string()));

            if let Err(e) = start_result {
                log::error!("Failed to start recording: {}", e);
                s.with_ui(|app| {
                    app.set_is_recording(false);
                    app.set_status_message(format!("Error: {}", e).into());
                });
                return;
            }

            s.state.sound.play_start_sound();
            s.with_ui(|app| {
                app.set_is_recording(true);
                app.set_status_message("Recording...".into());
            });
            if let Some(overlay) = overlay_toggle.upgrade() {
                show_recording_overlay(&overlay, &s, 0);
            }
        } else {
            // Stop recording — synchronous UI update on UI thread
            s.with_ui(|app| {
                app.set_is_recording(false);
                app.set_is_transcribing(true);
                app.set_status_message("Transcribing...".into());
            });
            if let Some(overlay) = overlay_toggle.upgrade() {
                hide_recording_overlay(&overlay);
            }

            let (audio_data, sample_rate) = {
                let mut audio = s.state.audio.lock().unwrap();
                match audio.stop_recording() {
                    Ok(Some((data, sr))) => (data, sr),
                    Ok(None) => {
                        s.with_ui(|app| {
                            app.set_is_transcribing(false);
                            app.set_status_message("No speech detected".into());
                        });
                        return;
                    }
                    Err(e) => {
                        log::error!("Failed to stop recording: {}", e);
                        s.with_ui(|app| {
                            app.set_is_transcribing(false);
                            app.set_status_message(format!("Error: {}", e).into());
                        });
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
            rt_handle.spawn(async move {
                let result = whisper_clone
                    .transcribe_audio(audio_data, sample_rate)
                    .await;
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
                            let _ = output.process_transcription(
                                &text,
                                output::OutputMode::from(output_cfg.mode),
                            );
                        }

                        // Update UI from the Slint event loop thread
                        let text_for_ui = text.clone();
                        let s_invoke = Arc::clone(&s_async);
                        let _ = slint::invoke_from_event_loop(move || {
                            s_invoke.with_ui(|app| {
                                app.set_is_transcribing(false);
                                app.set_transcription_text(text_for_ui.into());
                                app.set_status_message("Transcription complete".into());
                                refresh_history_ui(&app, &s_invoke.state, HISTORY_DAYS);

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
                            });
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
                        let s_invoke = Arc::clone(&s_async);
                        let _ = slint::invoke_from_event_loop(move || {
                            s_invoke.with_ui(|app| {
                                app.set_is_transcribing(false);
                                app.set_status_message(err_msg.into());
                                refresh_history_ui(&app, &s_invoke.state, HISTORY_DAYS);
                            });
                        });
                    }
                }
            });
        }
    });

    let shared_nav = Arc::clone(&shared);
    let rt_handle_history_nav = rt_handle.clone();
    app.on_request_navigate(move |panel: slint::SharedString| {
        // Update active panel so the UI switches views
        let should_load_history = panel == "history";
        shared_nav.with_ui(|app| {
            app.set_active_panel(panel.clone());
        });
        if should_load_history {
            refresh_history_ui_async(
                Arc::clone(&shared_nav),
                rt_handle_history_nav.clone(),
                HISTORY_DAYS,
            );
        }
    });

    app.on_copy_to_clipboard(move |text: slint::SharedString| {
        let text = text.to_string();
        std::thread::spawn(move || {
            if let Err(e) = output::OutputManager::copy_text_to_clipboard(&text) {
                log::error!("Failed to copy history text to clipboard: {}", e);
            }
        });
    });

    let shared_load = Arc::clone(&shared);
    let rt_handle_history_load = rt_handle.clone();
    app.on_load_history(move || {
        refresh_history_ui_async(
            Arc::clone(&shared_load),
            rt_handle_history_load.clone(),
            HISTORY_DAYS,
        );
    });

    let shared_update = Arc::clone(&shared);
    let rt_handle_update = rt_handle.clone();
    app.on_check_for_update(move || {
        shared_update.with_ui(|app| {
            app.set_settings_update_checking(true);
            app.set_settings_update_downloading(false);
            app.set_settings_update_progress(0.0);
            app.set_settings_update_notes("".into());
            app.set_settings_update_can_install(false);
            app.set_settings_update_install_message("".into());
            app.set_status_message("Checking for updates...".into());
        });

        let shared = Arc::clone(&shared_update);
        rt_handle_update.spawn(async move {
            let result = tokio::task::spawn_blocking(updater::check_for_update).await;
            let result = match result {
                Ok(result) => result,
                Err(e) => Err(format!("Update check task failed: {e}")),
            };

            let _ = slint::invoke_from_event_loop(move || {
                shared.with_ui(|app| {
                    app.set_settings_update_checking(false);
                    match result {
                        Ok(Some(update)) => {
                            log::info!(
                                "Update available: version {}, asset {}, url {}",
                                update.version,
                                update.asset_name,
                                update.download_url
                            );
                            let notes = if update.notes.trim().is_empty() {
                                format!("Asset: {}", update.asset_name)
                            } else {
                                update.notes
                            };
                            app.set_has_update(true);
                            app.set_settings_update_version(update.version.into());
                            app.set_settings_update_notes(notes.into());
                            app.set_settings_update_progress(0.0);
                            app.set_settings_update_can_install(update.can_install);
                            app.set_settings_update_install_message(update.install_hint.into());
                            app.set_status_message("Update available".into());
                        }
                        Ok(None) => {
                            app.set_has_update(false);
                            app.set_settings_update_version("".into());
                            app.set_settings_update_notes("".into());
                            app.set_settings_update_progress(0.0);
                            app.set_settings_update_can_install(false);
                            app.set_settings_update_install_message("".into());
                            app.set_status_message("QuillScribe is up to date".into());
                        }
                        Err(error) => {
                            app.set_has_update(false);
                            app.set_settings_update_version("".into());
                            app.set_settings_update_notes(
                                format!("Update check failed: {error}").into(),
                            );
                            app.set_settings_update_can_install(false);
                            app.set_settings_update_install_message("".into());
                            app.set_status_message("Update check failed".into());
                        }
                    }
                });
            });
        });
    });

    let shared_install = Arc::clone(&shared);
    let rt_handle_install = rt_handle.clone();
    app.on_install_update(move || {
        shared_install.with_ui(|app| {
            app.set_settings_update_downloading(true);
            app.set_settings_update_progress(0.0);
            app.set_status_message("Downloading update...".into());
        });

        let shared = Arc::clone(&shared_install);
        rt_handle_install.spawn(async move {
            let progress_shared = Arc::clone(&shared);
            let result = updater::install_update(move |progress| {
                let progress_shared = Arc::clone(&progress_shared);
                let progress = progress.round().clamp(0.0, 100.0);
                let _ = slint::invoke_from_event_loop(move || {
                    progress_shared.with_ui(|app| {
                        app.set_settings_update_progress(progress);
                    });
                });
            })
            .await;

            let _ = slint::invoke_from_event_loop(move || {
                shared.with_ui(|app| {
                    app.set_settings_update_downloading(false);
                    match result {
                        Ok(version) => {
                            app.set_settings_update_progress(100.0);
                            app.set_settings_update_version(version.into());
                            app.set_settings_update_notes(
                                "Update installed. Restart QuillScribe to finish.".into(),
                            );
                            app.set_status_message(
                                "Update installed. Restart QuillScribe to finish.".into(),
                            );
                        }
                        Err(error) => {
                            app.set_settings_update_notes(
                                format!("Update install failed: {error}").into(),
                            );
                            app.set_status_message("Update install failed".into());
                        }
                    }
                });
            });
        });
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
                audio
                    .get_available_devices(blocklist)
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
        shared_cb.with_ui(|app| app.set_settings_loading_devices(true));
        refresh_blocklist_and_devices(&shared_cb);
        shared_cb.with_ui(|app| app.set_settings_loading_devices(false));
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
        let item_str = item.trim().to_string();
        if item_str.is_empty() {
            return;
        }

        save_config_field(&shared_cb, |s| {
            if !s.audio.blocklist.contains(&item_str) {
                s.audio.blocklist.push(item_str);
            }
        });

        refresh_blocklist_and_devices(&shared_cb);
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_unhide_blocklist_device(move |device_id: slint::SharedString| {
        let target_id = device_id.to_string();
        if target_id.is_empty() {
            return;
        }

        let all_devices = all_audio_devices(&shared_cb);

        save_config_field(&shared_cb, |s| {
            if !device_matches_blocklist(&target_id, &s.audio.blocklist) {
                return;
            }

            s.audio.blocklist =
                unblock_device_from_blocklist(&target_id, &all_devices, &s.audio.blocklist);
        });

        refresh_blocklist_and_devices(&shared_cb);
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_remove_blocklist_item(move |idx: i32| {
        if idx < 0 {
            return;
        }

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
            app.set_settings_api_models(slint::ModelRc::from(model_rc));
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
    app.on_settings_category_changed(move |category: slint::SharedString| {
        let category_str = category.to_string();
        let models = whisper::WhisperManager::get_local_models_for_category(&category_str);
        let model_strings: Vec<slint::SharedString> = models.iter().map(|m| m.into()).collect();
        let model_rc = std::rc::Rc::new(slint::VecModel::from(model_strings));
        shared_cb.with_ui(|app| {
            app.set_settings_local_models(slint::ModelRc::from(model_rc));
            // Select the first model in the filtered list
            if let Some(first_model) = models.first() {
                app.set_settings_local_model(first_model.clone().into());
            }
        });
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
                app.set_settings_download_progress(0.0);
                app.set_settings_download_error("".into());
            });
            let model_name = {
                let config = s.state.config.lock().unwrap();
                config.get_whisper().local_model
            };
            let s2 = Arc::clone(&s);
            let s3 = Arc::clone(&s);
            let s4 = Arc::clone(&s);
            rt_handle.spawn(async move {
                match whisper::WhisperManager::download_model_with_progress(
                    &model_name,
                    move |downloaded, total| {
                        let progress = if total > 0 {
                            (downloaded as f32 / total as f32).min(1.0)
                        } else {
                            0.0
                        };
                        let s5 = Arc::clone(&s4);
                        let _ = slint::invoke_from_event_loop(move || {
                            s5.with_ui(|app| {
                                app.set_settings_download_progress(progress);
                            });
                        });
                    },
                )
                .await
                {
                    Ok(_) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            s2.with_ui(|app| {
                                app.set_settings_downloading_model(false);
                                app.set_settings_model_downloaded(true);
                                app.set_settings_download_progress(1.0);
                            });
                        });
                    }
                    Err(e) => {
                        let err = format!("{}", e);
                        let _ = slint::invoke_from_event_loop(move || {
                            s3.with_ui(|app| {
                                app.set_settings_downloading_model(false);
                                app.set_settings_download_error(err.into());
                                app.set_settings_download_progress(0.0);
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
            app.set_current_theme(key.clone().into());
        });
        tray::set_tray_theme(&key);
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_custom_titlebar_changed(move |custom: bool| {
        save_config_field(&shared_cb, |s| {
            s.ui.custom_titlebar = custom;
        });
        shared_cb.with_ui(|app| {
            window::apply_custom_titlebar(&app, custom);
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_always_on_top_changed(move |on_top: bool| {
        save_config_field(&shared_cb, |s| {
            s.ui.always_on_top = on_top;
        });
        shared_cb.with_ui(|app| {
            window::apply_always_on_top(&app, on_top);
        });
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_overlay_mode_changed(move |mode: slint::SharedString| {
        let mode_str = mode.to_string();
        save_config_field(&shared_cb, |s| {
            s.ui.overlay_mode = mode_str;
        });
    });

    let max_history_save_generation = Arc::new(AtomicU64::new(0));
    let pending_max_history_entries = Arc::new(Mutex::new(None::<usize>));

    let shared_cb = Arc::clone(&shared);
    let rt_handle_cb = rt_handle.clone();
    let generation_cb = Arc::clone(&max_history_save_generation);
    let pending_cb = Arc::clone(&pending_max_history_entries);
    app.on_settings_max_history_entries_changed(move |entries: i32| {
        let clamped = clamp_max_history_entries(entries);
        if entries != clamped as i32 {
            shared_cb.with_ui(|app| {
                app.set_settings_max_history_entries(clamped as i32);
            });
        }

        let history_changed = shared_cb.state.statistics.set_max_history_entries(clamped);
        if history_changed {
            shared_cb.with_ui(|app| {
                if app.get_active_panel() == "history" {
                    refresh_history_ui(&app, &shared_cb.state, HISTORY_DAYS);
                }
            });
        }
        *pending_cb.lock().unwrap() = Some(clamped);

        let generation = generation_cb.fetch_add(1, Ordering::SeqCst) + 1;
        let shared_save = Arc::clone(&shared_cb);
        let generation_save = Arc::clone(&generation_cb);
        let pending_save = Arc::clone(&pending_cb);
        rt_handle_cb.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(HISTORY_SAVE_DEBOUNCE_MS)).await;
            if generation_save.load(Ordering::SeqCst) != generation {
                return;
            }

            let Some(value) = pending_save.lock().unwrap().take() else {
                return;
            };
            let clamped = clamp_max_history_entries_usize(value);
            save_config_field(&shared_save, |s| {
                s.advanced.max_history_entries = clamped;
            });
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

    // Shortcut
    let shared_cb = Arc::clone(&shared);
    app.on_settings_start_recording_shortcut(move || {
        shared_cb.with_ui(|app| {
            app.set_settings_recording_shortcut(true);
        });

        let shared_cb2 = Arc::clone(&shared_cb);
        if let Err(e) = hotkey::start_shortcut_recording(move |shortcut| {
            let shortcut_str = shortcut.clone();
            let shared_for_ui = Arc::clone(&shared_cb2);
            let _ = slint::invoke_from_event_loop(move || {
                if let Err(e) = hotkey::reregister_hotkey(&shortcut_str) {
                    log::error!("Failed to re-register hotkey: {}", e);
                    shared_for_ui.with_ui(|app| {
                        app.set_settings_recording_shortcut(false);
                    });
                    return;
                }
                shared_for_ui.with_ui(|app| {
                    app.set_settings_record_toggle_shortcut(shortcut.into());
                    app.set_settings_recording_shortcut(false);
                });
                save_config_field(&shared_for_ui, |s| {
                    s.shortcuts.record_toggle = shortcut_str.clone();
                });
            });
        }) {
            log::error!("Failed to start shortcut recording: {}", e);
            shared_cb.with_ui(|app| {
                app.set_settings_recording_shortcut(false);
            });
        }
    });

    let shared_cb = Arc::clone(&shared);
    app.on_settings_stop_recording_shortcut(move || {
        shared_cb.with_ui(|app| {
            app.set_settings_recording_shortcut(false);
        });
        hotkey::stop_shortcut_recording();
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
    let shared_close = Arc::clone(&shared);
    let generation_close = Arc::clone(&max_history_save_generation);
    let pending_close = Arc::clone(&pending_max_history_entries);
    app.on_close_window(move || {
        flush_pending_max_history_entries(&shared_close, &pending_close, &generation_close);
        if let Some(app) = app_weak_close.upgrade() {
            window::hide_to_tray(&app);
        }
    });

    let app_weak_icon = app.as_weak();
    app.on_set_window_icon_theme(move |theme: slint::SharedString| {
        if let Some(app) = app_weak_icon.upgrade() {
            window::set_window_icon_theme(&app, theme.as_ref());
        }
    });

    // Set up system tray
    if let Err(e) = tray::setup_tray(&initial_theme, &app.as_weak()) {
        warn!("Failed to set up system tray: {}", e);
    }

    // Set up global hotkey
    let record_toggle_shortcut = {
        let config = shared.state.config.lock().unwrap();
        config.get_record_toggle()
    };
    app.set_settings_record_toggle_shortcut(record_toggle_shortcut.clone().into());
    hotkey::register_record_toggle(&app.as_weak(), &record_toggle_shortcut);

    // Set initial window (taskbar) icon to match theme
    window::set_window_icon_theme(&app, &initial_theme);

    // Audio level polling timer
    let shared_audio = Arc::clone(&shared);
    let overlay_audio = recording_overlay.as_weak();
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
            if let Some(overlay) = overlay_audio.upgrade() {
                overlay.set_audio_level(level);
            }
        },
    );

    // Intercept native window close (Alt+F4, etc.) — hide to tray instead of quitting
    let app_weak_native_close = app.as_weak();
    let shared_native_close = Arc::clone(&shared);
    let generation_native_close = Arc::clone(&max_history_save_generation);
    let pending_native_close = Arc::clone(&pending_max_history_entries);
    app.window().on_close_requested(move || {
        flush_pending_max_history_entries(
            &shared_native_close,
            &pending_native_close,
            &generation_native_close,
        );
        if let Some(app) = app_weak_native_close.upgrade() {
            app.window().hide().ok();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });

    // Run the Slint event loop — use run_event_loop_until_quit so the app
    // stays alive in the tray even after the window is hidden.
    // (app.run() would exit as soon as the last window is hidden.)
    app.window().show().ok();
    window::apply_always_on_top(&app, s.ui.always_on_top);
    slint::run_event_loop_until_quit().unwrap();

    flush_pending_max_history_entries(
        &shared,
        &pending_max_history_entries,
        &max_history_save_generation,
    );

    // Clean up tray
    tray::cleanup_tray();
}
