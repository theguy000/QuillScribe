//! What Windows says about the focused window and the control that has the
//! caret.
//!
//! The rules themselves live in the parent module; this one only reads them.

use super::{describe_target, platform_focused_window, FocusedElement, TargetWindow, WindowId};

/// Whether the focused window is still a live window, or `None` when another
/// window has focus.
pub(super) fn focused_window_exists() -> Option<bool> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};

    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }
    // A closed window leaves a stale handle behind, so "still focused" has to
    // ask the window manager whether the handle is still a window at all.
    Some(unsafe { IsWindow(window).as_bool() })
}

/// What the caret's host says about itself.
pub(super) fn target_window(keys: &[String]) -> TargetWindow {
    let Some(window) = platform_focused_window() else {
        return TargetWindow::default();
    };
    // The app knows its own window without asking anyone.
    let own = focused_window_is_ours(&window);
    let class_name = class_name(&window).unwrap_or_default();
    let automation = accessibility().unwrap_or_default();

    describe_target(
        &FocusedElement {
            class_name,
            ..automation
        },
        own,
        keys,
    )
}

/// The window handle behind a [`WindowId`] read from this platform.
fn window_handle(window: &WindowId) -> Option<usize> {
    window
        .as_str()
        .strip_prefix("win:")?
        .parse::<isize>()
        .ok()
        .and_then(|handle| usize::try_from(handle).ok())
}

fn focused_window_is_ours(window: &WindowId) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let Some(handle) = window_handle(window) else {
        return false;
    };
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(HWND(handle as *mut _), Some(&mut process)) };
    process != 0 && process == std::process::id()
}

fn class_name(window: &WindowId) -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let handle = window_handle(window)?;
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(HWND(handle as *mut _), &mut buffer) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..len as usize]))
}

/// Runs `CoUninitialize` only when this call actually opened an apartment.
struct Apartment(bool);

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// What UI Automation says about the focused element, or `None` when it will not
/// answer at all.
///
/// The focused element itself is what carries the password role: a password box
/// sits inside a window that is an ordinary `Chrome_WidgetWin_1` or `Edit`, so
/// asking the window would answer for the whole application rather than for the
/// field the user is dictating into.
///
/// The cost is that the focus query walks the target's accessibility tree, which
/// a foreign target can make slow. It runs once, at hotkey time, before the
/// socket opens, so it delays the utterance rather than a segment.
fn accessibility() -> Option<FocusedElement> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, UIA_IsPasswordPropertyId,
    };

    // `RPC_E_CHANGED_MODE` means someone already owns this thread's apartment
    // in a different mode; the interfaces work either way, and uninitialising
    // would close theirs. Every interface below is dropped before this guard.
    let _apartment = Apartment(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());

    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let element = unsafe { automation.GetFocusedElement() }.ok()?;

    let is_password =
        ui_automation_bool(unsafe { element.GetCurrentPropertyValue(UIA_IsPasswordPropertyId) });
    let name = unsafe { element.CurrentName() }
        .map(|name| name.to_string())
        .unwrap_or_default();

    Some(FocusedElement {
        is_password,
        name,
        ..FocusedElement::default()
    })
}

/// A UI Automation property read as a boolean, or `None` when the element does
/// not support it. Unsupported is a `VT_ERROR` holding `UIA_E_NOTSUPPORTED`, a
/// fatal failure is `DISP_E_TYPEMISMATCH`, and either way the caller learns
/// nothing rather than `false`.
fn ui_automation_bool(value: windows::core::Result<windows::core::VARIANT>) -> Option<bool> {
    // `GetCurrentPropertyValue` returns a `VT_BOOL` for a boolean property.
    bool::try_from(&value.ok()?).ok()
}
