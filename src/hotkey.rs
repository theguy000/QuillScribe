use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use log::{debug, error, info};

static HOTKEY_MANAGER: std::sync::Mutex<Option<GlobalHotKeyManager>> = std::sync::Mutex::new(None);

pub fn register_record_toggle(app_weak: &slint::Weak<crate::App>) {
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to create global hotkey manager: {}", e);
            return;
        }
    };

    // Default shortcut: Super+Shift+Space
    let shortcut_str = "Super+Shift+Space";
    let hotkey = match parse_shortcut(shortcut_str) {
        Ok(h) => h,
        Err(e) => {
            error!("Failed to parse shortcut '{}': {}", shortcut_str, e);
            return;
        }
    };

    if let Err(e) = manager.register(hotkey) {
        error!("Failed to register global shortcut '{}': {}", shortcut_str, e);
        let mut guard = HOTKEY_MANAGER.lock().unwrap();
        *guard = Some(manager);
        return;
    }

    info!("Registered global shortcut: {}", shortcut_str);
    let weak = app_weak.clone();
    std::thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                if event.state == HotKeyState::Pressed {
                    debug!("Global hotkey triggered: record toggle");
                    if let Some(app) = weak.upgrade() {
                        app.invoke_toggle_recording();
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    let mut guard = HOTKEY_MANAGER.lock().unwrap();
    *guard = Some(manager);
}

fn parse_shortcut(shortcut: &str) -> Result<global_hotkey::hotkey::HotKey, String> {
    let converted = convert_shortcut_format(shortcut);
    converted.parse().map_err(|e| format!("{}", e))
}

fn convert_shortcut_format(shortcut: &str) -> String {
    shortcut
        .replace("Meta+", "Super+")
        .replace("meta+", "Super+")
}
