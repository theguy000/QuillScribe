//! The target verdict: which fields receive text and which owe it to the clipboard.

use super::fixtures::{session, session_on, FakeSurface, BOUND, OTHER};
use super::*;

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
