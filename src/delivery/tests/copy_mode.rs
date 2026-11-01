//! Copy mode: the clipboard is the target.
//!
//! There is no window to bind, so the clipboard itself is rewritten with the
//! transcript so far after every fragment.

use super::fixtures::{copy_session, FakeClipboard};
use super::*;

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
