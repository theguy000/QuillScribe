# Deepgram is a live-streaming-only provider

**Status: superseded by [ADR-0002](./0002-deepgram-recorded-default.md).** Deepgram now supports a recorded mode as well, and that is the default; the reasoning below explains why the live path exists, not why it is the only one.

Deepgram offers both prerecorded (`POST /v1/listen`) and live (`wss://api.deepgram.com/v1/listen`) speech-to-text. QuillScribe implements **only the live path** for it. The reason is that QuillScribe's known latency problem is structural: every existing provider records the whole utterance first and only starts work after the user stops, so a 10-second clip costs seconds *after* release. A live session accrues the transcript while the user is still speaking and leaves only one flush round-trip after they stop. A prerecorded Deepgram would duplicate what Gemini already does and inherit the same latency shape, so it buys nothing.

## Considered Options

- **Prerecorded only, reusing `transcribe_audio(Vec<f32>, u32) -> Result<String>`.** Cheapest change, no new dependency, no new execution shape — but then Deepgram offers nothing the app cannot already do.
- **Both, behind a mode toggle.** Doubles the request-building, error handling and settings surface for a path nobody would deliberately choose.
- **The official `deepgram` crate (0.14.0).** Rejected: it self-describes as a "Community Rust SDK" rather than an officially supported one, and it shipped two minor releases in 13 days. It wraps `tokio-tungstenite` with the same surface we need to control directly (raw binary frames, `KeepAlive` on a timer, `Finalize` → `CloseStream` → wait for `Metadata`).

## Consequences

- The audio capture callback gains a sample tap so chunks can leave the recorder while recording is still in progress; batch providers keep consuming only the whole buffer at stop.
- The app now has two execution shapes — batch and live session — not one.
- `tokio-tungstenite` and its rustls tree enter the dependency graph.
- Deepgram has no session resumption: a dropped socket starts a new session with timestamps reset, so the session task reconnects and replays the un-sent tail from the local buffer instead of losing the utterance.
- Diarization is deliberately not implemented; it is a query-parameter-level addition later.
