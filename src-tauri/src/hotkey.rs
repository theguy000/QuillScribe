use log::{debug, error, info, warn};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use rdev::{listen, Event, EventType, Button};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::thread;

use crate::commands::AppState;

static MOUSE_SHORTCUT: Lazy<Mutex<Option<Button>>> = Lazy::new(|| Mutex::new(None));
static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));
static RDEV_LISTENER_STARTED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static RECORDING_MOUSE_SHORTCUT: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

pub fn start_mouse_shortcut_recording() {
    *RECORDING_MOUSE_SHORTCUT.lock().unwrap() = true;
    start_rdev_listener_if_needed();
}

pub fn stop_mouse_shortcut_recording() {
    *RECORDING_MOUSE_SHORTCUT.lock().unwrap() = false;
}

fn start_rdev_listener_if_needed() {
    let mut started = RDEV_LISTENER_STARTED.lock().unwrap();
    if *started {
        return;
    }
    *started = true;

    thread::spawn(|| {
        if let Err(error) = listen(rdev_callback) {
            error!("Error starting rdev listener: {:?}", error);
        }
    });
}

fn rdev_callback(event: Event) {
    if let EventType::ButtonPress(button) = event.event_type {
        // If we are currently recording a shortcut, emit the recorded button and stop recording
        let is_recording = { *RECORDING_MOUSE_SHORTCUT.lock().unwrap() };
        if is_recording {
            // Only accept extra mouse buttons (ignore left/right click for safety)
            let is_valid = match button {
                Button::Left | Button::Right => false,
                _ => true,
            };

            if is_valid {
                if let Some(app) = APP_HANDLE.lock().unwrap().as_ref() {
                    let shortcut_str = format!("{:?}", button);
                    debug!("Recorded mouse shortcut: {}", shortcut_str);
                    let _ = app.emit("mouse-shortcut-recorded", shortcut_str);
                    *RECORDING_MOUSE_SHORTCUT.lock().unwrap() = false;
                }
            }
            return; // Don't trigger the actual toggle while recording
        }

        // Normal hotkey triggering
        let expected_button = {
            *MOUSE_SHORTCUT.lock().unwrap()
        };

        if let Some(expected) = expected_button {
            // Check if the pressed button matches the expected mouse shortcut
            let mut matches = false;
            match (button, expected) {
                (Button::Left, Button::Left) => matches = true, // We still allow left if previously set (though UI prevents it now)
                (Button::Right, Button::Right) => matches = true,
                (Button::Middle, Button::Middle) => matches = true,
                (Button::Unknown(a), Button::Unknown(b)) if a == b => matches = true,
                _ => {}
            }

            if matches {
                if let Some(app) = APP_HANDLE.lock().unwrap().as_ref() {
                    debug!("Global hotkey (mouse) triggered: record toggle");
                    let _ = app.emit("hotkey-record-toggle", ());
                }
            }
        }
    }
}

/// Parses a string like "Unknown(1)" or "Middle" into an rdev::Button
fn parse_mouse_shortcut(shortcut: &str) -> Option<Button> {
    if shortcut == "Middle" {
        return Some(Button::Middle);
    }
    if shortcut.starts_with("Unknown(") && shortcut.ends_with(")") {
        let num_str = &shortcut["Unknown(".len()..shortcut.len() - 1];
        if let Ok(num) = num_str.parse::<u8>() {
            return Some(Button::Unknown(num));
        }
    }

    // Fallback for previous manual formats (if they were somehow set)
    match shortcut {
        "Mouse1" | "MouseMiddle" => Some(Button::Middle),
        _ => None,
    }
}

/// Register the global hotkey for record toggle based on config settings.
/// This should be called during setup and whenever the shortcut changes.
pub fn register_record_toggle(app: &AppHandle) {
    // Store app handle for rdev callback
    {
        *APP_HANDLE.lock().unwrap() = Some(app.clone());
    }

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

    // Reset mouse shortcut
    {
        *MOUSE_SHORTCUT.lock().unwrap() = None;
    }

    if let Some(mouse_btn) = parse_mouse_shortcut(&shortcut_str) {
        info!("Registering mouse shortcut: {}", shortcut_str);
        *MOUSE_SHORTCUT.lock().unwrap() = Some(mouse_btn);
        start_rdev_listener_if_needed();
        return;
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
