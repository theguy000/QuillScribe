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
use crate::verdict::{target_verdict, Refusal, TargetWindow, Verdict};

mod joining;
mod platform;

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
    fn focused_window_exists(&self) -> bool;

    /// Put `text` on the clipboard as a done deal, replacing whatever the user
    /// had copied. This is copy mode's only way to deliver, and it is never
    /// asked for when the utterance reaches a window instead.
    fn replace_clipboard(&mut self, text: &str) -> Result<()> {
        let _ = text;
        anyhow::bail!("this surface cannot write the clipboard")
    }
}

/// Where an utterance's words go.
///
/// A typed utterance is bound to the window that held focus at hotkey time. A
/// copy-only utterance has no window to bind and targets the clipboard, which
/// is rewritten with the transcript so far after every fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Window {
        id: WindowId,
        described: TargetWindow,
    },
    Clipboard,
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
    /// A refusal that sent the utterance's words away from the target, when one
    /// happened. The words are in `undelivered`, so the stop path takes them.
    pub refused: Option<Refusal>,
    /// Set when the utterance's target was the clipboard rather than a document.
    /// The stop path must not copy the leftovers a second time, because they are
    /// already there.
    pub copies_instead_of_typing: bool,
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
    /// The target must not receive this text at all. It is owed like any other
    /// fragment and the stop path puts it on the clipboard.
    Refused(Refusal),
}

/// One utterance's delivery into its bound window, or onto the clipboard.
pub struct DeliverySession {
    target: Target,
    surface: Box<dyn DeliverySurface>,
    ledger: Arc<Mutex<DeliveryLedger>>,
    /// Fragments that have not reached the target yet, oldest first, including
    /// the one a refused paste came from. In a typed utterance they arrived
    /// while the bound window was unfocused and are written when it comes back,
    /// or by the stop path when it does not. They stay out of `delivered` until
    /// then, so no fragment can be counted as landed and still owed at the same
    /// time.
    held: Vec<String>,
    /// Fragments whose paste was refused both times, oldest first. Nothing said
    /// while they wait can be written behind their words without landing out of
    /// order, so they block the queue until [`Self::finish`] makes the last
    /// attempt for the whole utterance.
    refused: Vec<String>,
    /// Whether the refused queue has already been tried twice while the user was
    /// speaking. The stop path then makes one last attempt, not another pair.
    refused_has_been_retried: bool,
    /// What the platform said about the target when the utterance started. It
    /// decides the shape of every fragment, and refuses the whole utterance when
    /// it is a secret input. `None` when the target is the clipboard.
    described: Option<TargetWindow>,
    /// Why the target refused the utterance, once it has. Set the first time a
    /// fragment meets a masked field, and kept so the stop path can say why the
    /// words went to the clipboard.
    refusal: Option<Refusal>,
    delivered: String,
}

impl DeliverySession {
    /// A session for a target the platform has described, such as a masked
    /// field or a terminal.
    pub fn new(
        window: WindowId,
        target: TargetWindow,
        surface: impl DeliverySurface + 'static,
        ledger: Arc<Mutex<DeliveryLedger>>,
    ) -> Self {
        Self::bound(
            Target::Window {
                id: window,
                described: target,
            },
            surface,
            ledger,
        )
    }

    /// A session for an output mode that copies rather than types.
    ///
    /// There is no window and no ledger: after each fragment the clipboard
    /// holds the transcript so far, and stopping leaves the whole transcript
    /// there. The save-and-restore rule does not apply, because nothing is
    /// pasted. A copy the user makes mid-utterance is overwritten by the next
    /// fragment, which is the price of the clipboard following the speech.
    pub fn copy_to_clipboard(surface: impl DeliverySurface + 'static) -> Self {
        Self::bound(Target::Clipboard, surface, DeliveryLedger::shared())
    }

    fn bound(
        target: Target,
        surface: impl DeliverySurface + 'static,
        ledger: Arc<Mutex<DeliveryLedger>>,
    ) -> Self {
        let described = match &target {
            Target::Window { described, .. } => Some(described.clone()),
            Target::Clipboard => None,
        };
        Self {
            target,
            surface: Box::new(surface),
            ledger,
            held: Vec::new(),
            refused: Vec::new(),
            refused_has_been_retried: false,
            described,
            refusal: None,
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
        // The verdict runs before the trim, because a dictated new line is not
        // whitespace at either end and the terminal rule has to see it.
        let text = match self.admit(text) {
            Ok(text) => text,
            Err(refusal) => return Err(DeliverError::Refused(refusal)),
        };
        let text = text.trim();
        if text.is_empty() {
            // An empty or whitespace-only Final segment writes nothing at all.
            return Ok(());
        }

        if self.target == Target::Clipboard {
            return self.copy_so_far(text);
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

    /// Ask the verdict what this fragment looks like at the target.
    ///
    /// A masked field refuses every fragment, not only the one that first met
    /// it: the utterance is meant for one window, and if the caret is in a
    /// secret input then the words stay owed until the stop path hands them to
    /// the clipboard. A terminal receives line breaks as spaces.
    fn admit(&mut self, text: &str) -> std::result::Result<String, Refusal> {
        match self
            .described
            .as_ref()
            .map(|target| target_verdict(target, text))
        {
            Some(Verdict::Write(text)) => Ok(text),
            Some(Verdict::Refuse(refusal)) => {
                // Refused words are owed, not dropped: the stop path reads the
                // report and puts them on the clipboard.
                if !text.trim().is_empty() {
                    self.held.push(text.trim().to_string());
                }
                if self.refusal != Some(refusal) {
                    debug!("Target refused the fragment: {:?}", refusal);
                    self.refusal = Some(refusal);
                }
                Err(refusal)
            }
            // The clipboard receives everything it is given.
            None => Ok(text.to_string()),
        }
    }

    /// Copy mode: the clipboard becomes the transcript so far.
    ///
    /// The whole utterance is rewritten each time rather than appended to, so
    /// the clipboard always reads as one continuous passage and a retry cannot
    /// double a fragment. Words are joined inside the session and not through
    /// the ledger, because there is no window to remember: the clipboard itself
    /// is the memory.
    fn copy_so_far(&mut self, text: &str) -> Result<(), DeliverError> {
        if !self.refused.is_empty() {
            // An earlier fragment never reached the clipboard, so writing the
            // rest over it would silently drop those words from the copy.
            debug!("Earlier fragments are still owed; holding this one behind them");
            self.held.push(text.to_string());
            return Err(DeliverError::Held);
        }

        self.held.push(text.to_string());
        if self.commit_transcript(1) {
            Ok(())
        } else {
            Err(DeliverError::Held)
        }
    }

    /// Make the clipboard read as the transcript so far plus `more`.
    ///
    /// Resolves the whole text, pastes it, and records it on success. On failure
    /// `more` joins whatever is already owed, so the stop path retries it in
    /// order. `retries_left` is the utterance's budget, the same as for a
    /// window. Returns whether the clipboard now holds the transcript.
    fn commit_transcript(&mut self, retries_left: u8) -> bool {
        let more = std::mem::take(&mut self.held);
        let transcript = self.transcript_with(&more);
        if self.paste_with_one_retry(&transcript, retries_left) {
            self.delivered = transcript;
            return true;
        }
        self.refused.extend(more);
        self.refused_has_been_retried = true;
        false
    }

    /// The delivered words and `more`, as the text a target receives.
    fn transcript_with(&self, more: &[String]) -> String {
        let mut full = self.delivered.clone();
        for text in more {
            if !full.is_empty() {
                full.push(' ');
            }
            full.push_str(text);
        }
        full
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

        let fragment = joining::join(self.ledger_whitespace(), &self.held, self.target.terminal());

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

        self.record_written(&fragment);
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
        match &self.target {
            Target::Window { id, .. } => self.surface.focused_window().as_ref() == Some(id),
            // The clipboard is always there.
            Target::Clipboard => true,
        }
    }

    fn ledger_whitespace(&self) -> Option<bool> {
        let Target::Window { id, .. } = &self.target else {
            return None;
        };
        self.ledger
            .lock()
            .ok()
            .and_then(|ledger| ledger.ends_with_whitespace(id))
    }

    /// Hand `text` to the target: a paste into the window, or the clipboard
    /// rewritten in copy mode.
    fn write(&mut self, text: &str) -> Result<()> {
        match &self.target {
            Target::Window { .. } => self.surface.paste(text),
            Target::Clipboard => self.surface.replace_clipboard(text),
        }
    }

    /// Remember what a window now holds, so the next fragment joins to it.
    fn record_written(&mut self, text: &str) {
        if let Target::Window { id, .. } = &self.target {
            if let Ok(mut ledger) = self.ledger.lock() {
                ledger.record(id, text);
            }
        }
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
            refused: self.refusal,
            copies_instead_of_typing: self.target == Target::Clipboard,
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
    ///
    /// In copy mode the same words are not pasted but leave the clipboard as one
    /// transcript, because the clipboard is where the user will look.
    pub fn finish(&mut self, unwritten: &str) -> DeliveryReport {
        let Some(queue) = self.owed_at_stop(unwritten) else {
            return self.report();
        };

        // Fragments refused while the user spoke already used their retry;
        // everything else is being attempted for the first time here.
        let retries_left = self.retries_at_stop();

        if self.target == Target::Clipboard {
            // The queue is what the clipboard still owes, so it joins the held
            // words the helper reads, and is retried only if it never was.
            self.held = queue;
            if self.commit_transcript(retries_left) {
                debug!("Stop left the whole transcript on the clipboard");
            } else {
                warn!("Stop could not put the transcript on the clipboard");
            }
            return self.report();
        }

        let document_gone = !self.surface.focused_window_exists();
        if document_gone || !self.bound_window_focused() {
            debug!(
                "Stop found the bound window {}; the remaining text takes the clipboard route",
                if document_gone { "gone" } else { "unfocused" }
            );
            // Still owed, so the report hands it to the clipboard.
            self.refused.extend(queue);
            return self.report();
        }

        let fragment = joining::join(self.ledger_whitespace(), &queue, self.target.terminal());
        self.write_owed_at_stop(&fragment, queue, retries_left);
        self.report()
    }

    /// The words the utterance still owes, in the order they were said. The
    /// flushed tail reaches the verdict like any fragment.
    ///
    /// `None` means the stop path has nothing to write: either the target
    /// refused the tail, or nothing at all is owed.
    fn owed_at_stop(&mut self, unwritten: &str) -> Option<Vec<String>> {
        let owed = match self.admit(unwritten) {
            Ok(text) => text.trim().to_string(),
            Err(refusal) => {
                debug!("Target refused the stop tail: {:?}", refusal);
                return None;
            }
        };
        let mut queue: Vec<String> = std::mem::take(&mut self.refused);
        queue.append(&mut self.held);
        if owed.is_empty() {
            return (!queue.is_empty()).then_some(queue);
        }
        queue.push(owed);
        Some(queue)
    }

    /// A retry is owed only to fragments that never had one while the user was
    /// speaking.
    fn retries_at_stop(&self) -> u8 {
        if self.refused_has_been_retried {
            0
        } else {
            1
        }
    }

    /// Paste the queue into the bound window and record which half of it landed.
    fn write_owed_at_stop(&mut self, fragment: &str, queue: Vec<String>, retries_left: u8) {
        if !self.paste_with_one_retry(fragment, retries_left) {
            warn!("Stop could not write what live typing still owed");
            self.refused = queue;
            self.refused_has_been_retried = true;
            return;
        }
        self.record_written(fragment);
        for text in queue {
            if !self.delivered.is_empty() {
                self.delivered.push(' ');
            }
            self.delivered.push_str(&text);
        }
        debug!("Stop wrote what live typing still owed");
    }

    /// Write the owed words once, or twice when they never had their retry.
    ///
    /// `retries_left` is the utterance's budget, not a per-call one: a fragment
    /// refused while the user spoke arrives here with none left, and one that
    /// only ever waited arrives with one.
    fn paste_with_one_retry(&mut self, fragment: &str, retries_left: u8) -> bool {
        for attempt in 0..=retries_left {
            match self.write(fragment) {
                Ok(()) => return true,
                Err(err) if attempt < retries_left => {
                    warn!("Live typing write failed, retrying once: {}", err);
                }
                Err(err) => {
                    warn!(
                        "Live typing write failed, leaving the fragment owed: {}",
                        err
                    );
                }
            }
        }
        false
    }
}

impl Target {
    fn terminal(&self) -> bool {
        match self {
            Target::Window { described, .. } => described.terminal,
            Target::Clipboard => false,
        }
    }
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
    platform::focused_window()
}

/// What the platform can say about where focused text would go.
///
/// `keys` are the API key fields QuillScribe's own window shows in plain text,
/// because the user can reveal them. The caret sitting in one of those fields
/// makes it a secret input even when the order of quotes in the role check
/// happens not to.
///
/// Best effort by construction: where the platform answers nothing, the target
/// reads as an ordinary text field and the app types rather than refusing. That
/// limit is recorded in `docs/live-typing-targets.md`.
pub fn target_window(keys: &[String]) -> TargetWindow {
    platform::target_window(keys)
}

/// Read the target by identifying the focused controls by name.
///
/// The API key inputs are the only text inputs QuillScribe draws, and their
/// placeholders name them. Matching on the description costs no accessibility
/// interface at all, which is why it is the route that always answers for the
/// app's own window.
pub fn named_key_fields() -> Vec<String> {
    KEY_FIELD_NAMES
        .iter()
        .map(|placeholder| placeholder.to_string())
        .collect()
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
        // One helper per platform, because a closed window can leave a stale
        // handle behind on Windows. Where the platform cannot say who holds
        // focus, there is no window to write into, so stop takes the words.
        platform::focused_window_exists()
    }

    fn replace_clipboard(&mut self, text: &str) -> Result<()> {
        // Copy mode owns the clipboard for the utterance: the transcript goes
        // there and stays, so a paste after a pause has everything said.
        self.output.copy_to_clipboard(text)
    }
}

/// What the caret's host says about itself, once the platform has read it.
///
/// Kept apart from the operating-system calls so the masked rule can be checked
/// on any build: a caller supplies whether UI Automation called the focus a
/// secret, the window class, and the element's name. A platform that cannot
/// answer leaves the fields `None` or empty.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Default)]
struct FocusedElement {
    /// UI Automation's `IsPassword`, when it answered at all.
    is_password: Option<bool>,
    /// The focused window's class.
    class_name: String,
    /// The focused element's name. Slint names an empty text input after its
    /// placeholder, which is how the app's own key fields are recognised.
    name: String,
}

impl FocusedElement {
    /// Whether the caret sits in a secret, judged by the platform.
    ///
    /// A read-only value pattern is deliberately not a signal: content and
    /// document hosts report it on ordinary fields, so treating it as a secret
    /// would refuse the editor the user is dictating into.
    fn is_secret_input(&self, keys: &[String]) -> bool {
        self.is_password == Some(true)
            || keys
                .iter()
                .any(|key| !key.is_empty() && self.name.contains(key.as_str()))
    }
}

/// The mask rule and the terminal rule, with the platform readings already done.
#[cfg_attr(not(test), allow(dead_code))]
fn describe_target(element: &FocusedElement, own: bool, keys: &[String]) -> TargetWindow {
    TargetWindow {
        masked: element.is_secret_input(keys),
        terminal: crate::verdict::is_terminal_class(&element.class_name, &element.name),
        own,
    }
}

/// The key-field names the app always recognises, on any platform and any build.
#[allow(dead_code)]
pub const KEY_FIELD_NAMES: &[&str] = &["40-character key", "sk-...", "gsk_...", "AIzaSy..."];

#[cfg(test)]
#[path = "delivery/tests.rs"]
mod tests;
