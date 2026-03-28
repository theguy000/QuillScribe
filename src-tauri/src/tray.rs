use log::{debug, info, warn};
use tauri::{
    menu::{CheckMenuItemBuilder, Menu, MenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::commands::AppState;
use crate::whisper::WhisperManager;

const TRAY_ID: &str = "main-tray";

fn icon_bytes_for_theme(theme: &str) -> &'static [u8] {
    match theme {
        "white" => include_bytes!("../icons/tray/white.png"),
        "warm_gray" => include_bytes!("../icons/tray/warm_gray.png"),
        "soft_beige" => include_bytes!("../icons/tray/soft_beige.png"),
        "blue_gray" => include_bytes!("../icons/tray/blue_gray.png"),
        "warm_taupe" => include_bytes!("../icons/tray/warm_taupe.png"),
        "soft_sage" => include_bytes!("../icons/tray/soft_sage.png"),
        "dark_charcoal" => include_bytes!("../icons/tray/dark_charcoal.png"),
        "dark_blue" => include_bytes!("../icons/tray/dark_blue.png"),
        "dark_purple" => include_bytes!("../icons/tray/dark_purple.png"),
        "dark_forest" => include_bytes!("../icons/tray/dark_forest.png"),
        "dark_burgundy" => include_bytes!("../icons/tray/dark_burgundy.png"),
        "obsidian" => include_bytes!("../icons/tray/obsidian.png"),
        _ => include_bytes!("../icons/tray/white.png"),
    }
}

/// Build the tray context menu, including the "Use Model" submenu.
fn build_tray_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "Show QuillScribe", true, None::<&str>)?;
    let start_item = MenuItem::with_id(
        app,
        "start_recording",
        "Start Recording",
        true,
        None::<&str>,
    )?;
    let stop_item =
        MenuItem::with_id(app, "stop_recording", "Stop Recording", false, None::<&str>)?;

    // ── Use Model submenu ────────────────────────────────────────────────
    let model_submenu = build_model_submenu(app)?;

    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &start_item,
            &stop_item,
            &model_submenu,
            &settings_item,
            &quit_item,
        ],
    )?;

    Ok(menu)
}

/// Build the "Use Model" submenu from current config state.
fn build_model_submenu(
    app: &AppHandle,
) -> Result<tauri::menu::Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    // Read whisper config from app state.
    let (current_mode, current_api_model, current_local_model) = {
        let state = app.state::<AppState>();
        let config = state.config.lock().map_err(|e| format!("{}", e))?;
        let whisper = config.get_whisper();
        (whisper.mode, whisper.api_model, whisper.local_model)
    };

    let api_models = WhisperManager::get_available_api_models();
    let downloaded_local_models = WhisperManager::get_downloaded_models();

    let mut submenu = SubmenuBuilder::new(app, "Use Model");

    // ── API models section ───────────────────────────────────────────────
    // Header item (disabled, acts as section label)
    let api_header =
        MenuItem::with_id(app, "_header_api", "-- API Models --", false, None::<&str>)?;
    submenu = submenu.item(&api_header);

    for model in &api_models {
        let is_active = current_mode == "api" && *model == current_api_model;
        let id = format!("model:api:{}", model);
        let item = CheckMenuItemBuilder::new(model)
            .id(id)
            .checked(is_active)
            .build(app)?;
        submenu = submenu.item(&item);
    }

    // ── Local models section ─────────────────────────────────────────────
    if !downloaded_local_models.is_empty() {
        let local_header = MenuItem::with_id(
            app,
            "_header_local",
            "-- Local Models --",
            false,
            None::<&str>,
        )?;
        submenu = submenu.item(&local_header);

        for model in &downloaded_local_models {
            let is_active = current_mode == "local" && *model == current_local_model;
            let id = format!("model:local:{}", model);
            let item = CheckMenuItemBuilder::new(model)
                .id(id)
                .checked(is_active)
                .build(app)?;
            submenu = submenu.item(&item);
        }
    }

    Ok(submenu.build()?)
}

/// Handle a model selection from the tray submenu.
///
/// `menu_id` has the form `model:{mode}:{model_name}`.
fn handle_model_selection(app: &AppHandle, menu_id: &str) {
    // Parse "model:api:gpt-4o-transcribe" or "model:local:base"
    let parts: Vec<&str> = menu_id.splitn(3, ':').collect();
    if parts.len() != 3 {
        warn!("Unexpected model menu id format: {}", menu_id);
        return;
    }
    let mode = parts[1];
    let model_name = parts[2];

    debug!("Tray model selection: mode={}, model={}", mode, model_name);

    // Update the config.
    let result: Result<(), String> = (|| {
        let state = app.state::<AppState>();
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let mut settings = config.get_settings();

        settings.whisper.mode = mode.to_string();
        match mode {
            "api" => {
                settings.whisper.api_model = model_name.to_string();
            }
            "local" => {
                settings.whisper.local_model = model_name.to_string();
            }
            _ => return Err(format!("Unknown mode: {}", mode)),
        }

        config.set_settings(settings);
        config.save_settings().map_err(|e| e.to_string())?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            info!(
                "Model switched via tray: mode={}, model={}",
                mode, model_name
            );
            // Rebuild the tray menu to update check marks.
            if let Err(e) = rebuild_tray_menu(app) {
                warn!("Failed to rebuild tray menu after model change: {}", e);
            }
            // Notify the frontend so it can reload settings.
            let _ = app.emit("tray-model-changed", ());
        }
        Err(e) => {
            warn!("Failed to switch model via tray: {}", e);
        }
    }
}

/// Sets up the system tray icon with a context menu.
pub fn setup_tray(app: &AppHandle, theme: &str) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app)?;
    let icon = tauri::image::Image::from_bytes(icon_bytes_for_theme(theme))?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("QuillScribe - Voice to Text")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            debug!("Tray menu event: {}", id);

            // Handle model submenu items.
            if id.starts_with("model:") {
                handle_model_selection(app, id);
                return;
            }

            match id {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "start_recording" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                    }
                    let _ = app.emit("tray-start-recording", ());
                }
                "stop_recording" => {
                    let _ = app.emit("tray-stop-recording", ());
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    let _ = app.emit("tray-open-settings", ());
                }
                "quit" => {
                    info!("Quit requested from tray");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    info!("System tray initialized with theme: {}", theme);
    Ok(tray)
}

/// Rebuild the tray context menu (e.g. after model download/delete/settings change).
pub fn rebuild_tray_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = build_tray_menu(app)?;
        tray.set_menu(Some(menu))?;
        debug!("Tray menu rebuilt");
    }
    Ok(())
}

/// Updates the tray icon to match the given theme.
pub fn set_tray_theme(app: &AppHandle, theme: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match tauri::image::Image::from_bytes(icon_bytes_for_theme(theme)) {
            Ok(icon) => {
                if let Err(e) = tray.set_icon(Some(icon)) {
                    warn!("Failed to set tray icon: {}", e);
                } else {
                    debug!("Tray icon updated for theme: {}", theme);
                }
            }
            Err(e) => warn!("Failed to decode tray icon for theme {}: {}", theme, e),
        }
    }
}

/// Updates the window/taskbar icon to match the given theme.
pub fn set_window_icon_theme(app: &AppHandle, theme: &str) {
    if let Some(window) = app.get_webview_window("main") {
        match tauri::image::Image::from_bytes(icon_bytes_for_theme(theme)) {
            Ok(icon) => {
                if let Err(e) = window.set_icon(icon) {
                    warn!("Failed to set window icon: {}", e);
                } else {
                    debug!("Window icon updated for theme: {}", theme);
                }
            }
            Err(e) => warn!("Failed to decode window icon for theme {}: {}", theme, e),
        }
    }
}
