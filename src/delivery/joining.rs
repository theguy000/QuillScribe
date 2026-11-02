//! How fragments become the text a target receives.
//!
//! The rules live here so the session only decides which fragments go together
//! and in what order. Everything in this module is pure.

/// One fragment as it reaches the document.
///
/// `previous` is the ledger's answer for this window: `None` when the app has
/// never typed into it, `Some(false)` when the last character it typed was not
/// whitespace. The caller has already trimmed `text` and rejected an empty one,
/// so the result never carries whitespace at either end.
pub(super) fn fragment(previous: Option<bool>, text: &str) -> String {
    let mut fragment = String::with_capacity(text.len() + 1);
    if previous == Some(false) {
        fragment.push(' ');
    }
    fragment.push_str(text);
    fragment
}

/// Several owed fragments as the one text the target receives.
///
/// Fragments are joined with a space while they are inside the utterance.
/// A dictated new line that survived the verdict, because the target is not a
/// terminal, is the one thing that joins without a space: the words said around
/// it were spoken on either side of the same mark, and the space would put a
/// visible gap before the line break.
///
/// `previous` is the ledger's answer for the first fragment, and `terminal` says
/// whether the target is a terminal, where a new line becomes a space.
pub(super) fn join(previous: Option<bool>, fragments: &[String], terminal: bool) -> String {
    let Some(first) = fragments.first() else {
        return String::new();
    };
    let mut joined = fragment(previous, first);
    for text in &fragments[1..] {
        // Nothing is inserted after a mark: the words arrived in a separate
        // fragment, but the sentence did not.
        if !joined.ends_with(['\n', '\r']) {
            joined.push(' ');
        }
        joined.push_str(text);
    }
    if terminal {
        // A newline that ends one fragment and a newline that starts the next
        // were the same dictated line break; a space between them would put two
        // spaces where one belongs.
        joined = joined.replace("\n\n", "\n");
    }
    joined
}
