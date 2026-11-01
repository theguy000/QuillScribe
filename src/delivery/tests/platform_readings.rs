//! What the platform reports about the target, turned into the two rules.

use super::*;

#[test]
fn a_password_role_is_a_masked_target() {
    let element = FocusedElement {
        is_password: Some(true),
        class_name: "Chrome_WidgetWin_1".to_string(),
        ..FocusedElement::default()
    };

    assert!(describe_target(&element, false, &named_key_fields()).masked);
}

#[test]
fn an_unsupported_role_is_not_a_password() {
    let element = FocusedElement {
        is_password: None,
        class_name: "Notepad".to_string(),
        ..FocusedElement::default()
    };

    assert!(!describe_target(&element, false, &named_key_fields()).masked);
}

#[test]
fn quills_own_key_field_is_recognised_by_its_name() {
    let element = FocusedElement {
        // Slint names an empty text input after its placeholder.
        name: "40-character key".to_string(),
        class_name: "SLINT".to_string(),
        ..FocusedElement::default()
    };

    let target = describe_target(&element, true, &named_key_fields());

    assert!(target.masked);
    assert!(target.own, "the window is the app's own");
}

#[test]
fn a_terminal_class_names_a_terminal_target() {
    let element = FocusedElement {
        class_name: "CASCADIA_HOSTING_WINDOW_CLASS".to_string(),
        ..FocusedElement::default()
    };

    assert!(
        describe_target(&element, false, &[]).terminal,
        "a terminal is typed into, with its newlines converted"
    );
}

#[test]
fn a_failed_role_read_still_reports_a_terminal() {
    let element = FocusedElement {
        is_password: None,
        class_name: "xterm".to_string(),
        ..FocusedElement::default()
    };

    let target = describe_target(&element, false, &[]);

    assert!(!target.masked);
    assert!(
        target.terminal,
        "the terminal rule does not need accessibility"
    );
}
