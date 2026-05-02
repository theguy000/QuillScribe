use log::{debug, info};
use crate::config::ConfigManager;

/// Apply always-on-top setting.
/// TODO: Implement with Slint/winit window API.
#[allow(dead_code)]
pub fn apply_always_on_top(_on_top: bool) {
    info!("Always-on-top set to {}", _on_top);
}

/// Apply the custom-titlebar toggle.
/// With Slint, the titlebar is rendered in the UI itself.
#[allow(dead_code)]
pub fn apply_custom_titlebar(_custom: bool) {
    debug!("Custom titlebar: {} (handled in Slint UI)", _custom);
}

/// Set always-on-top and persist to config.
#[allow(dead_code)]
pub fn set_always_on_top(config: &ConfigManager, on_top: bool) {
    config.set_always_on_top(on_top);
    let _ = config.save_settings();
    apply_always_on_top(on_top);
}
