use log::{debug, info, warn};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use crate::config::ConfigManager;

/// Start a window drag operation via winit.
/// Call this from the titlebar's TouchArea `pointer-event` on down.
pub fn drag_window(app: &crate::App) {
    app.window().with_winit_window(|winit_win| {
        if let Err(e) = winit_win.drag_window() {
            warn!("drag_window failed: {}", e);
        }
    });
}

/// Minimize the window via winit.
pub fn minimize_window(app: &crate::App) {
    app.window().with_winit_window(|winit_win| {
        winit_win.set_minimized(true);
    });
}

/// Close the window — quit the Slint event loop gracefully so cleanup runs.
pub fn close_window(app: &crate::App) {
    info!("Close requested");
    let _ = app.window().hide();
    let _ = slint::quit_event_loop();
}

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

/// Returns the 256x256 taskbar icon bytes for the given theme.
fn taskbar_icon_bytes_for_theme(theme: &str) -> &'static [u8] {
    match theme {
        "white" => include_bytes!("../icons/taskbar/white.png"),
        "warm_gray" => include_bytes!("../icons/taskbar/warm_gray.png"),
        "soft_beige" => include_bytes!("../icons/taskbar/soft_beige.png"),
        "blue_gray" => include_bytes!("../icons/taskbar/blue_gray.png"),
        "warm_taupe" => include_bytes!("../icons/taskbar/warm_taupe.png"),
        "soft_sage" => include_bytes!("../icons/taskbar/soft_sage.png"),
        "dark_charcoal" => include_bytes!("../icons/taskbar/dark_charcoal.png"),
        "dark_blue" => include_bytes!("../icons/taskbar/dark_blue.png"),
        "dark_purple" => include_bytes!("../icons/taskbar/dark_purple.png"),
        "dark_forest" => include_bytes!("../icons/taskbar/dark_forest.png"),
        "dark_burgundy" => include_bytes!("../icons/taskbar/dark_burgundy.png"),
        "obsidian" => include_bytes!("../icons/taskbar/obsidian.png"),
        _ => include_bytes!("../icons/taskbar/white.png"),
    }
}

/// Decode PNG bytes into raw RGBA data (bytes, width, height).
/// Shared by window icon and tray icon creation.
pub(crate) fn decode_png_to_rgba(png_bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode error: {}", e))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG frame error: {}", e))?;
    buf.resize(info.buffer_size(), 0);
    Ok((buf, info.width, info.height))
}

/// Decode PNG bytes into a winit window icon.
fn icon_from_png_bytes(png_bytes: &[u8]) -> Result<slint::winit_030::winit::window::Icon, String> {
    let (rgba, w, h) = decode_png_to_rgba(png_bytes)?;
    slint::winit_030::winit::window::Icon::from_rgba(rgba, w, h)
        .map_err(|e| format!("Icon from RGBA error: {}", e))
}

/// Set the window (taskbar) icon to match the current theme.
pub fn set_window_icon_theme(app: &crate::App, theme: &str) {
    let icon = match icon_from_png_bytes(taskbar_icon_bytes_for_theme(theme)) {
        Ok(icon) => icon,
        Err(e) => {
            warn!("Failed to set window icon for theme {}: {}", theme, e);
            return;
        }
    };
    app.window().with_winit_window(|winit_win| {
        winit_win.set_window_icon(Some(icon));
    });
    debug!("Window icon updated for theme: {}", theme);
}
