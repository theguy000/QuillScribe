# Deepgram supports both recorded and live, and recorded is the default

ADR 0001 made Deepgram live-streaming-only. That was reversed. Deepgram now has two modes, chosen in Settings → Engine → Deepgram mode, and **`recorded`** — upload the finished utterance to `POST /v1/listen` — is the default. `live` streams over `wss://api.deepgram.com/v1/listen` and is opt-in.

The reason ADR 0001 is wrong in practice: QuillScribe delivers text only when the user stops recording, because the output pipeline (clipboard / typing into the focused window) runs once, on a final transcript. Streaming audio live therefore produces no earlier text; it only moves work earlier. It pays off once text is delivered *while the user speaks*, which does not exist yet. Until then the recorded path is simpler, cheaper (prerecorded has a 50-concurrent limit against streaming's 150, but needs no socket, no keep-alive, no reconnect), and gives the same result at the same moment.

## Consequences

- One provider now has two execution shapes. `start_live_session_if_needed` is gated on the mode, and the recorded path goes through the ordinary batch call like OpenAI and Groq.
- The live path stays built and tested rather than deleted, so "type as you speak" has something to stand on.
- Which flags matter depends on the mode: `interim_results`, `utterance_end_ms`, `endpointing` and `vad_events` are live-only and are absent from the recorded URL; `smart_format`, `punctuate` and `dictation` apply to both.

## Considered Options

- **Keep live-only (ADR 0001).** Discarded: it forces a socket on users who gain nothing from it.
- **Recorded-only, delete the live path.** Discarded: it throws away working, tested streaming that the planned live-typing feature needs.
