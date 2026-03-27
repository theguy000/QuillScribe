use log::{debug, info, warn};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

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

/// Sets up the system tray icon with a context menu.
pub fn setup_tray(
    app: &AppHandle,
    theme: &str,
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    // Create menu items
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
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &start_item,
            &stop_item,
            &settings_item,
            &quit_item,
        ],
    )?;

    let icon = tauri::image::Image::from_bytes(icon_bytes_for_theme(theme))?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("QuillScribe - Voice to Text")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            debug!("Tray menu event: {}", id);
            match id {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "start_recording" => {
                    let _ = app.emit("tray-start-recording", ());
                }
                "stop_recording" => {
                    let _ = app.emit("tray-stop-recording", ());
                }
                "settings" => {
                    let _ = app.emit("tray-open-settings", ());
                }
                "quit" => {
                    info!("Quit requested from tray");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;

    info!("System tray initialized with theme: {}", theme);
    Ok(tray)
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
