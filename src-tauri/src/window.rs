use log::{debug, error, info};
use tauri::{AppHandle, Manager, PhysicalPosition};

use crate::config::ConfigManager;

/// Save the current window position and size to config.
pub fn save_window_position(app: &AppHandle, config: &ConfigManager) {
    if let Some(window) = app.get_webview_window("main") {
        match (window.outer_position(), window.outer_size()) {
            (Ok(pos), Ok(size)) => {
                config.set_window_position(Some(pos.x), Some(pos.y));
                let _ = config.save_settings();
                debug!(
                    "Saved window position: ({}, {}), size: ({}, {})",
                    pos.x, pos.y, size.width, size.height
                );
            }
            _ => {
                error!("Failed to get window position/size for saving");
            }
        }
    }
}

/// Restore window position from config.
pub fn restore_window_position(app: &AppHandle, config: &ConfigManager) {
    let (x, y) = config.get_window_position();

    if let (Some(x), Some(y)) = (x, y) {
        if let Some(window) = app.get_webview_window("main") {
            // Validate that the position is on a visible monitor
            if is_position_visible(app, x, y) {
                let _ = window.set_position(PhysicalPosition::new(x, y));
                info!("Restored window position: ({}, {})", x, y);
            } else {
                info!(
                    "Saved position ({}, {}) is off-screen, centering window",
                    x, y
                );
                let _ = window.center();
            }
        }
    } else {
        debug!("No saved window position, using default (center)");
    }
}

/// Check if the given position is visible on any connected monitor.
fn is_position_visible(app: &AppHandle, x: i32, y: i32) -> bool {
    if let Ok(monitors) = app.available_monitors() {
        for monitor in monitors {
            let pos = monitor.position();
            let size = monitor.size();
            if x >= pos.x
                && x < pos.x + size.width as i32
                && y >= pos.y
                && y < pos.y + size.height as i32
            {
                return true;
            }
        }
    }
    false
}

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
