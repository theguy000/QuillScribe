use log::{debug, error, info};
use tauri::{AppHandle, Manager};

use crate::config::ConfigManager;

/// Apply always-on-top setting.
pub fn apply_always_on_top(app: &AppHandle, on_top: bool) {
    if let Some(window) = app.get_webview_window("main") {
        match window.set_always_on_top(on_top) {
            Ok(_) => info!("Always-on-top set to {}", on_top),
            Err(e) => error!("Failed to set always-on-top to {}: {}", on_top, e),
        }
    } else {
        error!("Could not find main window for always-on-top");
    }
}

/// Apply the custom-titlebar toggle. Caller must drop the config lock first.
pub fn apply_custom_titlebar(app: &AppHandle, custom: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_decorations(!custom);
        debug!("Custom titlebar: {} (decorations: {})", custom, !custom);
    }
}

/// Set always-on-top and persist to config.
pub fn set_always_on_top(app: &AppHandle, config: &ConfigManager, on_top: bool) {
    config.set_always_on_top(on_top);
    let _ = config.save_settings();
    apply_always_on_top(app, on_top);
}
