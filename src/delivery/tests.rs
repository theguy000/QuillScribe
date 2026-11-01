//! Behaviour of one utterance's delivery, against a fake platform.

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
    session_on(ledger, surface, window, TargetWindow::text_field())
}

/// The same, with what the platform said about the target.
fn session_on(
    ledger: &Arc<Mutex<DeliveryLedger>>,
    surface: &FakeSurface,
    window: &str,
    target: TargetWindow,
) -> DeliverySession {
    DeliverySession::new(
        WindowId::new(window),
        target,
        surface.clone(),
        Arc::clone(ledger),
    )
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
            refused: None,
            copies_instead_of_typing: false,
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
        refused: None,
        copies_instead_of_typing: false,
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
        refused: None,
        copies_instead_of_typing: false,
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

// ── Copy mode: the clipboard is the target ──────────────────────────

/// The copy-mode target: the clipboard itself. There is no window to bind,
/// so `focus` stays unknown and no paste is ever asked for.
#[derive(Clone, Default)]
struct FakeClipboard {
    text: Arc<Mutex<Option<String>>>,
    failures: Arc<Mutex<usize>>,
}

impl FakeClipboard {
    fn text(&self) -> Option<String> {
        self.text.lock().unwrap().clone()
    }
}

impl DeliverySurface for FakeClipboard {
    fn paste(&mut self, _text: &str) -> Result<()> {
        anyhow::bail!("copy mode never pastes")
    }

    fn focused_window(&self) -> Option<WindowId> {
        None
    }

    fn focused_window_exists(&self) -> bool {
        false
    }

    fn replace_clipboard(&mut self, text: &str) -> Result<()> {
        let mut failures = self.failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            anyhow::bail!("the clipboard refused the write");
        }
        drop(failures);
        *self.text.lock().unwrap() = Some(text.to_string());
        Ok(())
    }
}

fn copy_session(surface: &FakeClipboard) -> DeliverySession {
    DeliverySession::copy_to_clipboard(surface.clone())
}

#[test]
fn the_clipboard_holds_the_transcript_so_far_after_each_final() {
    let clipboard = FakeClipboard::default();
    let mut session = copy_session(&clipboard);

    let _ = session.deliver("Hello there.");
    assert_eq!(clipboard.text().as_deref(), Some("Hello there."));

    let _ = session.deliver("How are you?");
    assert_eq!(
        clipboard.text().as_deref(),
        Some("Hello there. How are you?"),
        "each final replaces the clipboard with everything said so far"
    );

    let report = session.finish("Fine thanks.");
    assert_eq!(
        clipboard.text().as_deref(),
        Some("Hello there. How are you? Fine thanks."),
        "the flushed tail joins the transcript on the clipboard"
    );
    assert_eq!(report.delivered, "Hello there. How are you? Fine thanks.");
    assert_eq!(report.undelivered_count(), 0);
}

#[test]
fn copy_mode_leaves_the_clipboard_alone_when_nothing_was_said() {
    let clipboard = FakeClipboard::default();
    let mut session = copy_session(&clipboard);

    let report = session.finish("   ");

    assert_eq!(clipboard.text(), None);
    assert_eq!(
        report,
        DeliveryReport {
            copies_instead_of_typing: true,
            ..DeliveryReport::default()
        }
    );
}

#[test]
fn a_refused_copy_keeps_the_spoken_order_when_stop_writes_it() {
    // The first fragment fails its retry, so it is owed. The next one
    // queues behind it, and the tail comes last. Stop must write them in
    // the order they were said, never the owed one on the end.
    let clipboard = FakeClipboard {
        failures: Arc::new(Mutex::new(2)),
        ..FakeClipboard::default()
    };
    let mut session = copy_session(&clipboard);

    let _ = session.deliver("one");
    assert_eq!(clipboard.text(), None, "both attempts failed");

    let _ = session.deliver("two");
    assert_eq!(clipboard.text(), None, "it queues behind the owed words");

    let report = session.finish("three");
    assert_eq!(
        clipboard.text().as_deref(),
        Some("one two three"),
        "stop restores the spoken order on the clipboard"
    );
    assert_eq!(report.undelivered_count(), 0);
}

#[test]
fn a_clipboard_write_that_fails_is_retried_and_then_owed() {
    let clipboard = FakeClipboard::default();
    *clipboard.failures.lock().unwrap() = 2;
    let mut session = copy_session(&clipboard);

    let _ = session.deliver("Hello there.");

    assert_eq!(clipboard.text(), None);
    let report = session.report();
    assert_eq!(report.undelivered, ["Hello there."]);
}

#[test]
fn a_clipboard_that_refuses_every_write_never_loses_the_words() {
    let clipboard = FakeClipboard::default();
    let mut session = copy_session(&clipboard);
    *clipboard.failures.lock().unwrap() = 2;
    let _ = session.deliver("one");
    *clipboard.failures.lock().unwrap() = 1;
    let report = session.finish("two");

    assert_eq!(
        clipboard.text(),
        None,
        "the app does not claim text it could not put there"
    );
    assert_eq!(unwritten_words("one two", &report), "one two");
}

// ── The target verdict ──────────────────────────────────────────────

#[test]
fn a_masked_field_receives_nothing_and_owes_the_words_to_the_clipboard() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::masked());

    assert_eq!(
        session.deliver("my password is hunter two"),
        Err(DeliverError::Refused(Refusal::SecretInput))
    );
    // The user stops without another segment to give the stop path a chance.
    let report = session.finish("");

    assert!(
        surface.pasted().is_empty(),
        "a secret input is never written to, even by the stop path"
    );
    assert_eq!(
        unwritten_words("my password is hunter two", &report),
        "my password is hunter two",
        "the words go to the clipboard instead of being dropped"
    );
    assert_eq!(report.refused, Some(Refusal::SecretInput));
}

#[test]
fn quills_own_key_field_is_refused_the_way_any_masked_field_is() {
    let ledger = DeliveryLedger::shared();

    let elsewhere = FakeSurface::focused_on(BOUND);
    let mut masked_elsewhere = session_on(&ledger, &elsewhere, BOUND, TargetWindow::masked());
    let masked_elsewhere_result = masked_elsewhere.deliver("a key");

    // The app's own window is only special by being known directly.
    let app = FakeSurface::focused_on(BOUND);
    let mut own_key = session_on(&ledger, &app, BOUND, TargetWindow::own_key_field());
    let own_key_result = own_key.deliver("a key");

    assert_eq!(own_key_result, masked_elsewhere_result);
    assert_eq!(
        own_key_result,
        Err(DeliverError::Refused(Refusal::SecretInput))
    );
    assert!(app.pasted().is_empty());
}

#[test]
fn a_line_break_spread_across_two_segments_is_one_space_in_a_terminal() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(OTHER);
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::terminal());

    // Both arrive while the window is away and land in one paste.
    assert!(session.deliver("one\n").is_err());
    assert!(session.deliver("\ntwo").is_err());
    surface.focus_on(BOUND);
    let _ = session.deliver("three");

    assert_eq!(
        surface.pasted(),
        ["one two three"],
        "the one dictated line break is one space, not two"
    );
}

#[test]
fn every_fragment_of_a_refused_utterance_stays_owed_in_order() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::masked());

    assert!(session.deliver("one").is_err());
    assert!(session.deliver("two").is_err());
    let report = session.finish("three");

    assert!(surface.pasted().is_empty());
    assert_eq!(
        unwritten_words("one two three", &report),
        "one two three",
        "one continuous fragment reaches the clipboard"
    );
}

#[test]
fn a_dictated_new_line_arrives_as_a_space_in_a_terminal() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let surface_reattached = surface.clone();
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::terminal());

    // `ls -la\n` trims to the same words, so this is the tail the provider
    // still had buffered when the user stopped mid-command.
    let report = session.finish("ls -la\n");

    assert_eq!(
        surface_reattached.pasted(),
        ["ls -la"],
        "a pasted newline in a shell can run the command"
    );
    assert_eq!(report.delivered, "ls -la");
}

#[test]
fn a_dictated_new_line_arrives_as_a_new_line_everywhere_else() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut session = session(&ledger, &surface, BOUND);

    let _ = session.deliver("first line\nsecond line");

    assert_eq!(surface.pasted(), ["first line\nsecond line"]);
}

#[test]
fn a_terminal_new_line_arrives_as_a_space_at_stop_too() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::terminal());

    let report = session.finish("make\nall");

    assert_eq!(surface.pasted(), ["make all"]);
    assert_eq!(report.delivered, "make all");
}

#[test]
fn a_terminal_that_is_not_masked_still_receives_text() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::terminal());

    session
        .deliver("git status")
        .expect("a terminal is written to");

    assert_eq!(surface.pasted(), ["git status"]);
}

#[test]
fn an_unknown_target_is_typed_rather_than_refused() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    // What the platform reports when it cannot read a role at all.
    let mut session = session_on(&ledger, &surface, BOUND, TargetWindow::default());

    let _ = session.deliver("line\nbreak");

    assert_eq!(
        surface.pasted(),
        ["line\nbreak"],
        "without a role the app types, and the limit is documented"
    );
}

#[test]
fn the_key_field_names_recognise_quills_own_api_key_inputs() {
    let keys = named_key_fields();
    for name in ["40-character key", "sk-...", "gsk_...", "AIzaSy..."] {
        assert!(
            keys.iter().any(|key| key == name),
            "{name} names a key field"
        );
    }
}

// ── What the platform reports about the target ──────────────────────

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
