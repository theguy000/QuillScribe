//! What Windows says about the focused window.
//!
//! The rules themselves live in the parent module; this one only asks the
//! window manager and hands back its answers. UI Automation, which reads the
//! focused control rather than the window, lives in the sibling module.

use crate::delivery::{describe_target, FocusedElement, TargetWindow, WindowId};

/// Whether the focused window is still a live window, or `None` when another
/// window has focus.
pub(super) fn focused_window_exists() -> Option<bool> {
    let window = foreground_window()?;
    // A closed window leaves a stale handle behind, so "still focused" has to
    // ask the window manager whether the handle is still a window at all.
    Some(is_window(window))
}

/// The window that holds focus now, or `None` when there is none.
pub(super) fn focused_window() -> Option<WindowId> {
    let window = foreground_window()?;
    Some(WindowId::new(format!("win:{}", window as isize)))
}

/// What the caret's host says about itself.
pub(super) fn target_window(keys: &[String]) -> TargetWindow {
    let Some(window) = foreground_window() else {
        return TargetWindow::default();
    };
    // The app knows its own window without asking anyone.
    let own = is_our_process(window);
    let class_name = class_name(window).unwrap_or_default();
    let automation = super::automation::read().unwrap_or_default();

    describe_target(
        &FocusedElement {
            class_name,
            ..automation
        },
        own,
        keys,
    )
}

/// Windows calls that take a raw handle, kept together so the rest of this
/// module deals in plain handles.
mod call {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
    };

    pub(super) fn foreground() -> Option<HWND> {
        let window = unsafe { GetForegroundWindow() };
        (!window.0.is_null()).then_some(window)
    }

    pub(super) fn is_window(window: HWND) -> bool {
        unsafe { IsWindow(window).as_bool() }
    }

    pub(super) fn process_of(window: HWND) -> u32 {
        let mut process = 0;
        unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
        process
    }

    pub(super) fn class_of(window: HWND) -> Option<String> {
        let mut buffer = [0u16; 256];
        let len = unsafe { GetClassNameW(window, &mut buffer) };
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

/// The focused window's handle, widened to an integer so this module needs no
/// handle type of its own.
fn foreground_window() -> Option<usize> {
    Some(call::foreground()?.0 as usize)
}

fn is_window(window: usize) -> bool {
    call::is_window(handle(window))
}

/// The window's class, or `None` when the window manager will not say.
fn class_name(window: usize) -> Option<String> {
    call::class_of(handle(window))
}

fn is_our_process(window: usize) -> bool {
    let process = call::process_of(handle(window));
    process != 0 && process == std::process::id()
}

/// A handle read back from a plain integer. It comes from `GetForegroundWindow`
/// and is only ever handed straight back to the window manager.
fn handle(window: usize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(window as *mut _)
}
