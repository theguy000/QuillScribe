//! What UI Automation says about the focused control.
//!
//! Kept apart from the window reads because it is the expensive one: the focus
//! query walks the target's accessibility tree, which a foreign target can make
//! slow. It runs once, at hotkey time, before the socket opens, so it delays the
//! utterance rather than a segment.

use crate::delivery::FocusedElement;

/// What UI Automation says about the focused element, or `None` when it will not
/// answer at all.
///
/// The focused element itself is what carries the password role: a password box
/// sits inside a window that is an ordinary `Chrome_WidgetWin_1` or `Edit`, so
/// asking the window would answer for the whole application rather than for the
/// field the user is dictating into.
pub(super) fn read() -> Option<FocusedElement> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

    let _apartment = Apartment(initialize());
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let element = unsafe { automation.GetFocusedElement() }.ok()?;

    Some(FocusedElement {
        is_password: read_bool(&element),
        name: read_name(&element),
        ..FocusedElement::default()
    })
}

/// Opens a multithreaded apartment, reporting whether this call opened it.
///
/// `RPC_E_CHANGED_MODE` means someone already owns this thread's apartment in a
/// different mode; the interfaces work either way, and uninitialising would
/// close theirs, so the answer is `false` and the guard does nothing.
fn initialize() -> bool {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok()
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

fn read_name(element: &windows::Win32::UI::Accessibility::IUIAutomationElement) -> String {
    unsafe { element.CurrentName() }
        .map(|name| name.to_string())
        .unwrap_or_default()
}

fn read_bool(element: &windows::Win32::UI::Accessibility::IUIAutomationElement) -> Option<bool> {
    let value = unsafe {
        element.GetCurrentPropertyValue(windows::Win32::UI::Accessibility::UIA_IsPasswordPropertyId)
    };
    ui_automation_bool(value)
}

/// A UI Automation property read as a boolean, or `None` when the element does
/// not support it. Unsupported is a `VT_ERROR` holding `UIA_E_NOTSUPPORTED`, a
/// fatal failure is `DISP_E_TYPEMISMATCH`, and either way the caller learns
/// nothing rather than `false`.
fn ui_automation_bool(value: windows::core::Result<windows::core::VARIANT>) -> Option<bool> {
    // `GetCurrentPropertyValue` returns a `VT_BOOL` for a boolean property.
    bool::try_from(&value.ok()?).ok()
}
