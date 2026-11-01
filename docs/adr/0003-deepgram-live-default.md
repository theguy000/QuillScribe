# Deepgram defaults to live now that text is typed while the user speaks

ADR 0002 made `recorded` the default because QuillScribe delivered text only when the user stopped, so a live socket moved work earlier without producing text earlier. That premise is gone. Live typing writes each Final segment into the focused application during the utterance, and it cannot exist without a Live session. Keeping recorded as the default would ship the feature dormant and leave the main experience unchanged.

The default is therefore `live`, live typing is on for a fresh install, and the two settings are linked. Enabling live typing moves the mode to live, and switching the mode to recorded turns live typing off. A settings file that pairs recorded with live typing on loads with it off, so the two cannot contradict each other through the file either. Recorded stays available and unchanged for anyone who would rather not hold a socket open, and batch providers are untouched.

The switch is called "Type as I speak" and sits in Engine, Deepgram, under the mode selector. Whether it is shown, operable, and checked comes from one function, `live_typing_capability` in `src/live_typing.rs`, and the recording path asks the same function before it binds a window. In copy-only output the switch stays on and the clipboard holds the transcript so far after each Final segment instead; display only disables the switch with a reason.

## Consequences

- Every dictation holds a socket open. Streaming allows 150 concurrent connections against prerecorded's 50, so the connection ceiling for a given plan is higher on this default rather than lower, and Deepgram publishes no price difference between the two.
- A recorded utterance and a live utterance now behave differently in a way the user can see. One delivers text during speech, the other at the end, so the engine settings have to say which is which.
- ADR 0002's two-mode structure stands. Only the default changes, so that ADR remains the record of why both modes exist.
- The live path carries costs recorded mode does not: keep-alive frames during silence, reconnect handling, and no session resumption. The live module already implements all three.
- Real latency becomes a user-facing property. Measured on this configuration, a Final segment lands about 0.72 s after a pause at roughly one per sentence, and a sentence held back by formatting can stay open far longer. Those numbers define what "types as you speak" means in practice.
- Reverting to recorded is a settings change rather than a code change, which keeps this decision cheap to revisit.

## Considered Options

- **Keep recorded as the default and let users opt in.** Discarded: live typing cannot run without a Live session, so the feature would ship disabled, and the reason recorded was chosen in ADR 0002 no longer holds.
- **Live only, deleting the recorded path.** Discarded: recorded still suits users who do not want a socket, and ADR 0002's costs for the live path, a socket plus keep-alive plus reconnection, remain real.
- **Leave the two settings unlinked.** Discarded: a user could then enable live typing while the mode is recorded, which is a state that can never work.
