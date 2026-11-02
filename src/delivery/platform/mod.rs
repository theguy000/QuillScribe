//! The operating-system half of delivery: which window has focus and what it is.
//!
//! Everything the app asks the desktop lives here, so the rules in the parent
//! module can be checked on any build. [`focused_window`], [`target_window`]
//! and [`focused_window_exists`] are the only three answers the parent needs.

#[cfg(windows)]
mod automation;
#[cfg(windows)]
mod windows;

/// The window that holds focus now, or `None` when there is none.
#[cfg(windows)]
pub(crate) fn focused_window() -> Option<WindowId> {
    windows::focused_window()
}

/// What the caret's host says about itself.
#[cfg(windows)]
pub(crate) fn target_window(keys: &[String]) -> TargetWindow {
    windows::target_window(keys)
}

/// Windows can tell a closed window from a missing answer, so it uses its own
/// reading. Without a focused window there is no window to write into, so stop
/// takes the words.
#[cfg(windows)]
pub(crate) fn focused_window_exists() -> bool {
    windows::focused_window_exists().unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub(super) use self::linux::{focused_window, focused_window_exists, target_window};

#[cfg(target_os = "linux")]
mod linux {
    use crate::delivery::{describe_target, FocusedElement, TargetWindow, WindowId};

    /// The window that holds focus now, or `None` when there is none.
    ///
    /// xdotool is already a dependency of the X11 paste path, so reading the
    /// active window costs no new window-system code. Under Wayland it cannot
    /// answer, and neither can the app: an utterance with no window to bind
    /// delivers at stop instead (`output::focus_reading_unavailable`).
    pub(crate) fn focused_window() -> Option<WindowId> {
        let output = std::process::Command::new("xdotool")
            .arg("getactivewindow")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let window = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if window.is_empty() {
            return None;
        }
        Some(WindowId::new(format!("x11:{}", window)))
    }

    /// The platform either names the focused window or cannot answer at all, so
    /// a missing id means "no window to write into".
    pub(crate) fn focused_window_exists() -> bool {
        focused_window().is_some()
    }

    /// What the X11 window manager and the shell say about the focused window.
    ///
    /// There is no role query on Linux without a desktop-specific accessibility
    /// bridge, so the window's class and name are all the rule can read. That is
    /// enough for the terminal rule and for recognising QuillScribe's own key
    /// fields, and it is why a password box on Linux is best effort only: see
    /// `docs/live-typing-targets.md`.
    ///
    /// Ydotool pastes on Wayland, where nothing can name the focused window, so
    /// the app never reaches this description there: no delivery session is
    /// created.
    pub(crate) fn target_window(keys: &[String]) -> TargetWindow {
        if focused_window().is_none() {
            return TargetWindow::default();
        }

        // `getwindowclassname` is the window class (`gnome-terminal-server`)
        // and `getwindowname` is the title. Both open no accessibility
        // connection, which is also why no role is read here: a password box on
        // Linux is best effort.
        let class_name = xdotool(&["getwindowclassname", "@"]).unwrap_or_default();
        let name = xdotool(&["getwindowname", "@"]).unwrap_or_default();

        // The app's own window carries its own process id.
        let own = xdotool(&["getwindowpid", "@"])
            .and_then(|pid| pid.parse::<u32>().ok())
            .map(|pid| pid == std::process::id())
            .unwrap_or(false);

        describe_target(
            &FocusedElement {
                class_name,
                name,
                ..FocusedElement::default()
            },
            own,
            keys,
        )
    }

    fn xdotool(args: &[&str]) -> Option<String> {
        let output = std::process::Command::new("xdotool")
            .args(args)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// A desktop this build knows nothing about: nothing is ever written into a
/// window, so stop keeps the words.
#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) fn focused_window() -> Option<WindowId> {
    None
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) fn target_window(_keys: &[String]) -> TargetWindow {
    TargetWindow::default()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) fn focused_window_exists() -> bool {
    false
}

#[cfg(any(windows, not(target_os = "linux")))]
use super::{TargetWindow, WindowId};
