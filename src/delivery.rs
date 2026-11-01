//! Live typing: committed text written into the application the user is in.
//!
//! An utterance binds to the window that held focus when the recording started.
//! Every Final segment is pasted into that window as soon as the provider
//! commits it, except while that window is unfocused: then the fragment is held
//! for as long as the utterance lasts, and written if the window comes back
//! before it ends. Interim segments never come through here; they keep updating
//! QuillScribe's own panel.
//!
//! The app never reads the target document, so the only memory it has of what
//! is already there is what it typed itself. [`DeliveryLedger`] holds that
//! memory per window, which is what makes fragments read as continuous text
//! across utterances.
//!
//! Pressing stop goes through [`DeliverySession::finish`], which writes only
//! what never arrived: the phrase the provider still had buffered, and any
//! fragment a paste refused. Text already in the document is never written
//! again. When the bound window has gone, or focus has moved on or cannot be
//! read, nothing is written and the report keeps the words so the app can put
//! them on the clipboard instead.

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

    /// Whether the window holding focus is still open. A closed window leaves
    /// its identity behind, so at stop this is what tells "the user went
    /// elsewhere" apart from "the document is gone".
    ///
    /// The default only knows what [`Self::focused_window`] says; platforms that
    /// can tell a closed window from a missing answer override it.
    fn focused_window_exists(&self) -> bool {
        self.focused_window().is_some()
    }
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

/// Why writing to the document stopped short.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliverError {
    /// The bound window has no focus, so the fragment was held for the stop
    /// path instead of being typed into whatever the user is looking at.
    Unfocused,
    /// The fragment is owed but not yet written: either its paste was refused,
    /// or words before it are still owed and writing it now would land the
    /// document out of order.
    Held,
}

/// One utterance's delivery into its bound window.
pub struct DeliverySession {
    window: WindowId,
    surface: Box<dyn DeliverySurface>,
    ledger: Arc<Mutex<DeliveryLedger>>,
    /// Fragments that arrived while the bound window was unfocused, oldest
    /// first, including the one a refused paste came from. They are written
    /// when the window comes back, and they are what the stop path writes when
    /// it does not. They stay out of `delivered` until then, so no fragment can
    /// be counted as landed and still owed at the same time.
    held: Vec<String>,
    /// Fragments whose paste was refused both times, oldest first. Nothing said
    /// while they wait can be written behind their words without landing out of
    /// order, so they block the queue until [`Self::finish`] makes the last
    /// attempt for the whole utterance.
    refused: Vec<String>,
    /// Whether the refused queue has already been tried twice while the user was
    /// speaking. The stop path then makes one last attempt, not another pair.
    refused_has_been_retried: bool,
    delivered: String,
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
            held: Vec::new(),
            refused: Vec::new(),
            refused_has_been_retried: false,
            delivered: String::new(),
        }
    }

    /// Write one committed segment into the bound window.
    ///
    /// Nothing is written while the bound window is unfocused: the fragment is
    /// held rather than typed into whatever the user is looking at now. When
    /// the window comes back during the same utterance the held fragments are
    /// written first, in the order they arrived. Whatever is still held at stop
    /// belongs to the stop path. A paste that fails is retried once; when it
    /// fails again the fragment is refused and blocks the ones after it, until
    /// [`Self::finish`] makes the utterance's last attempt. It is never dropped.
    ///
    /// `Ok(())` means the text, and everything queued before it, reached the
    /// document.
    pub fn deliver(&mut self, text: &str) -> Result<(), DeliverError> {
        let text = text.trim();
        if text.is_empty() {
            // An empty or whitespace-only Final segment writes nothing at all.
            return Ok(());
        }

        if !self.bound_window_focused() {
            debug!("Bound window is not focused; holding the fragment for stop");
            self.held.push(text.to_string());
            return Err(DeliverError::Unfocused);
        }

        if !self.refused.is_empty() {
            // The words already in the queue are waiting for the stop path, so
            // this one waits behind them and the document stays in order.
            debug!("Earlier fragments are still owed; holding this one behind them");
            self.held.push(text.to_string());
            return Err(DeliverError::Held);
        }

        // `resume_held` takes this segment too, so the words land in the order
        // they were spoken.
        self.held.push(text.to_string());
        self.resume_held()
    }

    /// Write every fragment held while the bound window was away, oldest first.
    ///
    /// The caller has already checked that the window holds focus. The waiting
    /// fragments go in one paste, because a paste that lands only some of them
    /// would put the words out of order. If that paste is refused they are
    /// marked as refused and every later fragment queues behind them; the stop
    /// path makes the one last attempt.
    fn resume_held(&mut self) -> Result<(), DeliverError> {
        if self.held.is_empty() {
            return Ok(());
        }

        let mut fragment = fragment(self.ledger_whitespace(), &self.held[0]);
        for text in &self.held[1..] {
            fragment.push(' ');
            fragment.push_str(text);
        }

        if !self.paste_with_one_retry(&fragment, 1) {
            warn!(
                "Refusing {} fragment(s) until the stop path retries them",
                self.held.len()
            );
            // They are owed in order, so everything said after them waits
            // behind them rather than landing first.
            self.refused.append(&mut self.held);
            self.refused_has_been_retried = true;
            return Err(DeliverError::Held);
        }

        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.record(&self.window, &fragment);
        }
        let held = std::mem::take(&mut self.held);
        debug!("Bound window is back; delivered {} fragment(s)", held.len());
        for text in held {
            if !self.delivered.is_empty() {
                self.delivered.push(' ');
            }
            self.delivered.push_str(&text);
        }
        Ok(())
    }

    /// Whether the bound window holds focus. An answer the platform cannot give
    /// counts as unfocused, so the app never types blind.
    fn bound_window_focused(&self) -> bool {
        self.surface.focused_window().as_ref() == Some(&self.window)
    }

    fn ledger_whitespace(&self) -> Option<bool> {
        self.ledger
            .lock()
            .ok()
            .and_then(|ledger| ledger.ends_with_whitespace(&self.window))
    }

    /// Text this utterance put into the document. The leading space of a later
    /// utterance is not part of it, because the transcript does not have one.
    ///
    /// Fragments still held when the utterance ends are owed like any other
    /// fragment that never arrived, so the stop path writes them. The count is
    /// what tells the stop path they exist: `unwritten_words` compares the
    /// delivered words with the transcript by position, and held words sit in
    /// the middle of that comparison rather than after it.
    ///
    /// The owed fragments are listed in the order they were spoken. Once a paste
    /// is refused nothing lands until stop, so `held` only ever grows behind
    /// `refused`, never in front of it.
    pub fn report(&self) -> DeliveryReport {
        let mut undelivered: Vec<String> = self.refused.clone();
        undelivered.extend(self.held.iter().cloned());
        DeliveryReport {
            delivered: self.delivered.clone(),
            undelivered,
        }
    }

    /// The stop path: write what the utterance still owes, once.
    ///
    /// `unwritten` is the transcript after the words that already arrived; text
    /// in the document is never written again. It goes after the fragments an
    /// earlier paste left owed, so the document keeps the order the user spoke
    /// in.
    ///
    /// Nothing is written when the bound window has gone or focus sits
    /// somewhere else, including when the platform cannot say which window has
    /// focus. Those words stay in the report, which is what sends them to the
    /// clipboard.
    ///
    /// The owed words go in one paste, the way a resume does, because a paste
    /// that lands only part of them would put the document out of order. A
    /// refused paste gets its retry here if it never had one while the user was
    /// speaking. One that already had its retry is not tried twice again: a
    /// document that refuses writes is not hammered, and the status line stays
    /// honest about what is missing.
    pub fn finish(&mut self, unwritten: &str) -> DeliveryReport {
        let owed = unwritten.trim().to_string();
        if owed.is_empty() && self.refused.is_empty() && self.held.is_empty() {
            return self.report();
        }

        let document_gone = !self.surface.focused_window_exists();
        if document_gone || !self.bound_window_focused() {
            debug!(
                "Stop found the bound window {}; the remaining text takes the clipboard route",
                if document_gone { "gone" } else { "unfocused" }
            );
            if !owed.is_empty() {
                // Still owed, so the report hands it to the clipboard.
                self.held.push(owed);
            }
            return self.report();
        }

        // The words the utterance still owes, in the order they were said.
        let mut queue: Vec<String> = std::mem::take(&mut self.refused);
        queue.append(&mut self.held);
        if !owed.is_empty() {
            queue.push(owed);
        }

        let Some(first) = queue.first() else {
            return self.report();
        };
        let mut fragment = fragment(self.ledger_whitespace(), first);
        for text in &queue[1..] {
            fragment.push(' ');
            fragment.push_str(text);
        }

        // Fragments refused while the user spoke already used their retry;
        // everything else is being attempted for the first time here.
        let retries_left = if self.refused_has_been_retried { 0 } else { 1 };
        let landed = self.paste_with_one_retry(&fragment, retries_left);

        if landed {
            if let Ok(mut ledger) = self.ledger.lock() {
                ledger.record(&self.window, &fragment);
            }
            for text in queue {
                if !self.delivered.is_empty() {
                    self.delivered.push(' ');
                }
                self.delivered.push_str(&text);
            }
            debug!("Stop wrote what live typing still owed");
        } else {
            warn!("Stop could not write what live typing still owed");
            self.refused = queue;
            self.refused_has_been_retried = true;
        }

        self.report()
    }

    /// Paste the owed words once, or twice when they never had their retry.
    ///
    /// `retries_left` is the utterance's budget, not a per-call one: a fragment
    /// refused while the user spoke arrives here with none left, and one that
    /// only ever waited arrives with one.
    fn paste_with_one_retry(&mut self, fragment: &str, retries_left: u8) -> bool {
        for attempt in 0..=retries_left {
            match self.surface.paste(fragment) {
                Ok(()) => return true,
                Err(err) if attempt < retries_left => {
                    warn!("Live typing paste failed, retrying once: {}", err);
                }
                Err(err) => {
                    warn!(
                        "Live typing paste failed, leaving the fragment owed: {}",
                        err
                    );
                }
            }
        }
        false
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
    let spoken: Vec<&str> = transcript.split_whitespace().collect();
    let typed: Vec<&str> = report.delivered.split_whitespace().collect();

    // A fragment that never arrived leaves a gap, so what is typed is no longer
    // a prefix of what was said. Lift those words out and compare the rest: a
    // prefix that covers the rest means the tail after it is still owed.
    let Some(rest) = without_words(&spoken, &report.undelivered) else {
        warn!("Live typing and the transcript disagree; stop writes what was held");
        return report.undelivered.join(" ");
    };
    if rest.len() < typed.len() || rest[..typed.len()] != typed[..] {
        warn!("Live typing and the transcript disagree; stop writes what was held");
        return report.undelivered.join(" ");
    }

    let mut owed = report.undelivered.join(" ");
    let tail = rest[typed.len()..].join(" ");
    if !tail.is_empty() {
        if !owed.is_empty() {
            owed.push(' ');
        }
        owed.push_str(&tail);
    }
    owed
}

/// `words` as it reads once each held fragment is taken out, in order. `None`
/// when a held fragment does not appear where the utterance said it.
fn without_words<'a>(words: &[&'a str], held: &[String]) -> Option<Vec<&'a str>> {
    let mut rest = words.to_vec();
    for fragment in held {
        let fragment: Vec<&str> = fragment.split_whitespace().collect();
        let start = (0..=rest.len().saturating_sub(fragment.len()))
            .find(|start| rest[*start..*start + fragment.len()] == fragment[..])?;
        rest.drain(start..start + fragment.len());
    }
    Some(rest)
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

    fn focused_window_exists(&self) -> bool {
        platform_focused_window_exists()
    }
}

#[cfg(windows)]
fn platform_focused_window_exists() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow};

    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return false;
    }
    // A closed window leaves a stale handle behind, so "still focused" has to
    // ask the window manager whether the handle is still a window at all.
    unsafe { IsWindow(window).as_bool() }
}

#[cfg(target_os = "linux")]
fn platform_focused_window_exists() -> bool {
    let output = match std::process::Command::new("xdotool")
        .args(["getwindowpid", "@"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    output.status.success()
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

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_focused_window_exists() -> bool {
    false
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
        /// Whether the focused window is still a live window.
        exists: Arc<Mutex<bool>>,
    }

    impl FakeSurface {
        fn focused_on(window: &str) -> Self {
            Self {
                focus: Arc::new(Mutex::new(Some(WindowId::new(window)))),
                exists: Arc::new(Mutex::new(true)),
                ..Self::default()
            }
        }

        fn focus_unknown() -> Self {
            Self::default()
        }

        /// The user clicks another window, or back onto this one.
        fn focus_on(&self, window: &str) {
            *self.focus.lock().unwrap() = Some(WindowId::new(window));
        }

        /// The bound window is closed under the utterance.
        fn close(&self) {
            *self.focus.lock().unwrap() = None;
            *self.exists.lock().unwrap() = false;
        }

        /// The target refuses the next `times` pastes, without changing focus.
        fn fail_next(&self, times: usize) {
            *self.failures.lock().unwrap() = times;
        }

        fn failing(mut self, times: usize) -> Self {
            self.failures = Arc::new(Mutex::new(times));
            self.exists = Arc::new(Mutex::new(true));
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

        fn focused_window_exists(&self) -> bool {
            *self.exists.lock().unwrap()
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

        let _ = session(&ledger, &surface, BOUND).deliver("Hello there.");

        assert_eq!(surface.pasted(), ["Hello there."]);
    }

    #[test]
    fn fragments_inside_one_utterance_are_separated_by_one_space() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("Hello there.");
        let _ = session.deliver("How are you?");

        assert_eq!(surface.pasted(), ["Hello there.", " How are you?"]);
        assert_eq!(session.report().delivered, "Hello there. How are you?");
    }

    #[test]
    fn a_second_utterance_in_the_same_window_gets_one_leading_space() {
        let ledger = DeliveryLedger::shared();

        let first = FakeSurface::focused_on(BOUND);
        let _ = session(&ledger, &first, BOUND).deliver("Hello there.");

        let second = FakeSurface::focused_on(BOUND);
        let _ = session(&ledger, &second, BOUND).deliver("How are you?");

        assert_eq!(first.pasted(), ["Hello there."]);
        assert_eq!(second.pasted(), [" How are you?"]);
    }

    #[test]
    fn windows_keep_separate_records() {
        let ledger = DeliveryLedger::shared();

        let first = FakeSurface::focused_on(BOUND);
        let _ = session(&ledger, &first, BOUND).deliver("one");

        let other = FakeSurface::focused_on(OTHER);
        let _ = session(&ledger, &other, OTHER).deliver("two");

        let back = FakeSurface::focused_on(BOUND);
        let _ = session(&ledger, &back, BOUND).deliver("three");

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
        let _ = session(&before, &surface, BOUND).deliver("Hello there.");

        // A new process has an empty ledger: window identity did not survive.
        let after_restart = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let _ = session(&after_restart, &surface, BOUND).deliver("How are you?");

        assert_eq!(surface.pasted(), ["How are you?"]);
    }

    #[test]
    fn a_fragment_never_ends_with_whitespace() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);

        let _ = session(&ledger, &surface, BOUND).deliver("Hello there.   ");

        assert_eq!(surface.pasted(), ["Hello there."]);
    }

    #[test]
    fn an_empty_or_whitespace_only_final_writes_nothing() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("");
        let _ = session.deliver("   ");
        let _ = session.deliver("one");
        let _ = session.deliver("  ");
        let _ = session.deliver("two");

        assert_eq!(surface.pasted(), ["one", " two"]);
        assert_eq!(session.report().delivered, "one two");
    }

    #[test]
    fn a_failed_paste_is_retried_once_and_then_arrives() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND).failing(1);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("Hello there.");

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

        let _ = session.deliver("Hello there.");

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

        let _ = session.deliver("Hello there.");

        assert!(
            surface.pasted().is_empty(),
            "nothing reaches whichever window the user is in now"
        );
    }

    #[test]
    fn returning_to_the_bound_window_delivers_the_held_fragment() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("Hello there.");
        surface.focus_on(BOUND);
        let _ = session.deliver("How are you?");

        assert_eq!(
            surface.pasted(),
            ["Hello there. How are you?"],
            "the held words land when the window comes back, in the order they were spoken"
        );
        assert_eq!(session.report().delivered, "Hello there. How are you?");
        assert_eq!(session.report().undelivered_count(), 0);
    }

    #[test]
    fn held_fragments_arrive_in_the_order_they_were_spoken() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("one");
        let _ = session.deliver("two");
        surface.focus_on(BOUND);
        let _ = session.deliver("three");

        assert_eq!(surface.pasted(), ["one two three"]);
        assert_eq!(session.report().delivered, "one two three");
    }

    #[test]
    fn a_window_that_was_never_bound_never_receives_text() {
        let ledger = DeliveryLedger::shared();
        let bound = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &bound, BOUND);

        let _ = session.deliver("one");
        bound.focus_on(OTHER);
        let _ = session.deliver("two");
        let _ = session.deliver("three");
        bound.focus_on(BOUND);
        let _ = session.deliver("four");

        assert_eq!(
            bound.pasted(),
            ["one", " two three four"],
            "the words said elsewhere arrive late, in order, never there"
        );
        assert_eq!(session.report().delivered, "one two three four");
    }

    #[test]
    fn words_held_at_stop_are_owed_to_the_stop_path() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("one");
        let _ = session.deliver("two");

        let report = session.report();
        assert_eq!(
            unwritten_words("one two", &report),
            "one two",
            "the stop path writes what never landed"
        );
        assert_eq!(
            report.undelivered_count(),
            2,
            "the stop path only knows to write them because the count says so"
        );
    }

    #[test]
    fn a_held_fragment_in_the_middle_is_not_typed_twice_at_stop() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("one");
        surface.focus_on(OTHER);
        let _ = session.deliver("two");
        surface.focus_on(BOUND);
        // The user stops before another segment gives the held words a chance.
        let report = session.report();

        assert_eq!(
            unwritten_words("one two three", &report),
            "two three",
            "the held words and the tail come back, and nothing already in the document does"
        );
    }

    #[test]
    fn stop_in_the_middle_of_a_sentence_owes_only_the_tail() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("one");

        assert_eq!(
            unwritten_words("one two three", &session.report()),
            "two three"
        );
    }

    #[test]
    fn a_paste_refused_on_resume_does_not_lose_the_held_words() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("one");
        surface.focus_on(BOUND);
        surface.fail_next(2);
        let _ = session.deliver("two");

        assert!(
            surface.pasted().is_empty(),
            "a refused resume writes nothing at all"
        );
        assert_eq!(
            session.report().undelivered,
            ["one", "two"],
            "the words stay owed rather than arriving out of order"
        );
    }

    #[test]
    fn a_paste_refused_on_resume_lands_later_at_stop() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("one");
        surface.focus_on(BOUND);
        surface.fail_next(2);
        let _ = session.deliver("two");
        let _ = session.deliver("three");

        // The refused fragment already spent its retry, so nothing else is
        // written behind its back; what waits comes back at stop, in order.
        assert!(surface.pasted().is_empty());
        let report = session.finish("");

        assert_eq!(
            surface.pasted(),
            ["one two three"],
            "the single stop attempt writes all three in order"
        );
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn an_unknown_focus_answer_is_treated_as_unfocused() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focus_unknown();
        let mut session = session(&ledger, &surface, BOUND);

        let _ = session.deliver("Hello there.");

        assert!(
            surface.pasted().is_empty(),
            "the app never types blind on a focus answer it cannot read"
        );
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

    // ── Stop reconciliation ─────────────────────────────────────────────

    #[test]
    fn stopping_mid_sentence_writes_the_buffered_tail_and_nothing_else() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("one").unwrap();
        // Finalize flushes the phrase still in the provider's buffer.
        let report = session.finish("two three");

        assert_eq!(
            surface.pasted(),
            ["one", " two three"],
            "the flushed tail arrives after the sentence already in the document"
        );
        assert_eq!(report.delivered, "one two three");
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn text_already_in_the_document_is_never_written_again_at_stop() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.").unwrap();
        let report = session.finish("");

        assert_eq!(
            surface.pasted(),
            ["Hello there."],
            "a clean stop adds one paste for nothing"
        );
        assert_eq!(report.delivered, "Hello there.");
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn a_fragment_whose_paste_failed_arrives_at_stop_after_one_retry() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND).failing(2);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("Hello there.").unwrap_err();
        // The stop path's single attempt succeeds now that the target accepts it.
        let report = session.finish("");

        assert_eq!(surface.pasted(), ["Hello there."]);
        assert_eq!(report.delivered, "Hello there.");
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn a_fragment_only_ever_held_gets_its_retry_at_stop() {
        let ledger = DeliveryLedger::shared();
        // The user is elsewhere for the first phrase, then comes back just as the
        // target starts refusing pastes. This fragment never had an attempt of
        // its own, so the stop retry is the one that counts.
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        assert_eq!(session.deliver("one"), Err(DeliverError::Unfocused));
        surface.focus_on(BOUND);

        // One refusal, then success: that is exactly one retry, and it arrives.
        surface.fail_next(1);
        let report = session.finish("two");

        assert_eq!(surface.pasted(), ["one two"]);
        assert_eq!(report.delivered, "one two");
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn a_fragment_refused_at_stop_is_not_hammered_and_is_reported() {
        let ledger = DeliveryLedger::shared();
        // Two refusals during the utterance, then the target refuses the stop
        // attempt too: the app gives up rather than trying again and again.
        let surface = FakeSurface::focused_on(BOUND).failing(3);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("one").unwrap_err();
        let report = session.finish("two");

        assert!(surface.pasted().is_empty());
        assert_eq!(report.undelivered, ["one", "two"]);
    }

    #[test]
    fn a_refused_fragment_blocks_later_words_until_stop_writes_them_in_order() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND).failing(2);
        let mut session = session(&ledger, &surface, BOUND);

        assert_eq!(session.deliver("one"), Err(DeliverError::Held));
        // The user keeps speaking: "two" must not jump the queue.
        assert_eq!(session.deliver("two"), Err(DeliverError::Held));
        assert!(surface.pasted().is_empty());

        let report = session.finish("three");

        assert_eq!(
            surface.pasted(),
            ["one two three"],
            "the stop path writes what never arrived as one continuous fragment"
        );
        assert_eq!(report.delivered, "one two three");
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn a_closed_bound_window_sends_the_remaining_text_to_the_clipboard_route() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("one").unwrap();
        surface.close();

        let report = session.finish("two three");

        assert_eq!(
            surface.pasted(),
            ["one"],
            "a window that is gone is not where the rest lands"
        );
        assert_eq!(
            unwritten_words("one two three", &report),
            "two three",
            "the words the stop path has to put on the clipboard"
        );
        assert_eq!(
            report.undelivered,
            ["two three"],
            "one joined fragment is what the clipboard receives"
        );
    }

    #[test]
    fn stop_never_types_into_another_window() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        session.deliver("one").unwrap_err();
        let report = session.finish("two");

        assert!(
            surface.pasted().is_empty(),
            "nothing goes into whichever window the user happens to be in"
        );
        assert_eq!(unwritten_words("one two", &report), "one two");
    }

    #[test]
    fn a_held_fragment_waits_for_stop_without_burning_its_retry() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(OTHER);
        let mut session = session(&ledger, &surface, BOUND);

        // Held because the bound window is unfocused; nothing was refused.
        assert_eq!(session.deliver("one"), Err(DeliverError::Unfocused));
        surface.focus_on(BOUND);

        let report = session.finish("two");

        assert_eq!(
            surface.pasted(),
            ["one two"],
            "the stop path retries what it can, with the separated prefix kept"
        );
        assert_eq!(report.delivered, "one two");
        assert_eq!(report.undelivered_count(), 0);
    }

    #[test]
    fn stop_keeps_the_joining_space_for_a_previous_utterance() {
        let ledger = DeliveryLedger::shared();

        let first = FakeSurface::focused_on(BOUND);
        session(&ledger, &first, BOUND).deliver("one").unwrap();

        let second = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &second, BOUND);
        let report = session.finish("two");

        assert_eq!(first.pasted(), ["one"]);
        assert_eq!(
            second.pasted(),
            [" two"],
            "the stop path joins to the previous utterance like any fragment"
        );
        assert_eq!(report.delivered, "two");
    }

    #[test]
    fn an_utterance_that_never_spoke_writes_nothing_at_stop() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focused_on(BOUND);
        let mut session = session(&ledger, &surface, BOUND);

        let report = session.finish("   ");

        assert!(surface.pasted().is_empty());
        assert_eq!(report, DeliveryReport::default());
    }

    #[test]
    fn stop_with_no_bound_window_never_writes() {
        let ledger = DeliveryLedger::shared();
        let surface = FakeSurface::focus_unknown();
        let mut session = session(&ledger, &surface, BOUND);

        let report = session.finish("one two");

        assert!(surface.pasted().is_empty());
        assert_eq!(unwritten_words("one two", &report), "one two");
    }
}
