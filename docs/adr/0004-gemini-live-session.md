# Gemini runs as a live session, not a batch upload

Gemini's speech-to-text reached the same reverse-engineered WebChannel the Gemini
web app's dictate button uses, but in the shape QuillScribe had always used for
API providers: record the whole utterance, then upload it. That shape is what
made Gemini feel broken next to the browser doing exactly the same thing.

The numbers, measured through this codebase on one machine, same audio, same
endpoint:

| utterance | batch, after the user stops | live session, after the user stops |
| --------- | --------------------------- | ---------------------------------- |
| 3.4 s     | 2.7 s                       | 0.6 s                              |
| 10.1 s    | 2.4 s                       | 0.4 s                              |
| 60.8 s    | 17.3 s                      | 0.9 s                              |

The batch cost had two terms, both invisible while the upload was fast:

- **Audio the recognizer still has to decode.** Roughly 0.22 s per second of
  speech, because a streaming recognizer consumes its input at its own pace and
  cannot start before the bytes arrive. A single upload of 67 s of audio still
  took 15 s to answer, so this was never a bandwidth problem.
- **A penalty per upload message.** Splitting the same audio into 24 KB messages,
  which is what the batch path did, cost about 0.7 s per message on top. For a
  67 s utterance that alone was 13 s.

The app's own history file shows the same curve from the other side: 5 s
dictations answered in 2.4-3 s, 30 s dictations in 8.7 s, 85 s dictations in
22 s. The browser felt instant because it had already sent the audio before the
user stopped.

Gemini is therefore driven by a live session, opened when recording starts, and
`is_live_provider` returns true for it. Audio leaves as one Opus-in-WebM cluster
per second of speech (`src/whisper/gemini/live.rs`), so the recognizer's work
overlaps the utterance and only the flush is left at the end. The transcript is
unchanged: one text, delivered when the user stops.

## Considered Options

- **Bigger batch messages.** Keeps the batch shape and removes the per-message
  penalty; measured at 15 s for a 67 s utterance. It leaves the decode term, so
  long dictations stay unusable. Rejected as a fix, though the live path inherits
  the finding: it never sends anything as small as before either.
- **Pre-open the session at record start, keep uploading at stop.** Saves the
  handshake, the recognizer init and the per-message penalty (~2 s) and keeps the
  decode term. Strictly worse than streaming for about the same amount of code.
- **A WebSocket downstream.** The web client may use one, and
  `tokio-tungstenite` is already in the tree for Deepgram. The long-poll works and
  is already proven against the endpoint, so the extra transport would be risk
  without a measured win.
- **Leaving Gemini batch and telling the user to wait.** Keeps the code still and
  keeps the latency the report was about.

## Consequences

- The WebChannel downstream is a long-poll that the server ends after about a
  minute even mid-utterance. The session re-attaches with the next `AID`, and the
  recognizer keeps its state across that: a 68 s utterance re-attached once and
  returned the full transcript. A closing message seen before the end marker is
  not a closing message, so the reader only treats one as final after `GAE=`.
- `WebmOpusStream` writes a container whose length is not known when it starts:
  unknown-size Segment, no Duration, one cluster per second. It reuses the batch
  writer's element builders, so the container is still described in one place.
- A live session can fail where a batch upload could not: a dropped connection
  mid-utterance. The stop path therefore falls back to the batch request on the
  recorder's own buffer, so a failure costs latency, never words.
- The live session has no Final segments, so it does not touch live typing.
  `supports_live_typing` stays Deepgram-only and the switch never appears for
  Gemini. The panel's in-progress line does fill in while the user speaks.
- `Progress` and `ProgressSink` moved from `src/deepgram.rs` to
  `src/live_provider.rs`, beside the `LiveSession` enum the stop path flushes,
  because two providers now report through them.
- Measured on this configuration: a 60 s dictation answers about 0.9 s after the
  user stops, and that number no longer grows with the utterance.
