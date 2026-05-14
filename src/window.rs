use log::{debug, info, warn};
use slint::{winit_030::WinitWindowAccessor, ComponentHandle};
#[cfg(target_os = "linux")]
use std::sync::Once;

#[cfg(target_os = "linux")]
static WAYLAND_TOPMOST_WARNING: Once = Once::new();

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

/// Hide the window to the system tray instead of quitting.
pub fn hide_to_tray(app: &crate::App) {
    info!("Hiding to tray");
    app.window().hide().ok();
}

/// Quit the application — hide window and stop the Slint event loop.
pub fn quit_app(app: &crate::App) {
    info!("Quit requested");
    app.window().hide().ok();
    slint::quit_event_loop().ok();
}

/// Returns true when the current Linux session appears to be native Wayland.
#[cfg(target_os = "linux")]
fn is_likely_native_wayland_from_env(
    session_type: Option<&str>,
    has_wayland_display: bool,
    winit_backend: Option<&str>,
) -> bool {
    let session_is_wayland = session_type
        .map(|value| value.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false);
    let forced_x11 = winit_backend
        .map(|value| value.eq_ignore_ascii_case("x11"))
        .unwrap_or(false);

    (session_is_wayland || has_wayland_display) && !forced_x11
}

#[cfg(target_os = "linux")]
fn is_likely_native_wayland() -> bool {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let has_wayland_display = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let winit_backend = std::env::var("WINIT_UNIX_BACKEND").ok();

    is_likely_native_wayland_from_env(
        session_type.as_deref(),
        has_wayland_display,
        winit_backend.as_deref(),
    )
}

#[cfg(target_os = "linux")]
fn linux_has_compositor_from_env(
    session_type: Option<&str>,
    has_wayland_display: bool,
    kde_full_session: bool,
    gnome_desktop_session: bool,
    current_desktop: Option<&str>,
) -> bool {
    if is_likely_native_wayland_from_env(session_type, has_wayland_display, None) {
        return true;
    }

    if kde_full_session || gnome_desktop_session {
        return true;
    }

    const COMPOSITED_DESKTOPS: &[&str] = &[
        "kde",
        "gnome",
        "unity",
        "cinnamon",
        "deepin",
        "pantheon",
        "enlightenment",
    ];
    current_desktop
        .map(|desktop| {
            let desktop = desktop.to_lowercase();
            COMPOSITED_DESKTOPS.iter().any(|de| desktop.contains(de))
        })
        .unwrap_or(false)
}

/// Returns true when transparent overlay windows are expected to render correctly.
pub fn has_compositor() -> bool {
    #[cfg(target_os = "linux")]
    {
        if linux_has_compositor_from_env(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("KDE_FULL_SESSION").is_some(),
            std::env::var_os("GNOME_DESKTOP_SESSION_ID").is_some(),
            std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        ) {
            return true;
        }

        for name in ["picom", "compton", "xcompmgr"] {
            if std::process::Command::new("pgrep")
                .args(["-x", name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[cfg(target_os = "linux")]
fn is_winit_native_wayland(winit_win: &slint::winit_030::winit::window::Window) -> Option<bool> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    match winit_win.window_handle().ok()?.as_raw() {
        RawWindowHandle::Wayland(_) => Some(true),
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn warn_native_wayland_topmost_limitation_once() {
    WAYLAND_TOPMOST_WARNING.call_once(|| {
        warn!(
            "Native Wayland does not support winit WindowLevel::AlwaysOnTop; overlay/window stacking is best-effort"
        );
    });
}

/// Apply a winit window level to a Slint window when the native window exists.
pub fn apply_window_level(window: &slint::Window, on_top: bool) {
    let level = if on_top {
        slint::winit_030::winit::window::WindowLevel::AlwaysOnTop
    } else {
        slint::winit_030::winit::window::WindowLevel::Normal
    };

    if window
        .with_winit_window(|winit_win| {
            #[cfg(target_os = "linux")]
            if on_top && is_winit_native_wayland(winit_win).unwrap_or_else(is_likely_native_wayland)
            {
                warn_native_wayland_topmost_limitation_once();
            }

            winit_win.set_window_level(level);
        })
        .is_some()
    {
        debug!("Applied window level {:?}", level);
    } else {
        #[cfg(target_os = "linux")]
        if on_top && is_likely_native_wayland() {
            warn_native_wayland_topmost_limitation_once();
        }

        debug!(
            "Native winit window unavailable while applying level {:?}",
            level
        );
    }
}

/// Apply the main-window always-on-top setting.
pub fn apply_always_on_top(app: &crate::App, on_top: bool) {
    apply_window_level(app.window(), on_top);
    info!("Always-on-top set to {}", on_top);
}

/// Reassert topmost behavior for the recording overlay.
pub fn apply_overlay_topmost(window: &slint::Window) {
    apply_window_level(window, true);
}

#[cfg(target_os = "linux")]
fn x11_window_id(winit_win: &slint::winit_030::winit::window::Window) -> Option<u32> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    match winit_win.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
        RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn rounded_rectangles(width: u16, height: u16, radius: u16) -> Vec<x11rb::protocol::xproto::Rectangle> {
    use x11rb::protocol::xproto::Rectangle;

    let width = width.max(1);
    let height = height.max(1);
    let radius = radius.min(width / 2).min(height / 2);
    if radius == 0 {
        return vec![Rectangle { x: 0, y: 0, width, height }];
    }

    let mut rects = Vec::with_capacity(height as usize);
    let r = radius as f64;
    for y in 0..height {
        let top = y < radius;
        let bottom = y >= height - radius;
        let inset = if top || bottom {
            let cy = if top { r - 0.5 } else { height as f64 - r - 0.5 };
            let dy = (y as f64 + 0.5 - cy).abs();
            let dx = (r * r - dy * dy).max(0.0).sqrt();
            (r - dx).ceil() as u16
        } else {
            0
        };
        let rect_width = width.saturating_sub(inset.saturating_mul(2)).max(1);
        rects.push(Rectangle { x: inset as i16, y: y as i16, width: rect_width, height: 1 });
    }
    rects
}

#[cfg(target_os = "linux")]
fn harden_x11_overlay(window_id: u32, width: u16, height: u16, radius: u16) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::shape::{self, ConnectionExt as ShapeConnectionExt};
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
    use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

    let (conn, _) = x11rb::connect(None)?;
    let window = window_id;
    let atom = |name: &[u8]| -> Result<u32, Box<dyn std::error::Error>> {
        Ok(conn.intern_atom(false, name)?.reply()?.atom)
    };

    let wm_class = b"quillscribe-overlay\0QuillScribeOverlay\0";
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, wm_class)?;
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, b"QuillScribe Recording Overlay")?;
    conn.change_property8(PropMode::REPLACE, window, atom(b"_NET_WM_NAME")?, atom(b"UTF8_STRING")?, b"QuillScribe Recording Overlay")?;

    let window_types = [
        atom(b"_NET_WM_WINDOW_TYPE_TOOLTIP")?,
        atom(b"_NET_WM_WINDOW_TYPE_NOTIFICATION")?,
        atom(b"_NET_WM_WINDOW_TYPE_UTILITY")?,
    ];
    conn.change_property32(PropMode::REPLACE, window, atom(b"_NET_WM_WINDOW_TYPE")?, AtomEnum::ATOM, &window_types)?;

    let states = [
        atom(b"_NET_WM_STATE_ABOVE")?,
        atom(b"_NET_WM_STATE_STICKY")?,
        atom(b"_NET_WM_STATE_SKIP_TASKBAR")?,
        atom(b"_NET_WM_STATE_SKIP_PAGER")?,
    ];
    conn.change_property32(PropMode::REPLACE, window, atom(b"_NET_WM_STATE")?, AtomEnum::ATOM, &states)?;

    let motif_hints: [u32; 5] = [2, 0, 0, 0, 0];
    conn.change_property32(PropMode::REPLACE, window, atom(b"_MOTIF_WM_HINTS")?, atom(b"_MOTIF_WM_HINTS")?, &motif_hints)?;
    conn.change_property32(PropMode::REPLACE, window, atom(b"_NET_WM_BYPASS_COMPOSITOR")?, AtomEnum::CARDINAL, &[2])?;
    conn.change_property32(PropMode::REPLACE, window, atom(b"_NET_WM_WINDOW_OPACITY")?, AtomEnum::CARDINAL, &[u32::MAX])?;
    conn.delete_property(window, atom(b"_KDE_NET_WM_SHADOW")?)?;

    let rects = rounded_rectangles(width, height, radius);
    conn.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, x11rb::protocol::xproto::ClipOrdering::UNSORTED, window, 0, 0, &rects)?;
    conn.shape_rectangles(shape::SO::SET, shape::SK::INPUT, x11rb::protocol::xproto::ClipOrdering::UNSORTED, window, 0, 0, &rects)?;
    conn.flush()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn harden_recording_overlay_impl(
    winit_win: &slint::winit_030::winit::window::Window,
    is_full: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(window_id) = x11_window_id(winit_win) else {
        return Ok(());
    };
    let size = winit_win.inner_size();
    let width = u16::try_from(size.width.min(u16::MAX as u32)).unwrap_or(u16::MAX).max(1);
    let height = u16::try_from(size.height.min(u16::MAX as u32)).unwrap_or(u16::MAX).max(1);
    let scale = winit_win.scale_factor();
    let radius = if !is_full {
        height / 2
    } else {
        ((16.0 * scale).round() as u16).max(1)
    };

    harden_x11_overlay(window_id, width, height, radius)
}

/// Apply X11-specific metadata and shape hints to reduce compositor shadows on the recording overlay.
pub fn harden_recording_overlay(window: &slint::Window, is_full: bool) {
    #[cfg(target_os = "linux")]
    {
        if window
            .with_winit_window(|winit_win| {
                if let Err(e) = harden_recording_overlay_impl(winit_win, is_full) {
                    debug!("Failed to harden X11 recording overlay: {}", e);
                }
            })
            .is_none()
        {
            debug!("Native winit window unavailable while hardening recording overlay");
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, is_full);
    }
}

/// Apply the custom-titlebar toggle.
/// The Slint UI handles this via the no-frame property binding.
pub fn apply_custom_titlebar(app: &crate::App, custom: bool) {
    app.set_custom_titlebar(custom);
    debug!("Custom titlebar: {} (Slint no-frame updated)", custom);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn native_wayland_detection_uses_session_and_backend_hints() {
        assert!(is_likely_native_wayland_from_env(
            Some("wayland"),
            false,
            None
        ));
        assert!(is_likely_native_wayland_from_env(None, true, None));
        assert!(!is_likely_native_wayland_from_env(Some("x11"), false, None));
        assert!(!is_likely_native_wayland_from_env(
            Some("wayland"),
            true,
            Some("x11")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_compositor_detection_uses_session_and_desktop_hints() {
        assert!(linux_has_compositor_from_env(
            Some("wayland"),
            false,
            false,
            false,
            None
        ));
        assert!(linux_has_compositor_from_env(
            Some("x11"),
            false,
            true,
            false,
            None
        ));
        assert!(linux_has_compositor_from_env(
            Some("x11"),
            false,
            false,
            false,
            Some("XFCE:GNOME")
        ));
        assert!(!linux_has_compositor_from_env(
            Some("x11"),
            false,
            false,
            false,
            Some("XFCE")
        ));
    }

    #[test]
    fn decode_png_to_rgba_valid_icon() {
        let bytes = include_bytes!("../icons/taskbar/white.png");
        let (rgba, w, h) = decode_png_to_rgba(bytes).expect("decode valid PNG");
        assert!(!rgba.is_empty(), "RGBA buffer should not be empty");
        assert!(w > 0, "width should be > 0");
        assert!(h > 0, "height should be > 0");
        assert_eq!(
            rgba.len() as u32,
            w * h * 4,
            "RGBA buffer size should match w*h*4"
        );
    }

    #[test]
    fn taskbar_icon_bytes_for_known_themes() {
        let themes = [
            "white",
            "warm_gray",
            "soft_beige",
            "blue_gray",
            "warm_taupe",
            "soft_sage",
            "dark_charcoal",
            "dark_blue",
            "dark_purple",
            "dark_forest",
            "dark_burgundy",
            "obsidian",
        ];
        for theme in themes {
            let bytes = taskbar_icon_bytes_for_theme(theme);
            assert!(
                !bytes.is_empty(),
                "theme '{}' should have non-empty icon bytes",
                theme
            );
        }
    }

    #[test]
    fn taskbar_icon_bytes_unknown_theme_fallback() {
        let fallback = taskbar_icon_bytes_for_theme("white");
        let unknown = taskbar_icon_bytes_for_theme("nonexistent_theme_xyz");
        assert_eq!(unknown, fallback, "unknown theme should fall back to white");
    }
}
