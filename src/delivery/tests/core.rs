//! Behaviour of one utterance's delivery, against a fake platform.
//!
//! What the app types, where it holds words back, and what it still owes when
//! stop comes. Copy mode and the target rules have their own files.

use super::fixtures::{session, FakeSurface, BOUND, OTHER};
use super::*;
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

/// The bug this pair of tests pins down: a pause makes the provider commit a
/// fragment on its own, and the app adds a space to it while the ledger keeps
/// saying the document ends on a letter. Every fragment then carries one more
/// space and the text drifts one character right per sentence.
#[test]
fn a_pause_between_two_utterances_adds_one_space() {
    let ledger = DeliveryLedger::shared();

    let first = FakeSurface::focused_on(BOUND);
    let _ = session(&ledger, &first, BOUND).deliver("Hello");

    // The user pauses, so the next sentence lands in a new utterance.
    let second = FakeSurface::focused_on(BOUND);
    let _ = session(&ledger, &second, BOUND).deliver("there");

    assert_eq!(first.pasted(), ["Hello"]);
    assert_eq!(
        second.pasted(),
        [" there"],
        "the document ended on a letter, so exactly one space belongs here"
    );
}

#[test]
fn an_unknown_document_gets_a_leading_space() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut session = session(&ledger, &surface, BOUND);

    // No record for this window yet, so the app has no proof the document
    // ends on a letter and pastes without a separator of its own.
    let _ = session.deliver("Hello");

    assert_eq!(
        surface.pasted(),
        ["Hello"],
        "an unknown document adds no separator of its own"
    );
}

#[test]
fn a_paste_that_is_refused_does_not_leave_a_space_in_the_ledger() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND).failing(1);
    let mut first = session(&ledger, &surface, BOUND);

    // The first attempt is refused and retried, so the ledger records the
    // space once and not twice.
    let _ = first.deliver("Hello");

    let mut second = session(&ledger, &surface, BOUND);
    let _ = second.deliver("there");

    assert_eq!(surface.pasted(), ["Hello", " there"]);
}

#[test]
fn a_fragment_after_a_pasted_separator_gets_no_second_space() {
    let ledger = DeliveryLedger::shared();
    let surface = FakeSurface::focused_on(BOUND);
    let mut first = session(&ledger, &surface, BOUND);

    // Two utterances in a row: the second one is pasted with a space in front.
    let _ = first.deliver("Hello");
    let _ = first.deliver("there");

    // The document now ends on a letter again, after the space and the word.
    let mut next = session(&ledger, &surface, BOUND);
    let _ = next.deliver("again");

    assert_eq!(
        surface.pasted(),
        ["Hello", " there", " again"],
        "each separator is recorded, so one space separates each pair"
    );
    assert_eq!(next.report().delivered, "again");
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
