use log::{debug, error, info, warn};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::commands::AppState;

/// Register the global hotkey for record toggle based on config settings.
/// This should be called during setup and whenever the shortcut changes.
pub fn register_record_toggle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let shortcut_str = {
        let config = match state.config.lock() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to lock config for hotkey registration: {}", e);
                return;
            }
        };
        config.get_record_toggle()
    };

    if shortcut_str.is_empty() {
        info!("No record toggle shortcut configured, skipping registration");
        return;
    }

    // Unregister any existing shortcuts first
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!("Failed to unregister existing shortcuts: {}", e);
    }

    // Convert the shortcut string format if needed
    // Python format: "Meta+Shift+`" -> Tauri format: "Super+Shift+`"
    let tauri_shortcut = convert_shortcut_format(&shortcut_str);

    let parsed: Shortcut = match tauri_shortcut.parse() {
        Ok(s) => s,
        Err(e) => {
            error!(
                "Failed to parse shortcut '{}': {}. Falling back to default.",
                tauri_shortcut, e
            );
            match "Super+Shift+Space".parse() {
                Ok(s) => s,
                Err(e2) => {
                    error!("Failed to parse fallback shortcut: {}", e2);
                    return;
                }
            }
        }
    };

    match app
        .global_shortcut()
        .on_shortcut(parsed, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            debug!("Global hotkey triggered: record toggle");
            let _ = app.emit("hotkey-record-toggle", ());
        }) {
        Ok(_) => {
            info!(
                "Registered global shortcut: {} (tauri: {})",
                shortcut_str, tauri_shortcut
            );
        }
        Err(e) => {
            error!(
                "Failed to register global shortcut '{}': {}",
                tauri_shortcut, e
            );
        }
    }
}

/// Convert Python-style shortcut format to Tauri-compatible format.
/// Python uses "Meta" for the Windows key, Tauri uses "Super".
/// Python uses "Ctrl", Tauri uses "Control" or "Ctrl" (both work).
fn convert_shortcut_format(shortcut: &str) -> String {
    shortcut
        .replace("Meta+", "Super+")
        .replace("meta+", "Super+")
}
