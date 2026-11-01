//! Live typing: committed text written into the application the user is in.
//!
//! An utterance binds to the window that held focus when the recording started.
//! Every Final segment is pasted into that window as soon as the provider
//! commits it. Interim segments never come through here; they keep updating
//! QuillScribe's own panel.
//!
//! The app never reads the target document, so the only memory it has of what
//! is already there is what it typed itself. [`DeliveryLedger`] holds that
//! memory per window, which is what makes fragments read as continuous text
//! across utterances.

use anyhow::Result;
use log::{debug, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::output::OutputManager;

/// The window an utterance is bound to.
///
/// Opaque platform data: an `HWND` on Windows, an X11 window id on Linux. An id
/// is only ever compared with another id read from the same platform.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowId(String);

impl WindowId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// The one platform-specific thing a delivery session needs.
///
/// [`PlatformSurface`] does the real work; the tests use a fake.
pub trait DeliverySurface: Send {
    /// Write `text` at the caret of the window that currently holds focus.
    fn paste(&mut self, text: &str) -> Result<()>;

    /// The window that holds focus now, or `None` when the platform cannot say.
    fn focused_window(&self) -> Option<WindowId>;
}

/// What the app has typed into one window.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowRecord {
    characters: usize,
    ends_with_whitespace: bool,
}

/// Per-window record of what the app has typed.
///
/// The joining rule reads two things from here: whether the app has ever typed
/// into this window, and whether the last character it typed was whitespace. It
/// lives for the life of the process because window identity does not survive a
/// restart, and a fresh process starts with no record, which is what makes the
/// first fragment after a restart arrive without a leading space.
#[derive(Debug, Default)]
pub struct DeliveryLedger {
    windows: HashMap<WindowId, WindowRecord>,
}

impl DeliveryLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// A ledger to keep on the app state for the whole process.
    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new()))
    }

    /// `None` when the app has typed nothing into this window.
    fn ends_with_whitespace(&self, window: &WindowId) -> Option<bool> {
        self.windows
            .get(window)
            .map(|record| record.ends_with_whitespace)
    }

    fn record(&mut self, window: &WindowId, typed: &str) {
        let Some(last) = typed.chars().last() else {
            return;
        };
        let record = self.windows.entry(window.clone()).or_insert(WindowRecord {
            characters: 0,
            ends_with_whitespace: false,
        });
        record.characters += typed.chars().count();
        record.ends_with_whitespace = last.is_whitespace();
        debug!(
            "Delivery ledger: {:?} holds {} characters, last one whitespace: {}",
            window, record.characters, record.ends_with_whitespace
        );
    }
}

/// What live typing managed during one utterance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveryReport {
    /// Words this utterance put into the document, in order.
    pub delivered: String,
    /// Fragments that never arrived, in order.
    pub undelivered: Vec<String>,
}

impl DeliveryReport {
    pub fn undelivered_count(&self) -> usize {
        self.undelivered.len()
    }
}

/// One utterance's delivery into its bound window.
pub struct DeliverySession {
    window: WindowId,
    surface: Box<dyn DeliverySurface>,
    ledger: Arc<Mutex<DeliveryLedger>>,
    delivered: String,
    undelivered: Vec<String>,
}

impl DeliverySession {
    pub fn new(
        window: WindowId,
        surface: impl DeliverySurface + 'static,
        ledger: Arc<Mutex<DeliveryLedger>>,
    ) -> Self {
        Self {
            window,
            surface: Box::new(surface),
            ledger,
            delivered: String::new(),
            undelivered: Vec::new(),
        }
    }

    /// Write one committed segment into the bound window.
    ///
    /// Nothing is written while the bound window is unfocused: the fragment is
    /// held for the stop path rather than typed into whatever the user is
    /// looking at now. A paste that fails is retried once, then the fragment is
    /// counted as undelivered.
    pub fn deliver(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            // An empty or whitespace-only Final segment writes nothing at all.
            return;
        }

        let previous = self
            .ledger
            .lock()
            .ok()
            .and_then(|ledger| ledger.ends_with_whitespace(&self.window));
        let fragment = fragment(previous, text);

        if self.surface.focused_window().as_ref() != Some(&self.window) {
            debug!("Bound window is not focused; holding the fragment for stop");
            self.hold(text);
            return;
        }

        if !self.paste_with_one_retry(&fragment) {
            self.hold(text);
            return;
        }

        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.record(&self.window, &fragment);
        }
        if !self.delivered.is_empty() {
            self.delivered.push(' ');
        }
        self.delivered.push_str(text);
    }

    /// Text this utterance put into the document. The leading space of a later
    /// utterance is not part of it, because the transcript does not have one.
    pub fn report(&self) -> DeliveryReport {
        DeliveryReport {
            delivered: self.delivered.clone(),
            undelivered: self.undelivered.clone(),
        }
    }

    fn paste_with_one_retry(&mut self, fragment: &str) -> bool {
        for attempt in 0..=1 {
            match self.surface.paste(fragment) {
                Ok(()) => return true,
                Err(err) if attempt == 0 => {
                    warn!("Live typing paste failed, retrying once: {}", err);
                }
                Err(err) => {
                    warn!(
                        "Live typing paste failed twice, holding the fragment: {}",
                        err
                    );
                }
            }
        }
        false
    }

    fn hold(&mut self, text: &str) {
        self.undelivered.push(text.to_string());
    }
}

/// The exact text to paste for one committed segment.
///
/// `previous` is the ledger's answer for this window: `None` when the app has
/// never typed into it, `Some(false)` when the last character it typed was not
/// whitespace. The caller has already trimmed `text` and rejected an empty one,
/// so the result never carries whitespace at either end.
fn fragment(previous: Option<bool>, text: &str) -> String {
    let mut fragment = String::with_capacity(text.len() + 1);
    if previous == Some(false) {
        fragment.push(' ');
    }
    fragment.push_str(text);
    fragment
}

/// Everything the utterance still owes the document when the user stops.
///
/// Usually that is the tail after the words that already arrived. When a
/// fragment in the middle was held back, the delivered text is no longer a
/// prefix of the transcript, and the held fragments are what is missing instead.
///
/// The transcript is the authority on what was said; the report holds what
/// actually reached the document. Typing the whole transcript whenever the two
/// disagree would duplicate sentences the user already has, so only the held
/// fragments come back in that case.
pub fn unwritten_words(transcript: &str, report: &DeliveryReport) -> String {
    match tail_after_prefix(transcript, &report.delivered) {
        Some(tail) if !tail.is_empty() => tail,
        Some(_) => report.undelivered.join(" "),
        None => {
            warn!("Live typing and the transcript disagree; stop writes what was held");
            report.undelivered.join(" ")
        }
    }
}

/// The words of `transcript` after `delivered`, or `None` when `delivered` is
/// not a word-for-word prefix of it.
fn tail_after_prefix(transcript: &str, delivered: &str) -> Option<String> {
    let spoken: Vec<&str> = transcript.split_whitespace().collect();
    let typed: Vec<&str> = delivered.split_whitespace().collect();

    if spoken.len() < typed.len() || spoken[..typed.len()] != typed[..] {
        return None;
    }

    Some(spoken[typed.len()..].join(" "))
}

/// Which window holds focus, as far as the platform can tell.
///
/// Read when the utterance starts, because that window owns the whole
/// utterance. `None` means live typing cannot run for this utterance.
pub fn focused_window() -> Option<WindowId> {
    platform_focused_window()
}

/// The real surface: paste the fragment and put the user's clipboard back.
///
/// It pastes rather than types on every platform, so the target application's
/// autocorrect and completion cannot rewrite the characters and nothing depends
/// on the keyboard layout. The session retries a refused fragment once, so this
/// can be asked for the same text twice.
pub struct PlatformSurface {
    output: OutputManager,
}

impl Default for PlatformSurface {
    fn default() -> Self {
        Self {
            output: OutputManager::new(),
        }
    }
}

impl DeliverySurface for PlatformSurface {
    fn paste(&mut self, text: &str) -> Result<()> {
        self.output.paste_preserving_clipboard(text)
    }

    fn focused_window(&self) -> Option<WindowId> {
        focused_window()
    }
}

#[cfg(windows)]
fn platform_focused_window() -> Option<WindowId> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }
    Some(WindowId::new(format!("win:{}", window.0 as isize)))
}

#[cfg(target_os = "linux")]
fn platform_focused_window() -> Option<WindowId> {
    // xdotool is already a dependency of the X11 paste path, so reading the
    // active window costs no new window-system code. Wayland has no portable
    // answer, and without one live typing cannot run.
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

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_focused_window() -> Option<WindowId> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUND: &str = "win:1";
    const OTHER: &str = "win:2";

    /// Stands in for the operating system: records what would have been typed
    /// into the target document, and answers which window has focus.
    #[derive(Clone, Default)]
    struct FakeSurface {
        pasted: Arc<Mutex<Vec<String>>>,
        focus: Arc<Mutex<Option<WindowId>>>,
        failures: Arc<Mutex<usize>>,
    }

    impl FakeSurface {
        fn focused_on(window: &str) -> Self {
            Self {
                focus: Arc::new(Mutex::new(Some(WindowId::new(window)))),
                ..Self::default()
            }
        }

        fn focus_unknown() -> Self {
            Self::default()
        }

        fn failing(mut self, times: usize) -> Self {
            self.failures = Arc::new(Mutex::new(times));
            self
        }

        fn pasted(&self) -> Vec<String> {
            self.pasted.lock().unwrap().clone()
        }
    }

    impl DeliverySurface for FakeSurface {
        fn paste(&mut self, text: &str) -> Result<()> {
            let mut failures = self.failures.lock().unwrap();
            if *failures > 0 {
                *failures -= 1;
                anyhow::bail!("the target refused the paste");
            }
            drop(failures);
            self.pasted.lock().unwrap().push(text.to_string());
            Ok(())
        }

        fn focused_window(&self) -> Option<WindowId> {
            self.focus.lock().unwrap().clone()
        }
    }

    fn session(
        ledger: &Arc<Mutex<DeliveryLedger>>,
        surface: &FakeSurface,
        window: &str,
    ) -> DeliverySession {
        DeliverySession::new(WindowId::new(window), surface.clone(), Arc::clone(ledger))
    }

    #[test]
    fn the_first_fragment_into_a_window_has_no_leading_space() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);

        session(&ledger, &surface, BOUND).deliver("Hello there.");

        assert_eq!(surface.pasted(), ["Hello there."]);
    }

    #[test]
    fn fragments_inside_one_utterance_are_separated_by_one_space() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.");
        session.deliver("How are you?");

        assert_eq!(surface.pasted(), ["Hello there.", " How are you?"]);
        assert_eq!(session.report().delivered, "Hello there. How are you?");
    }

    #[test]
    fn a_second_utterance_in_the_same_window_gets_one_leading_space() {
        let ledger = DeliveryLedger::shared();

        let first = FakeSurface::focused_on(BOUND);
        session(&ledger, &first, BOUND).deliver("Hello there.");

        let second = FakeSurface::focused_on(BOUND);
        session(&ledger, &second, BOUND).deliver("How are you?");

        assert_eq!(first.pasted(), ["Hello there."]);
        assert_eq!(second.pasted(), [" How are you?"]);
    }

    #[test]
    fn windows_keep_separate_records() {
        let ledger = DeliveryLedger::shared();

        let first = FakeSurface::focused_on(BOUND);
        session(&ledger, &first, BOUND).deliver("one");

        let other = FakeSurface::focused_on(OTHER);
        session(&ledger, &other, OTHER).deliver("two");

        let back = FakeSurface::focused_on(BOUND);
        session(&ledger, &back, BOUND).deliver("three");

        assert_eq!(other.pasted(), ["two"], "a new window starts a new record");
        assert_eq!(
            back.pasted(),
            [" three"],
            "the first window kept its record"
        );
    }

    #[test]
    fn a_restart_forgets_what_was_typed_before() {
        let before = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        session(&before, &surface, BOUND).deliver("Hello there.");

        // A new process has an empty ledger: window identity did not survive.
        let after_restart = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        session(&after_restart, &surface, BOUND).deliver("How are you?");

        assert_eq!(surface.pasted(), ["How are you?"]);
    }

    #[test]
    fn a_fragment_never_ends_with_whitespace() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);

        session(&ledger, &surface, BOUND).deliver("Hello there.   ");

        assert_eq!(surface.pasted(), ["Hello there."]);
    }

    #[test]
    fn an_empty_or_whitespace_only_final_writes_nothing() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("");
        session.deliver("   ");
        session.deliver("one");
        session.deliver("  ");
        session.deliver("two");

        assert_eq!(surface.pasted(), ["one", " two"]);
        assert_eq!(session.report().delivered, "one two");
    }

    #[test]
    fn a_failed_paste_is_retried_once_and_then_arrives() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND).failing(1);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.");

        assert_eq!(surface.pasted(), ["Hello there."]);
        assert_eq!(
            session.report(),
            DeliveryReport {
                delivered: "Hello there.".to_string(),
                undelivered: Vec::new(),
            }
        );
    }

    #[test]
    fn a_fragment_that_fails_twice_is_counted_as_undelivered() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND).failing(2);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.");

        assert!(surface.pasted().is_empty());
        let report = session.report();
        assert_eq!(report.delivered, "");
        assert_eq!(report.undelivered_count(), 1);
        assert_eq!(report.undelivered, ["Hello there."]);
    }

    #[test]
    fn nothing_is_typed_while_the_bound_window_is_unfocused() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.");

        assert!(surface.pasted().is_empty());
        assert_eq!(session.report().undelivered, ["Hello there."]);
    }

    #[test]
    fn an_unknown_focus_answer_is_treated_as_unfocused() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focus_unknown();
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.");

        assert!(surface.pasted().is_empty());
        assert_eq!(session.report().undelivered, ["Hello there."]);
    }

    #[test]
    fn stop_owes_only_the_words_that_never_reached_the_document() {
        let report = DeliveryReport {
            delivered: "Hello there.".to_string(),
            undelivered: Vec::new(),
        };

        assert_eq!(
            unwritten_words("Hello there. How are you?", &report),
            "How are you?"
        );
        assert_eq!(unwritten_words("Hello there.", &report), "");
    }

    #[test]
    fn stop_owes_the_whole_transcript_when_nothing_was_delivered() {
        assert_eq!(
            unwritten_words("Hello there.", &DeliveryReport::default()),
            "Hello there."
        );
    }

    #[test]
    fn stop_owes_the_held_fragment_when_the_document_skipped_a_middle_one() {
        let report = DeliveryReport {
            delivered: "one three".to_string(),
            undelivered: vec!["two".to_string()],
        };

        assert_eq!(
            unwritten_words("one two three", &report),
            "two",
            "the skipped words are what still has to be written"
        );
    }
}
