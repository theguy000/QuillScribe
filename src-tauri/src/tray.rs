use log::{debug, info};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

/// Sets up the system tray icon with a context menu.
pub fn setup_tray(app: &AppHandle) -> Result<TrayIcon, Box<dyn std::error::Error>> {
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

    let tray = TrayIconBuilder::new()
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

    info!("System tray initialized");
    Ok(tray)
}
