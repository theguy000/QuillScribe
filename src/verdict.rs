//! The target verdict: whether a fragment may be written where it is pointed.
//!
//! Live typing writes into another application, and some targets must never
//! receive dictated words. This module decides that question from a description
//! of the target, so the rule is a pure function the tests can drive without a
//! window. The platform reads the description; nothing here talks to an
//! operating system.
//!
//! Two refusals exist, both decided in target safety (ticket 05):
//!
//! - Masked and secret inputs, detected best effort from the accessibility role,
//!   plus QuillScribe's own key fields, which the app knows directly.
//! - Nothing else. Terminals and elevated windows are written to.
//!
//! The one rewrite is the newline rule: a dictated new line becomes a space in a
//! terminal, where a pasted newline can run whatever was just dictated.
//!
//! Refused text is not dropped. The delivery session keeps it owed, and the stop
//! path puts it on the clipboard, which is the same route as text the document
//! never took.

/// What the platform managed to learn about the window a fragment would go to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetWindow {
    /// The field that holds the caret is a password box, a secret input, or one
    /// of QuillScribe's own key fields.
    pub masked: bool,
    /// Writing a new line here could run a command rather than insert a mark.
    pub terminal: bool,
    /// The window belongs to QuillScribe itself.
    pub own: bool,
}

/// The descriptions the tests and the callers name.
#[cfg_attr(not(test), allow(dead_code))]
impl TargetWindow {
    /// An ordinary text field in another application.
    pub fn text_field() -> Self {
        Self::default()
    }

    pub fn masked() -> Self {
        Self {
            masked: true,
            ..Self::default()
        }
    }

    pub fn terminal() -> Self {
        Self {
            terminal: true,
            ..Self::default()
        }
    }

    /// QuillScribe's own window with an ordinary field focused.
    pub fn own() -> Self {
        Self {
            own: true,
            ..Self::default()
        }
    }

    /// QuillScribe's own API key field, which is a masked input like any other.
    pub fn own_key_field() -> Self {
        Self {
            masked: true,
            own: true,
            ..Self::default()
        }
    }

    /// Whether the window is QuillScribe's own. The refusal does not depend on
    /// it; it is what tells a log line whether the app is dictating into itself.
    #[allow(dead_code)]
    pub fn is_own(&self) -> bool {
        self.own
    }
}

/// What a fragment deserves once the target has been read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Write this text. A dictated new line has already become a space when the
    /// target is a terminal.
    Write(String),
    /// Write nothing here. The words stay owed and the stop path puts them on
    /// the clipboard.
    Refuse(Refusal),
}

/// Why the app would not write into the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The caret sits in a masked or secret input.
    SecretInput,
}

impl Refusal {
    /// The status-line wording for text this refusal sent to the clipboard.
    pub fn user_message(&self) -> &'static str {
        match self {
            Refusal::SecretInput => "refused for the masked field",
        }
    }
}

/// Decide whether `text` may be written into `target`, and in what shape.
///
/// The refusal comes first: nothing is written into a secret input, newline or
/// not. Otherwise a terminal receives every line break as a space, so a dictated
/// `new line` cannot execute a command, and everywhere else the text is written
/// exactly as the provider committed it.
///
/// When the platform cannot report a role it has nothing to describe, which
/// reads as an ordinary text field: the app types rather than refusing.
pub fn target_verdict(target: &TargetWindow, text: &str) -> Verdict {
    if target.masked {
        return Verdict::Refuse(Refusal::SecretInput);
    }
    if target.terminal {
        return Verdict::Write(newlines_to_spaces(text));
    }
    Verdict::Write(text.to_string())
}

/// Every line break, in any of its spellings, becomes one space. Runs of
/// whitespace elsewhere are left for the delivery session's trimming.
fn newlines_to_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            if !in_break {
                out.push(' ');
            }
            in_break = true;
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

/// The role bits of a window class that say the target is a terminal.
///
/// Kept platform free so the rule is one rule: the window classes registered by
/// the consoles and terminals the app knows about, plus the interface an
/// elevated or remote window presents. The caller supplies the class name with
/// any application-id prefix removed, in either case.
pub fn is_terminal_class(class: &str, title: &str) -> bool {
    const TERMINAL_CLASSES: &[&str] = &[
        // Windows consoles and terminals.
        "consolewindowclass",
        "cascadia_hosting_window_class",
        "mintty",
        "putty",
        "windowsterminal",
        "conhost",
        // X11 and Wayland terminals.
        "gnome-terminal",
        "gnome-terminal-server",
        "org.gnome.terminal",
        "org.gnome.console",
        "konsole",
        "org.kde.konsole",
        "xterm",
        "uxterm",
        "urxvt",
        "rxvt",
        "kitty",
        "org.wezfurlong.kitty",
        "alacritty",
        "org.alacritty",
        "wezterm",
        "org.wezfurlong.wezterm",
        "foot",
        "org.codeberg.dnkl.foot",
        "tilix",
        "org.gnome.tilix",
        "terminator",
        "xfce4-terminal",
        "lxterminal",
        "terminology",
        "org.lxqt.qterminal",
        "qterminal",
        "st",
        "tmux",
    ];

    let class = class.trim().to_ascii_lowercase();
    if TERMINAL_CLASSES.contains(&class.as_str()) {
        return true;
    }
    // A terminal's title often names the session rather than the program, and
    // Windows Terminal announces itself only that way in a hosted pane.
    let title = title.trim().to_ascii_lowercase();
    title.starts_with("windows terminal")
        || title.ends_with(" - tmux")
        || title.starts_with("tmux:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(target: &TargetWindow, text: &str) -> String {
        match target_verdict(target, text) {
            Verdict::Write(text) => text,
            Verdict::Refuse(refusal) => panic!("refused as {:?} instead of writing", refusal),
        }
    }

    fn refused_as(target: &TargetWindow, text: &str) -> Refusal {
        match target_verdict(target, text) {
            Verdict::Refuse(refusal) => refusal,
            Verdict::Write(text) => panic!("wrote {:?} instead of refusing", text),
        }
    }

    #[test]
    fn a_masked_field_receives_nothing() {
        assert_eq!(
            refused_as(&TargetWindow::masked(), "my password is hunter two"),
            Refusal::SecretInput
        );
    }

    #[test]
    fn quills_own_api_key_field_is_refused_like_any_other_masked_field() {
        assert_eq!(
            refused_as(&TargetWindow::own_key_field(), " forty character key "),
            Refusal::SecretInput
        );
        assert_eq!(
            target_verdict(&TargetWindow::own_key_field(), "key"),
            target_verdict(&TargetWindow::masked(), "key"),
            "the own-window rule adds nothing and takes nothing away"
        );
    }

    #[test]
    fn a_dictated_new_line_is_a_space_in_a_terminal() {
        assert_eq!(
            written(&TargetWindow::terminal(), "first line\nsecond line"),
            "first line second line"
        );
    }

    #[test]
    fn a_dictated_new_line_stays_a_new_line_everywhere_else() {
        assert_eq!(
            written(&TargetWindow::text_field(), "first line\nsecond line"),
            "first line\nsecond line"
        );
        assert_eq!(
            written(&TargetWindow::own(), "first line\nsecond line"),
            "first line\nsecond line"
        );
    }

    #[test]
    fn a_paragraph_break_in_a_terminal_is_one_space() {
        assert_eq!(
            written(&TargetWindow::terminal(), "one\n\ntwo"),
            "one two",
            "a paragraph break is still one line break, so it is still one space"
        );
        assert_eq!(
            written(&TargetWindow::terminal(), "one\r\n\r\ntwo"),
            "one two",
            "a CRLF pair is one line break, so a Windows paragraph break is one space"
        );
    }

    #[test]
    fn a_terminal_that_is_not_masked_still_receives_text() {
        assert_eq!(
            written(&TargetWindow::terminal(), "ls -la"),
            "ls -la",
            "the operator cleared terminals, so they are written to"
        );
    }

    #[test]
    fn a_masked_terminal_receives_nothing_at_all() {
        let masked_terminal = TargetWindow {
            masked: true,
            terminal: true,
            own: false,
        };

        assert_eq!(
            refused_as(&masked_terminal, "one\ntwo"),
            Refusal::SecretInput
        );
    }

    #[test]
    fn a_target_no_one_could_describe_has_text_typed_into_it() {
        // What the platform reports when the accessibility role is unreadable:
        // nothing is known, so nothing is refused.
        let unknown = TargetWindow::default();

        assert_eq!(
            written(&unknown, "password: hunter two"),
            "password: hunter two"
        );
        assert_eq!(written(&unknown, "line\nbreak"), "line\nbreak");
    }

    #[test]
    fn windows_consoles_and_terminals_are_recognised() {
        for class in [
            "ConsoleWindowClass",
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "mintty",
            "PuTTY",
            "Konsole",
            "xterm",
        ] {
            assert!(is_terminal_class(class, ""), "{class} should be a terminal");
        }
        assert!(is_terminal_class(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows Terminal"
        ));
    }

    #[test]
    fn an_editor_is_not_mistaken_for_a_terminal() {
        for class in ["Notepad", "Chrome_WidgetWin_1", "gvim", "Code"] {
            assert!(!is_terminal_class(class, "notes.txt - Notepad"));
        }
    }
}
