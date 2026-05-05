use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use log::{debug, error, info};
use rdev::{listen, EventType, Key};
use std::sync::{
    atomic::{AtomicBool, Ordering},
};

struct GlobalHotKeyManagerWrapper(GlobalHotKeyManager);
unsafe impl Send for GlobalHotKeyManagerWrapper {}
unsafe impl Sync for GlobalHotKeyManagerWrapper {}

static HOTKEY_MANAGER: std::sync::Mutex<Option<GlobalHotKeyManagerWrapper>> = std::sync::Mutex::new(None);
static REGISTERED_HOTKEY: std::sync::Mutex<Option<global_hotkey::hotkey::HotKey>> = std::sync::Mutex::new(None);

// Single persistent listener thread. The callback becomes a no-op when
// RECORDING_ACTIVE is false, avoiding per-session thread/hook leaks.
static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
static LISTENER_STARTED: AtomicBool = AtomicBool::new(false);

// Stored callback + modifier state for the persistent listener.
type RecordingCallback = Box<dyn Fn(String) + Send>;
static RECORDING_CALLBACK: std::sync::Mutex<Option<RecordingCallback>> = std::sync::Mutex::new(None);
static MODIFIER_STATE: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Map rdev Key to the key name format expected by global-hotkey.
fn key_to_hotkey_name(key: &Key) -> Option<String> {
    Some(match key {
        // Letters
        Key::KeyA => "A".into(), Key::KeyB => "B".into(), Key::KeyC => "C".into(),
        Key::KeyD => "D".into(), Key::KeyE => "E".into(), Key::KeyF => "F".into(),
        Key::KeyG => "G".into(), Key::KeyH => "H".into(), Key::KeyI => "I".into(),
        Key::KeyJ => "J".into(), Key::KeyK => "K".into(), Key::KeyL => "L".into(),
        Key::KeyM => "M".into(), Key::KeyN => "N".into(), Key::KeyO => "O".into(),
        Key::KeyP => "P".into(), Key::KeyQ => "Q".into(), Key::KeyR => "R".into(),
        Key::KeyS => "S".into(), Key::KeyT => "T".into(), Key::KeyU => "U".into(),
        Key::KeyV => "V".into(), Key::KeyW => "W".into(), Key::KeyX => "X".into(),
        Key::KeyY => "Y".into(), Key::KeyZ => "Z".into(),
        // Numbers
        Key::Num0 => "0".into(), Key::Num1 => "1".into(), Key::Num2 => "2".into(),
        Key::Num3 => "3".into(), Key::Num4 => "4".into(), Key::Num5 => "5".into(),
        Key::Num6 => "6".into(), Key::Num7 => "7".into(), Key::Num8 => "8".into(),
        Key::Num9 => "9".into(),
        // Function keys
        Key::F1 => "F1".into(), Key::F2 => "F2".into(), Key::F3 => "F3".into(),
        Key::F4 => "F4".into(), Key::F5 => "F5".into(), Key::F6 => "F6".into(),
        Key::F7 => "F7".into(), Key::F8 => "F8".into(), Key::F9 => "F9".into(),
        Key::F10 => "F10".into(), Key::F11 => "F11".into(), Key::F12 => "F12".into(),
        // Special keys
        Key::Space => "Space".into(),
        Key::Return => "Return".into(),
        Key::Escape => "Escape".into(),
        Key::Backspace => "Backspace".into(),
        Key::Tab => "Tab".into(),
        Key::Delete => "Delete".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),
        Key::Insert => "Insert".into(),
        Key::CapsLock => "CapsLock".into(),
        // Arrow keys
        Key::UpArrow => "Up".into(), Key::DownArrow => "Down".into(),
        Key::LeftArrow => "Left".into(), Key::RightArrow => "Right".into(),
        // Other
        Key::NumLock => "NumLock".into(),
        Key::PrintScreen => "Print".into(),
        Key::ScrollLock => "ScrollLock".into(),
        Key::Pause => "Pause".into(),
        // Modifiers — handled separately
        Key::ControlLeft | Key::ControlRight
        | Key::Alt | Key::AltGr
        | Key::ShiftLeft | Key::ShiftRight
        | Key::MetaLeft | Key::MetaRight => return None,
        _ => format!("{:?}", key),
    })
}

fn is_modifier(key: &Key) -> bool {
    modifier_name(key).is_some()
}

fn modifier_name(key: &Key) -> Option<&'static str> {
    match key {
        Key::ControlLeft | Key::ControlRight => Some("Control"),
        Key::Alt => Some("Alt"),
        Key::AltGr => Some("AltGr"),
        Key::ShiftLeft | Key::ShiftRight => Some("Shift"),
        Key::MetaLeft | Key::MetaRight => Some("Meta"),
        _ => None,
    }
}

/// Start the single persistent rdev listener thread (once, ever).
fn ensure_listener_started() {
    if LISTENER_STARTED.load(Ordering::Acquire) {
        return;
    }
    // Mark started before spawning so we don't race
    LISTENER_STARTED.store(true, Ordering::Release);

    std::thread::spawn(move || {
        if let Err(e) = listen(move |event| {
            if !RECORDING_ACTIVE.load(Ordering::Relaxed) {
                return;
            }

            match event.event_type {
                EventType::KeyPress(key) => {
                    if is_modifier(&key) {
                        let mut mods = MODIFIER_STATE.lock().unwrap();
                        let name = modifier_name(&key).unwrap();
                        if !mods.contains(&name.to_string()) {
                            mods.push(name.to_string());
                        }
                    } else if let Some(key_name) = key_to_hotkey_name(&key) {
                        let mut mods = MODIFIER_STATE.lock().unwrap();
                        if !mods.is_empty() {
                            let shortcut = format!("{}+{}", mods.join("+"), key_name);
                            RECORDING_ACTIVE.store(false, Ordering::Relaxed);
                            mods.clear();
                            drop(mods);
                            // Invoke the stored callback
                            if let Ok(cb_guard) = RECORDING_CALLBACK.lock() {
                                if let Some(ref cb) = *cb_guard {
                                    cb(shortcut);
                                }
                            }
                        }
                    }
                }
                EventType::KeyRelease(key) => {
                    if is_modifier(&key) {
                        let mut mods = MODIFIER_STATE.lock().unwrap();
                        if let Some(name) = modifier_name(&key) {
                            mods.retain(|m| m != name);
                        }
                    }
                }
                _ => {}
            }
        }) {
            error!("Failed to start keyboard listener: {:?}", e);
            LISTENER_STARTED.store(false, Ordering::Release);
        }
    });
}

pub fn register_record_toggle(app_weak: &slint::Weak<crate::App>, shortcut: &str) {
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to create global hotkey manager: {}", e);
            return;
        }
    };

    let shortcut_str = shortcut.to_string();
    let hotkey = match parse_shortcut(shortcut) {
        Ok(h) => h,
        Err(e) => {
            error!("Failed to parse shortcut '{}': {}", shortcut_str, e);
            return;
        }
    };

    if let Err(e) = manager.register(hotkey) {
        error!("Failed to register global shortcut '{}': {}", shortcut_str, e);
        let mut guard = HOTKEY_MANAGER.lock().unwrap();
        *guard = Some(GlobalHotKeyManagerWrapper(manager));
        return;
    }

    info!("Registered global shortcut: {}", shortcut_str);
    {
        let mut rh = REGISTERED_HOTKEY.lock().unwrap();
        *rh = Some(hotkey);
    }
    let weak = app_weak.clone();
    std::thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                if event.state == HotKeyState::Pressed {
                    debug!("Global hotkey triggered: record toggle");
                    let weak_clone = weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = weak_clone.upgrade() {
                            app.invoke_toggle_recording();
                        }
                    });
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    let mut guard = HOTKEY_MANAGER.lock().unwrap();
    *guard = Some(GlobalHotKeyManagerWrapper(manager));
}

/// Re-register the global hotkey with a new shortcut.
/// Unregisters the previous hotkey first.
pub fn reregister_hotkey(shortcut: &str) -> Result<(), String> {
    let hotkey = parse_shortcut(shortcut)?;

    let mut guard = HOTKEY_MANAGER.lock().unwrap();
    let manager = match guard.as_mut() {
        Some(m) => &mut m.0,
        None => return Err("Hotkey manager not initialized".to_string()),
    };

    let old_hotkey = *REGISTERED_HOTKEY.lock().unwrap();

    if let Some(old) = old_hotkey {
        if let Err(e) = manager.unregister(old) {
            return Err(format!("Failed to unregister old shortcut: {}", e));
        }
    }

    if let Err(e) = manager.register(hotkey) {
        if let Some(old) = old_hotkey {
            if let Err(restore_error) = manager.register(old) {
                error!("Failed to restore previous hotkey: {}", restore_error);
                let mut rh = REGISTERED_HOTKEY.lock().unwrap();
                *rh = None;
            }
        }
        Err(format!("Failed to register shortcut '{}': {}", shortcut, e))
    } else {
        let mut rh = REGISTERED_HOTKEY.lock().unwrap();
        *rh = Some(hotkey);
        info!("Re-registered global shortcut: {}", shortcut);
        Ok(())
    }
}

/// Start recording a keyboard shortcut. The callback is invoked from a
/// background thread — callers must dispatch to the UI thread themselves.
pub fn start_keyboard_recording<F>(callback: F) -> Result<(), String>
where
    F: Fn(String) + Send + 'static,
{
    if RECORDING_ACTIVE.load(Ordering::Relaxed) {
        return Err("Keyboard listener already active".to_string());
    }

    // Store the callback for the persistent listener to use
    {
        let mut cb = RECORDING_CALLBACK.lock().unwrap();
        *cb = Some(Box::new(callback));
    }

    // Clear stale modifier state
    MODIFIER_STATE.lock().unwrap().clear();

    // Ensure the persistent listener thread is running
    ensure_listener_started();

    // Activate recording
    RECORDING_ACTIVE.store(true, Ordering::Relaxed);

    Ok(())
}

pub fn stop_keyboard_recording() {
    RECORDING_ACTIVE.store(false, Ordering::Relaxed);
    MODIFIER_STATE.lock().unwrap().clear();
    *RECORDING_CALLBACK.lock().unwrap() = None;
}

fn parse_shortcut(shortcut: &str) -> Result<global_hotkey::hotkey::HotKey, String> {
    let converted = convert_shortcut_format(shortcut);
    converted.parse().map_err(|e| format!("{}", e))
}

fn convert_shortcut_format(shortcut: &str) -> String {
    shortcut
        .replace("Meta+", "Super+")
        .replace("meta+", "Super+")
        .replace("AltGr+", "Alt+")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── key_to_hotkey_name ─────────────────────────────────────────────────

    #[test]
    fn key_to_hotkey_name_letters() {
        assert_eq!(key_to_hotkey_name(&Key::KeyA), Some("A".into()));
        assert_eq!(key_to_hotkey_name(&Key::KeyZ), Some("Z".into()));
    }

    #[test]
    fn key_to_hotkey_name_numbers() {
        assert_eq!(key_to_hotkey_name(&Key::Num0), Some("0".into()));
        assert_eq!(key_to_hotkey_name(&Key::Num9), Some("9".into()));
    }

    #[test]
    fn key_to_hotkey_name_function_keys() {
        assert_eq!(key_to_hotkey_name(&Key::F1), Some("F1".into()));
        assert_eq!(key_to_hotkey_name(&Key::F12), Some("F12".into()));
    }

    #[test]
    fn key_to_hotkey_name_special_keys() {
        assert_eq!(key_to_hotkey_name(&Key::Space), Some("Space".into()));
        assert_eq!(key_to_hotkey_name(&Key::Return), Some("Return".into()));
        assert_eq!(key_to_hotkey_name(&Key::Escape), Some("Escape".into()));
        assert_eq!(key_to_hotkey_name(&Key::Backspace), Some("Backspace".into()));
        assert_eq!(key_to_hotkey_name(&Key::Tab), Some("Tab".into()));
        assert_eq!(key_to_hotkey_name(&Key::Delete), Some("Delete".into()));
        assert_eq!(key_to_hotkey_name(&Key::Home), Some("Home".into()));
        assert_eq!(key_to_hotkey_name(&Key::End), Some("End".into()));
        assert_eq!(key_to_hotkey_name(&Key::PageUp), Some("PageUp".into()));
        assert_eq!(key_to_hotkey_name(&Key::PageDown), Some("PageDown".into()));
        assert_eq!(key_to_hotkey_name(&Key::Insert), Some("Insert".into()));
        assert_eq!(key_to_hotkey_name(&Key::CapsLock), Some("CapsLock".into()));
    }

    #[test]
    fn key_to_hotkey_name_arrow_keys() {
        assert_eq!(key_to_hotkey_name(&Key::UpArrow), Some("Up".into()));
        assert_eq!(key_to_hotkey_name(&Key::DownArrow), Some("Down".into()));
        assert_eq!(key_to_hotkey_name(&Key::LeftArrow), Some("Left".into()));
        assert_eq!(key_to_hotkey_name(&Key::RightArrow), Some("Right".into()));
    }

    #[test]
    fn key_to_hotkey_name_other_known_keys() {
        assert_eq!(key_to_hotkey_name(&Key::NumLock), Some("NumLock".into()));
        assert_eq!(key_to_hotkey_name(&Key::PrintScreen), Some("Print".into()));
        assert_eq!(key_to_hotkey_name(&Key::ScrollLock), Some("ScrollLock".into()));
        assert_eq!(key_to_hotkey_name(&Key::Pause), Some("Pause".into()));
    }

    #[test]
    fn key_to_hotkey_name_modifiers_return_none() {
        assert_eq!(key_to_hotkey_name(&Key::ControlLeft), None);
        assert_eq!(key_to_hotkey_name(&Key::ControlRight), None);
        assert_eq!(key_to_hotkey_name(&Key::Alt), None);
        assert_eq!(key_to_hotkey_name(&Key::AltGr), None);
        assert_eq!(key_to_hotkey_name(&Key::ShiftLeft), None);
        assert_eq!(key_to_hotkey_name(&Key::ShiftRight), None);
        assert_eq!(key_to_hotkey_name(&Key::MetaLeft), None);
        assert_eq!(key_to_hotkey_name(&Key::MetaRight), None);
    }

    // ── is_modifier ────────────────────────────────────────────────────────

    #[test]
    fn is_modifier_true_for_all_modifiers() {
        assert!(is_modifier(&Key::ControlLeft));
        assert!(is_modifier(&Key::ControlRight));
        assert!(is_modifier(&Key::Alt));
        assert!(is_modifier(&Key::AltGr));
        assert!(is_modifier(&Key::ShiftLeft));
        assert!(is_modifier(&Key::ShiftRight));
        assert!(is_modifier(&Key::MetaLeft));
        assert!(is_modifier(&Key::MetaRight));
    }

    #[test]
    fn is_modifier_false_for_non_modifiers() {
        assert!(!is_modifier(&Key::KeyA));
        assert!(!is_modifier(&Key::Num1));
        assert!(!is_modifier(&Key::F1));
        assert!(!is_modifier(&Key::Space));
        assert!(!is_modifier(&Key::Return));
    }

    // ── modifier_name ─────────────────────────────────────────────────────

    #[test]
    fn modifier_name_returns_correct_names() {
        assert_eq!(modifier_name(&Key::ControlLeft), Some("Control"));
        assert_eq!(modifier_name(&Key::ControlRight), Some("Control"));
        assert_eq!(modifier_name(&Key::Alt), Some("Alt"));
        assert_eq!(modifier_name(&Key::AltGr), Some("AltGr"));
        assert_eq!(modifier_name(&Key::ShiftLeft), Some("Shift"));
        assert_eq!(modifier_name(&Key::ShiftRight), Some("Shift"));
        assert_eq!(modifier_name(&Key::MetaLeft), Some("Meta"));
        assert_eq!(modifier_name(&Key::MetaRight), Some("Meta"));
    }

    #[test]
    fn modifier_name_returns_none_for_non_modifiers() {
        assert_eq!(modifier_name(&Key::KeyA), None);
        assert_eq!(modifier_name(&Key::Space), None);
        assert_eq!(modifier_name(&Key::F1), None);
    }

    // ── convert_shortcut_format ───────────────────────────────────────────

    #[test]
    fn convert_shortcut_format_replaces_meta_with_super() {
        assert_eq!(convert_shortcut_format("Meta+A"), "Super+A");
    }

    #[test]
    fn convert_shortcut_format_replaces_lowercase_meta() {
        assert_eq!(convert_shortcut_format("meta+A"), "Super+A");
    }

    #[test]
    fn convert_shortcut_format_replaces_altgr_with_alt() {
        assert_eq!(convert_shortcut_format("AltGr+A"), "Alt+A");
    }

    #[test]
    fn convert_shortcut_format_handles_combined_replacements() {
        assert_eq!(convert_shortcut_format("Meta+AltGr+A"), "Super+Alt+A");
    }

    #[test]
    fn convert_shortcut_format_no_change_when_no_replacements() {
        assert_eq!(convert_shortcut_format("Control+Shift+A"), "Control+Shift+A");
    }

    #[test]
    fn convert_shortcut_format_preserves_non_meta_text() {
        assert_eq!(convert_shortcut_format("Super+Shift+Space"), "Super+Shift+Space");
    }

    // ── parse_shortcut ────────────────────────────────────────────────────

    #[test]
    fn parse_shortcut_valid_simple() {
        assert!(parse_shortcut("Control+A").is_ok());
    }

    #[test]
    fn parse_shortcut_valid_multi_modifier() {
        assert!(parse_shortcut("Super+Shift+Space").is_ok());
    }

    #[test]
    fn parse_shortcut_valid_with_meta_conversion() {
        // "Meta+" is converted to "Super+" before parsing
        assert!(parse_shortcut("Meta+Shift+Space").is_ok());
    }

    #[test]
    fn parse_shortcut_valid_with_altgr_conversion() {
        // "AltGr+" is converted to "Alt+" before parsing
        assert!(parse_shortcut("AltGr+A").is_ok());
    }

    #[test]
    fn parse_shortcut_rejects_empty() {
        assert!(parse_shortcut("").is_err());
    }

    #[test]
    fn parse_shortcut_rejects_invalid_key() {
        assert!(parse_shortcut("Control+FooBar").is_err());
    }

    #[test]
    fn parse_shortcut_rejects_bare_modifier() {
        // A bare modifier like "Control" alone is not a valid hotkey
        assert!(parse_shortcut("Control").is_err());
    }

    // ── start/stop_keyboard_recording state ───────────────────────────────

    #[test]
    fn stop_keyboard_recording_deactivates_state() {
        // Ensure clean state first
        stop_keyboard_recording();
        // Should not panic on double-stop
        stop_keyboard_recording();
        // After stop, starting should succeed
        let result = start_keyboard_recording(|_| {});
        assert!(result.is_ok(), "should be able to start after stop");
        // Clean up
        stop_keyboard_recording();
    }

    #[test]
    fn start_keyboard_recording_rejects_when_already_active() {
        stop_keyboard_recording();
        let result = start_keyboard_recording(|_| {});
        assert!(result.is_ok(), "first start should succeed");
        let result2 = start_keyboard_recording(|_| {});
        assert!(result2.is_err(), "second start should fail");
        stop_keyboard_recording();
    }

    #[test]
    fn stop_keyboard_recording_clears_modifier_state() {
        stop_keyboard_recording();
        // Push something into modifier state
        MODIFIER_STATE.lock().unwrap().push("Control".into());
        stop_keyboard_recording();
        assert!(MODIFIER_STATE.lock().unwrap().is_empty(), "modifiers should be cleared after stop");
    }
}
