# Deepgram Speech-to-Text — Source-Cited Fact Sheet

Researched 2026-11-01 against current Deepgram docs (developers.deepgram.com), the live pricing page,
crates.io, and the deepgram-rust-sdk repo. Every claim below traces to a URL in **Sources**.
Anything I could not verify from a source is explicitly marked **UNCERTAIN**.

> **Reading warning — the feature pages lie about availability.**
> Doc feature pages carry chip rows like `Pre-recorded · Streaming:Nova · Streaming: Flux`, and those
> chips overstate. Example: the *Interim Results* page carries a "Pre-recorded" chip, but
> `interim_results` does not exist on the prerecorded reference; the *Paragraphs* page carries
> "Streaming:Nova" chips, but `paragraphs` is absent from the streaming reference and from the
> streaming SDK matrix. **Treat the two API reference pages (and the AsyncAPI schema they embed) as
> authoritative, not the chips.** [S1][S2][S12][S15][S29]

---

## A. The two modes

### A1. Endpoints, transports, hostnames

| Mode | Method / transport | URL | Notes |
|---|---|---|---|
| **Prerecorded (batch)** | HTTPS `POST` | `https://api.deepgram.com/v1/listen` | Body is either `application/json` `{"url": "..."}` **or** raw audio bytes with an `audio/*` Content-Type | [S2] |
| **Live streaming** | `wss://` WebSocket | `wss://api.deepgram.com/v1/listen` | AsyncAPI declares server `wss://api.deepgram.com/`, channel `/v1/listen` | [S1] |
| **Flux (turn-based streaming)** — third mode, added 2025 | `wss://` WebSocket | `wss://api.deepgram.com/v2/listen` | Different model family and message schema (`TurnInfo`, `EndOfTurn`); not the classic `/v1/listen` protocol | [S3][S15] |

Regional hostnames (same paths, same API keys; **Whisper models are not available in any region**):

| Region | REST | Streaming |
|---|---|---|
| North America (default) | `api.deepgram.com` | `wss://api.deepgram.com/v1/listen` |
| Europe | `api.eu.deepgram.com` | `wss://api.eu.deepgram.com/v1/listen` |
| Australia | `api.au.deepgram.com` | `wss://api.au.deepgram.com/v1/listen` |
| India (GA 2026-09-15) | `api.in.deepgram.com` | `wss://api.in.deepgram.com/v1/listen` |

EU/AU/IN endpoints never fall back to another region: if the region is down, requests fail. [S3][S32]

Prerecorded also supports an async/callback form: `?callback=<url>&callback_method=POST|PUT`, which
returns `{"request_id": "..."}` instead of a transcript body. [S2]

### A2. How audio reaches the server

**Raw binary WebSocket frames.** The client→server audio message is declared in the AsyncAPI spec as
`ListenV1Media: { type: string, format: binary }` — i.e. a plain binary frame. There is **no**
JSON envelope and **no** base64 wrapping for audio. JSON text frames are used *only* for control
messages (`KeepAlive`, `Finalize`, `CloseStream`, `Configure`). The lower-level implementation guide
confirms the split: `SendBinary([]bytes)` for audio, `SendMessage([]byte)` for JSON control. [S1][S6]

**Accepted `encoding` values** (streaming `encoding` enum, exact):

```
linear16, linear32, flac, alaw, mulaw, amr-nb, amr-wb, opus, ogg-opus, speex, g729
```

- `linear16` = 16-bit little-endian signed PCM; `linear32` = 32-bit LE float PCM.
- **`mp3` is not in the streaming enum.** MP3 works as *containerized* audio: you omit `encoding`
  (and `sample_rate`) entirely and stream the container bytes, letting Deepgram read the header.
  Deepgram advertises "over 100" formats including MP3/MP4/WAV/FLAC/Ogg/Opus/WebM. [S21][S23]
- **Raw audio requires BOTH `encoding` and `sample_rate`**; containerized audio requires NEITHER.
  Getting this wrong is a documented cause of immediate disconnects. [S21][S22][S24][S6]

**Sample rates:** arbitrary integer `sample_rate` for PCM (common: 8000, 16000, 44100). Two hard
constraints: `encoding=amr-nb` **must** use `sample_rate=8000`; `encoding=amr-wb` **must** use
`sample_rate=16000`. [S22]

**Sending the whole file at once instead of in realtime:**

- **You cannot dump a file instantly.** Docs state "Deepgram allows audio to be streamed at a
  **maximum of 1.25x realtime**", so a 60 s clip needs ≥ ~48 s of wall-clock time to push. [S25]
- **The first audio frame must arrive within 10 s of the socket opening**, or the connection closes. [S25]
- **It still transcribes.** Nothing in the protocol requires realtime pacing; `CloseStream` is
  explicitly documented to "finish processing any cached data, send the response to the client, send
  a summary metadata object, and then terminate" — so buffered audio is not lost. [S9]
- **Interim results are not disabled, but they become useless.** Interims are cumulative partials
  ("another big" → "another big problem") emitted as audio arrives; under a burst upload you get a
  rapid cascade of superseded partials. Only the `is_final: true` messages carry committed text. [S16]
- **UNCERTAIN:** the exact interim cadence under a burst upload is not documented. Docs only say
  interim results are "typically sent every second" during normal streaming. [S12]

### A3. WebSocket authentication

Three documented methods, no fourth:

1. **`Authorization: Token <API_KEY>` header** — the standard method, on the WS handshake. [S1][S4]
2. **`Sec-WebSocket-Protocol: token, <API_KEY>`** — the subprotocol trick. Docs are explicit:
   *"The use of this header is only required when making 'client side' connections to Deepgram, where
   custom `Authorization` headers are prohibited by security measures in apps, including web apps,
   mobile apps and certain desktop apps."* The value is literally two subprotocol entries: the word
   `token`, then the key. [S5]
3. **Short-lived JWT** — `Authorization: Bearer <JWT>` from the Token-Based Auth API (30-second TTL),
   for handing a credential to an untrusted client. [S2][S4]

**There is no documented `?token=` query parameter.** Current docs describe only the header and the
subprotocol; a query-string key is not mentioned anywhere I fetched. **UNCERTAIN** whether an
undocumented `?token=` still works — do not build on it.

**Verdict for a Rust desktop client:** use the **`Authorization: Token <KEY>` header**. A native Rust
app is not a browser, so it can set arbitrary handshake headers, and the subprotocol workaround is
documented only as a fallback for header-hostile environments. (If you ever port to a WebView/browser
context, switch to the subprotocol form, because `Sec-WebSocket-Protocol` is settable from JS while
`Authorization` is not.) [S5]

### A4. Session lifecycle messages

All three are **text** frames containing a minimal JSON object. [S1][S7][S8][S9]

**KeepAlive** — send every 3–5 s during silence:

```json
{ "type": "KeepAlive" }
```

- **The server sends no response.** [S7]
- **Timeout:** "If no audio data or `KeepAlive` messages are sent within a **10-second** window, the
  connection will close with a **`NET-0001`** error." So **yes, KeepAlive prevents the timeout.** [S7]
- Must be a **text** frame — "sending it as binary may result in incorrect handling". [S7]
- **Word timings are based on audio, not connection age:** pause 10 s → 30 s with KeepAlives, and the
  next result's word times resume at 10 s, not 30 s. [S7]

**Finalize** — flush buffered audio without closing:

```json
{ "type": "Finalize" }
```

Optionally target one channel (0-based): `{ "type": "Finalize", "channel": 0 }`. Omit `channel` to
finalize all. The server processes remaining audio and returns a Results message which **may** carry
`"from_finalize": true` — "In most cases you will receive this response, but it is not guaranteed if
there is no significant amount of audio data to process." Use the response's `channel_index` to know
which channel was finalized. [S8]

**CloseStream** — flush, report, and terminate:

```json
{ "type": "CloseStream" }
```

Server behaviour, in order: process remaining audio → send the final transcription response(s) →
send one **`Metadata`** message → terminate the socket. [S9]

**What the server sends back when the stream closes** (real example from the CloseStream docs; note
`duration: 0.0` / `channels: 0` because that example sent no audio):

```json
{
  "type": "Metadata",
  "transaction_key": "deprecated",
  "request_id": "8c8ebea9-dbec-45fa-a035-e4632cb05b5f",
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "created": "2024-08-29T22:37:55.202Z",
  "duration": 0.0,
  "channels": 0
}
```

**New in 2026 (bonus): `Configure`** — mid-stream `{"type":"Configure","keyterms":[...],"features":{"numerals":true}}`.
Nova-3 only for keyterms (500-token limit); other models return `ConfigureFailure` with
`CONFIGURE_KEYTERMS_NOT_SUPPORTED`. Replies `ConfigureSuccess` or `ConfigureFailure`. [S1][S32]

---

## B. Feature flags, exact names and semantics

`ER` = **E**xists on prerecorded **R**eference · `ES` = **E**xists on **S**treaming reference.
Defaults are from the AsyncAPI/OpenAPI schemas. [S1][S2]

### B1. `punctuate`

| | |
|---|---|
| Supported | **Both** (ER, ES) |
| Name / type | `punctuate` — boolean, `true`/`false` |
| Default | `false` |
| Semantics | "Add punctuation and capitalization to the transcript." Implied by `smart_format=true`, `paragraphs=true`, and `detect_entities=true`. |

### B2. `dictation` — the user's suspicion, resolved

**`dictation` is a real Deepgram parameter, but the guess about what it does is wrong in an important way.**

| | |
|---|---|
| Supported | **Both** (ER, ES). Feature page chips: Pre-recorded, Streaming:Nova, Streaming:Flux. **English only.** [S12] |
| Name / type | `dictation` — boolean |
| Default | `false` |
| What it actually does | Converts **spoken dictation commands into punctuation marks**. Say "comma" → get `,`. Say "period" → get `.`. Command list: `period`→`.`, `comma`→`,`, `colon`→`:`, `question mark`→`?`, `exclamation mark`→`!`, `new line`→`\n`, `new paragraph`→`\n\n`. [S12] |
| Dependency | **`punctuate=true` is required**, otherwise dictation does not work. Docs: "Be sure to add `dictation=true&punctuate=true`". [S12] |
| Models | English only, all regions. Chips claim prerecorded + Nova streaming + Flux streaming. The SDK feature matrix lists `dictation` as GA for **prerecorded** only. **UNCERTAIN** for streaming — the streaming AsyncAPI does accept the param, so it is plumbed through, but no streaming example is published. [S12][S20] |
| Deprecated? | **No.** Not deprecated, and **not** superseded by `smart_format`. The Smart Formatting page explicitly lists Dictation under "Additional Formatters — these formatters are **not** included in Smart Formatting but may be enabled individually." [S13] |

So: if the user wants "8:37 PM" / "$1,234" style formatting, that is **`smart_format`**, not `dictation`.
`dictation` is for people literally dictating punctuation aloud.

> **Doc bug to be aware of:** the streaming AsyncAPI gives `dictation` the description *"Identify and
> extract key entities from content in submitted audio"* — a copy-paste of the `detect_entities` text.
> That description is wrong; the Dictation feature page is the correct reference. [S1][S12]

### B3. `smart_format`

| | |
|---|---|
| Supported | **Both** (ER, ES) |
| Name / type | `smart_format` — boolean |
| Default | `false` |
| What it does | "Applies additional formatting to transcripts to optimize them for human readability." **At minimum: punctuation + paragraphs** (for whitespace-delimited languages). For **English**: dates, times, currency amounts, phone numbers, emails, URLs. For non-English: all formatters available for that language, always punctuation + paragraphs, plus `numerals` for select languages. [S13] |
| Model/language restrictions | "Smart Format capabilities vary between models… Deepgram will always apply the best-available formatting for your chosen combination of model, model option and language." Broadest support is English-language models. No hard model block is documented. |
| Interaction | Enables Punctuation implicitly — "If you've set `smart_format=true`, no need to also set `punctuate=true`." |
| Streaming caveat | Entities that look incomplete are held until the speaker continues **or 3 seconds of silence** elapse. `Finalize` ends it sooner. `no_delay=true` forces immediate finalization but **skips formatting in many cases**. [S13] |
| Price | **Included at no cost** (whereas diarization/redaction/keyterm are billed add-ons). [S19] |
| Multichannel | For prerecorded `multichannel=true`, English formatting may use cross-channel context; streaming uses only the current channel. [S13] |

### B4. `interim_results`

| | |
|---|---|
| Supported | **Streaming only** (ES). Absent from the prerecorded reference. |
| Name / type | `interim_results` — boolean |
| Default | `false` |
| Semantics | Server sends ongoing partial transcripts that are revised as more audio arrives. Results are **cumulative from the segment `start`**, not incremental deltas. Interim messages are typically emitted about **once per second**. [S16][S12] |
| Shape of a partial | Same `Results` envelope with `"is_final": false`; `start + duration` equals the `end` of the last (often mid-word) word. `confidence` rises as the word completes. Full example in D1. |

### B5. `endpointing`

| | |
|---|---|
| Supported | **Streaming** (ES). Not present in the prerecorded query list. |
| Name / type | `endpointing` — integer (schema types it "Any type"; docs: "an integer representing a millisecond value"); `false` also accepted |
| Units | **milliseconds** |
| Default | **`10`** (AsyncAPI default `'10'`; docs: "enabled by default and set to 10 milliseconds") |
| Minimum | **Not documented as an explicit floor.** `10` is the default; `endpointing=false` disables endpointing entirely (results then arrive at the cadence of Deepgram's chunking). Docs' own examples use `300`; the code-switching guide recommends `100`. **UNCERTAIN** whether values <10 are rejected. [S10][S27] |
| Mechanism | A VAD watches the stream; when a sufficiently long pause is detected the transcript for that range is finalized. |
| `speech_final` meaning | Set to `true` on the Results message that was emitted **because an endpoint was detected** — i.e. "the speaker paused long enough that more audio won't improve this." [S10] |

> Caveat: the endpointing doc's own example shows `"is_final": false` **together with**
> `"speech_final": true`. Do not assume `speech_final` implies `is_final` — use `is_final` as the
> commit/append signal and treat `speech_final` as "turn boundary". [S10]

### B6. `utterance_end_ms`

| | |
|---|---|
| Supported | **Streaming** (ES). |
| Name / type | `utterance_end_ms` — string/integer, milliseconds |
| Minimum | **`1000`** (also the default). **Maximum `5000`.** Any integer step within range. (Self-hosted/Dedicated with a modified step size use that step as the minimum instead.) |
| Requirement | **`interim_results=true` is required** — "Utterance end relies on Deepgram's `interim_results` feature". Values below 1000 gain nothing because interims are ~1/s. |
| What it emits | A dedicated message: `{ "type": "UtteranceEnd", "channel": [0, 1], "last_word_end": 2.395 }` |
| Semantics | Analyzes interims/finals for a gap of the configured length **after the last finalized word**, then fires. It does **not** detect gaps contained inside a single final result. |
| Trap | **`last_word_end: -1`** means the result was already finalized before the condition was met — **ignore that UtteranceEnd** to avoid duplicate handling. |
| Voice-agent warning | UtteranceEnd fires on the gap even if speech then continues, so it is a poor "turn is over" signal for conversational agents. [S11] |

### B7. `diarize`

| | |
|---|---|
| Supported | **Both.** Feature chips: Pre-recorded, Streaming:Nova, Streaming:Flux. Rate-limit table has an explicit "Speaker Diarization" row for both Pre-Recorded and Streaming. [S14][S17] |
| Name / type | `diarize` — boolean, **`deprecated`** in both references. Default `false`. |
| Replacement | **`diarize_model`** — enum. Specifying it both enables diarization and picks the version; you must **not** set both `diarize` and `diarize_model` (requests setting both are rejected). [S14] |
| Version matrix | Batch: `latest` (= v2), `v1`, `v2`. Streaming: `latest` (= **v1**), `v1`; **`v2` returns a validation error on streaming**. Legacy `diarize=true` always routes to v1. [S14][S2] |
| Response | Each **word** gets an integer `speaker`, **0-based**. Prerecorded also returns `speaker_confidence` (float). **Streaming returns `speaker` only — no `speaker_confidence`.** Note the *paths differ*: prerecorded `results.channels[0].alternatives[0].words[].speaker`; streaming `channel.alternatives[0].words[].speaker` (no `results` wrapper, no `channels` array — see G1). [S14][S2][S1] |
| Metadata | `metadata.diarize_info = { model_uuid, arch: "v1"|"v2" }` — present only when a diarizer actually ran; never `null`/`{}`; absent means no diarizer ran. [S14] |
| Model support | Compatible with **all Nova batch models (Nova-1, Nova-2, Nova-3), plus `enhanced` and `base`**. **Whisper is not supported.** [S14] |
| Language support | Feature page claims "All available languages". |
| Multichannel | **Not incompatible — they compose.** With both on, each channel is diarized independently, so the two people on two channels are *both* `speaker: 0` on their own channel. That is correct behaviour, not a bug. [S28] |
| Cost / limits | **$0.0020/min** add-on; diarization requests are capped lower: 50 concurrent streaming + 50 prerecorded in NA (25/25 in EU/AU/IN) on PAYG. [S19][S17] |

### B8. The rest of the flag matrix

| Parameter | Prerecorded | Streaming | Type / default | Semantics & gotchas |
|---|---|---|---|---|
| `vad_events` | yes | **yes** (`false`) | boolean, `false` | Emits `{"type":"SpeechStarted","channel":[0,1],"timestamp":9.54}` when speech starts. Marked **`Beta`** in the SDK matrix. Its timestamp may not match the first word's start time. [S30][S20] |
| `filler_words` | **yes** | **no** | boolean, `false` | Transcribes "uh"/"um" and interruptions. Absent from the streaming reference and from the streaming SDK matrix. Nova-2 is the recommended model for filler-word identification. [S2][S20][S15] |
| `profanity_filter` | yes | yes | boolean, `false` | Replaces flagged words with the nearest non-profane word, or removes them. |
| `numerals` | yes | yes | boolean, `false` | Converts written-form numbers to digits. Included in `smart_format` for select languages. |
| `encoding` | yes | yes | enum | Raw/headerless only. Streaming enum: `linear16, linear32, flac, alaw, mulaw, amr-nb, amr-wb, opus, ogg-opus, speex, g729`. Prerecorded enum is narrower: `linear16, flac, mulaw, amr-nb, amr-wb, opus, speex, g729` (no `linear32`/`alaw`/`ogg-opus`) — for prerecorded you normally just set the Content-Type instead. **Requires `sample_rate`.** [S1][S2][S21] |
| `sample_rate` | yes | yes | int32 | Required with `encoding`. Omit both for containerized audio. amr-nb→8000, amr-wb→16000. [S22] |
| `channels` | **no** (not in ref) | yes | integer, default `'1'` | Number of independent channels in the stream. Pairs with `multichannel`. |
| `multichannel` | yes | yes | boolean, `false` | Transcribe each channel independently; response gains `results.channels[N]`. Bills by total processed duration across channels (10 min stereo = 20 min billed). [S2][S19] |
| `model` | yes — **default `base-general`** | yes — **no default declared** | enum/string | **Always pass `model` explicitly.** The streaming reference declares no default; prerecorded defaults to the legacy `base-general`. [S1][S2] |
| `language` | yes — default `en` | yes — default `en` | BCP-47 string | `multi` enables code-switching (see C). |
| `version` | yes — default `latest` | yes — default `latest` | enum/string | Model version pin. `latest` in practice. |
| `keywords` | yes | yes | list of string | Boosting **with** weights/intensifiers (`term:0.15` semantics). **Not supported with Nova-3** — use `keyterm`. [S2] |
| `keyterm` | yes | yes | list of string | The **replacement for `keywords` on Nova-3**; Nova-3 only. Plain terms only — a `term:0.15` weight is **silently ignored** and the whole string treated as a literal term. Repeat the param for multiple terms; join multi-word phrases with `%20`/`+`; never comma-separate. 500-token limit. **$0.0013/min.** [S2][S19] |
| `redact` | yes | yes | **different enums** | Prerecorded: `pci`, `pii`, `numbers`. Streaming: `true`, `pci`, `numbers`, `aggressive_numbers`, `ssn`. **$0.0020/min.** [S1][S2][S19] |
| `paragraphs` | **yes** | **no** (see warning) | boolean, `false` | Splits transcript into paragraphs; implicitly enables punctuation; paragraph breaks are influenced by speaker changes (diarization) and channel changes (multichannel). Emits `alternatives[0].paragraphs.{transcript, paragraphs[]}`. Absent from the streaming reference and streaming SDK matrix, though the feature page chips claim streaming. **UNCERTAIN for streaming — don't rely on it there;** `smart_format` already yields paragraphs. [S29][S1][S20] |
| `summarize` | **yes** | **no** | enum `v2` or boolean | Audio Intelligence summarization; adds `results.summary.{result,short}` and `metadata.summary_info`. Billed per token. [S2][S19] |
| `detect_language` | **yes** | **no** | boolean or list | Identifies the dominant language; surfaces as `channels[].detected_language`. [S2] |
| `utterances` | **yes** | **no** | boolean, `false` | Segments speech into semantic units → `results.utterances[]` with `transcript`, `words`, `speaker`, `id`, `start`, `end`. |
| `utt_split` | yes | no | double, **`0.8`** | Seconds to wait before detecting a pause between words (tunes `utterances`). |
| `words` | always present | always present | — | Not a flag; word arrays are always returned in `alternatives[].words[]`. |
| `search` | yes | yes | list of string | Returns `channels[].search[]` with `{query, hits[{confidence,start,end,snippet}]}`. |
| `detect_entities` | yes | yes | boolean, `false` | NER; entities appear in **final** results only as `alternatives[].entities[]`. Implicitly enables punctuation. Streaming: added 2026, `$0.0017/min`. [S1][S2][S19] |
| `extra`, `tag` | yes | yes | key/value, string | Tagging/usage attribution. `tag` labels usage reports; `extra` echoes into the response/metadata. |
| `callback`, `callback_method` | yes | yes (declared) | url, `POST` | Async delivery. **`dg-token` header verification was removed 2026-10-27** — use Basic Auth in the callback URL or a secret in `extra`. [S32] |
| `mip_opt_out` | yes | yes | boolean, `false` | Opt out of the Model Improvement Program. Since 2026-03-05 this has **no pricing impact**. [S32] |
| `language_hint` | yes | yes | string / repeatable | Nova-3 multilingual only (`model=nova-3&language=multi`); ignored otherwise; invalid code → HTTP 400. [S1][S2][S27] |
| `no_delay` | — | yes | boolean | Streaming smart-format escape hatch: forces immediate finalization, sacrificing formatting. [S13] |

---

## C. Models

### C1. Current model names

**Flux** (streaming, `/v2/listen`, conversational/turn-based, Nova-3-class accuracy): `flux-general-en`,
`flux-general-multi`. [S15]

**Nova-3** (current flagship, batch + streaming, no turn detection): `nova-3` / `nova-3-general`,
`nova-3-medical`, `nova-3-pharma`, `nova-3-<CUSTOM>`. [S15]

**Nova-2**: `nova-2` / `nova-2-general`, `nova-2-meeting`, `nova-2-phonecall`, `nova-2-finance`,
`nova-2-conversationalai`, `nova-2-voicemail`, `nova-2-video`, `nova-2-medical`, `nova-2-drivethru`,
`nova-2-automotive`, `nova-2-atc`, `nova-2-<CUSTOM>`. [S15]

**Legacy Nova-1**: `nova` / `nova-general`, `nova-phonecall`, `nova-medical`, `nova-<CUSTOM>`. [S15]

**Enhanced**: `enhanced` / `enhanced-general`, `enhanced-meeting`, `enhanced-phonecall`,
`enhanced-finance`, `enhanced-<CUSTOM>`. **Base**: `base` / `base-general`, `base-meeting`,
`base-phonecall`, `base-finance`, `base-conversationalai`, `base-voicemail`, `base-video`,
`base-<CUSTOM>`. [S15]

**Whisper Cloud** (prerecorded only, North America only, poor scalability): `whisper`,
`whisper-tiny`, `whisper-base`, `whisper-small`, `whisper-medium`, `whisper-large`. [S15][S3]

> **Inconsistency to note:** the streaming API reference's `model` enum is missing `nova-3-pharma`,
> `nova-2-phonecall`, `nova-2-atc`, and all `flux-*` names, and includes bare legacy aliases
> (`meeting`, `phonecall`, `finance`, `video`, `custom`). The Models & Languages page is the
> authoritative list of names; the enum lags. [S1][S15]

### C2. Recommended default in 2026

- **`nova-3`** — "our highest-performing general-purpose ASR (no turn detection). Recommended for
  meetings, event captioning, multi-speaker, multilingual, noisy, or far-field audio **in batch or
  streaming**." This is the default choice for a dictation/transcription app. [S15]
- **`flux-general-en` / `flux-general-multi`** — "recommended for real-time agents, customer support
  bots, and interactive, turn-based experiences", because it has model-native end-of-turn detection.
  Only worth it if you want server-side turn detection. [S15]
- **`nova-2`** — "recommended for use cases with languages not yet supported by nova-3, and filler
  word identification." [S15]
- **`enhanced`** — better WER than base, and the family that still supports classic `keywords` boosting. [S15]

### C3. Feature support by model

| Feature | Support |
|---|---|
| `dictation` | English only; prerecorded + Nova streaming + Flux streaming (per chips); SDK matrix lists it GA for **prerecorded**. [S12][S20] |
| `diarize` / `diarize_model` | All Nova (1/2/3 batch + streaming) + `enhanced` + `base`. **Not Whisper.** v2 batch only; streaming pins to v1. [S14] |
| `smart_format` | All models; capability varies — richest for English-language models; on non-English at minimum punctuation + paragraphs (+ numerals for select languages). [S13] |
| `keyterm` | **Nova-3 only** (incl. nova-3-medical/pharma; monolingual + multilingual). [S2][S1] |
| `keywords` | Everything **except** Nova-3. [S2] |

### C4. Language support story

- **Default is `language=en` on every model** unless you pass `language`. [S15]
- **Nova-3 multilingual** — `model=nova-3&language=multi`, **55 languages**, and it is true
  code-switching: per-word `language` tags and a `languages[]` array per alternative, sorted by word
  count. `language_hint` (repeatable) biases detection without restricting it. Docs recommend
  **`endpointing=100`** for code-switching. [S27][S15]
- **Nova-3 monolingual** — ~70 language codes incl. regional variants (`en-US`, `fr-CA`, `pt-BR`,
  `zh-HK`, …). [S15]
- **Nova-2 `multi`** — **Spanish + English only** (narrower than Nova-3). [S15][S27]
- **Flux multilingual** (`flux-general-multi`) — 10 languages: en, es, fr, de, hi, ru, pt, ja, it, nl;
  returns detected languages on every `TurnInfo` event, no `language=multi` needed. [S15][S27]
- `nova-3-medical` also accepts `multi` (same 10-language set as Flux). [S15]
- `language_hint` is **Nova-3-multilingual only**; Nova-2 rejects it, and a code outside the supported
  list fails the request with **HTTP 400 `INVALID_QUERY_PARAMETER`**. [S27][S1]

---

## D. Response shapes

### D1. Streaming messages

**(a) Interim partial — `is_final: false`** (real example, from the Interim Results docs): note the
last word is cut off (`end` == `start + duration`), which is why the model expects to revise it. [S16]

```json
{
  "channel_index": [0, 1],
  "duration": 1.039875,
  "start": 0,
  "is_final": false,
  "channel": {
    "alternatives": [
      {
        "transcript": "another big",
        "confidence": 0.9600255,
        "words": [
          { "word": "another", "start": 0.2971154, "end": 0.7971154, "confidence": 0.9588303 },
          { "word": "big",     "start": 0.85173076, "end": 1.039875, "confidence": 0.9600255 }
        ]
      }
    ]
  }
}
```

**(b) Final — `is_final: true` + `speech_final: true`** (real example, from the Multilingual
Code-Switching docs, so it also shows the multilingual fields): [S27]

```json
{
  "type": "Results",
  "channel_index": [0, 1],
  "duration": 2.91,
  "start": 2.84,
  "is_final": true,
  "speech_final": true,
  "channel": {
    "alternatives": [
      {
        "transcript": "veo que actualmente cuenta con cobertura integral",
        "confidence": 1.0,
        "languages": ["es"],
        "languages_hinted": ["en", "es"],
        "words": [
          { "word": "veo", "start": 2.9199998, "end": 3.32, "confidence": 0.97314453, "language": "es" },
          { "word": "que", "start": 3.32, "end": 3.3999999, "confidence": 1.0, "language": "es" }
        ]
      }
    ]
  },
  "metadata": {
    "request_id": "01a15a73-2fe4-7be1-b95b-c3d34f6f3950",
    "model_info": { "name": "general-nova-3", "version": "1990-01-01.2", "arch": "nova-3" },
    "model_uuid": "60457470-46ba-4b26-8735-fb6853631a05"
  },
  "from_finalize": false
}
```

For an English-only stream, drop `languages`, `languages_hinted`, and the per-word `language`. With
diarization on, each word additionally carries `speaker: 0|1|…`. [S14]

**(c) `Metadata`** — sent once when the socket closes (see A4 for the exact payload). Schema:
`type, transaction_key (deprecated), request_id, sha256, created, duration, channels`. [S9][S1]

**(d) `UtteranceEnd`** (real example): [S11]

```json
{
  "channel": [0, 1],
  "last_word_end": 2.395,
  "type": "UtteranceEnd"
}
```

`channel` is `[channel_index, total_channels]`. `last_word_end` matches
`channel.alternatives[0].words[n].end` of the preceding final. **`-1` = stale, ignore.**

**(e) `SpeechStarted`** (bonus, `vad_events=true`): [S30]

```json
{ "type": "SpeechStarted", "channel": [0, 1], "timestamp": 9.54 }
```

Full server→client message set on `/v1/listen`: `Results`, `Metadata`, `UtteranceEnd`,
`SpeechStarted`, `ConfigureSuccess`, `ConfigureFailure`, `Error`. Fatal failures close the socket;
`Error` carries `{type, variant, description, message?}`. [S1]

### D2. Prerecorded response with `smart_format` + `diarize` + `utterances`

> **Provenance note:** the *schema* below is exact (OpenAPI + the Diarization and Paragraphs pages).
> No single published example turns on all three flags at once, so the **values** are illustrative
> and assembled from three doc examples rather than copied from one real response. [S2][S14][S29]

```json
{
  "metadata": {
    "request_id": "a847f427-4ad5-4d67-9b95-db801e58251c",
    "sha256": "154e291ecfa8be6ab8343560bcc109008fa7853eb5372533e8efdefc9b504c33",
    "created": "2024-05-12T18:57:13.426Z",
    "duration": 25.933313,
    "channels": 1,
    "models": ["30089e05-99d1-4376-b32e-c263170674af"],
    "model_info": {
      "30089e05-99d1-4376-b32e-c263170674af": {
        "name": "2-general-nova", "version": "2024-01-09.29447", "arch": "nova-2"
      }
    },
    "diarize_info": { "model_uuid": "9a1c8b3e-2f44-4c8a-b1d0-example0000", "arch": "v2" },
    "transaction_key": "deprecated"
  },
  "results": {
    "channels": [
      {
        "alternatives": [
          {
            "transcript": "Yeah, as much as, it's worth having a talk to the neighbors. Hello.",
            "confidence": 0.9840088,
            "words": [
              {
                "word": "yeah", "punctuated_word": "Yeah,",
                "start": 0.08, "end": 0.32, "confidence": 0.9975586,
                "speaker": 0, "speaker_confidence": 0.98
              },
              {
                "word": "hello", "punctuated_word": "Hello.",
                "start": 15.259043, "end": 15.338787, "confidence": 0.9721591,
                "speaker": 1, "speaker_confidence": 0.5853265
              }
            ],
            "paragraphs": {
              "transcript": "\nYeah, as much as, it's worth having a talk to the neighbors.\n\nHello.",
              "paragraphs": [
                {
                  "sentences": [
                    { "text": "Yeah, as much as, it's worth having a talk to the neighbors.", "start": 0.08, "end": 3.77 }
                  ],
                  "speaker": 0,
                  "num_words": 11,
                  "start": 0.08,
                  "end": 3.77
                },
                {
                  "sentences": [{ "text": "Hello.", "start": 15.25, "end": 15.33 }],
                  "speaker": 1,
                  "num_words": 1,
                  "start": 15.25,
                  "end": 15.33
                }
              ]
            },
            "entities": [
              {
                "label": "CARDINAL", "value": "55", "raw_value": "fifty five",
                "confidence": 0.99, "start_word": 4, "end_word": 6
              }
            ]
          }
        ],
        "detected_language": "en",
        "search": [
          { "query": "neighbors", "hits": [{ "confidence": 0.99, "start": 2.9, "end": 3.6, "snippet": "talk to the neighbors" }] }
        ]
      }
    ],
    "utterances": [
      {
        "start": 0.08, "end": 3.77, "confidence": 0.98, "channel": 0,
        "transcript": "Yeah, as much as, it's worth having a talk to the neighbors.",
        "speaker": 0, "id": "bfa4b5f9-0000-4000-8000-000000000001",
        "words": [
          { "word": "yeah", "start": 0.08, "end": 0.32, "confidence": 0.9975586, "speaker": 0, "speaker_confidence": 0.98, "punctuated_word": "Yeah," }
        ]
      }
    ]
  }
}
```

Key structural facts: `results.channels[]` is an array (one entry per channel with
`multichannel=true`, always length 1 otherwise); `alternatives[]` is an array but index 0 is the only
one populated; `paragraphs` is an **object** containing a `transcript` string *and* a nested
`paragraphs` array (the nesting is a known wart); `entities` sits on the alternative, next to `words`;
`utterances` sits on `results`, next to `channels`. `speaker_confidence` is prerecorded-only. [S2][S14][S29]

---

## E. Operational

### E1. Pricing and free credit

| Plan | Terms |
|---|---|
| **Pay as you go** | "No minimums, no expiration, and no credit card required. **Free $200 credit** then pay-as-you-go." The $200 ≈ **43,000 minutes / 700+ hours** of Nova-3, and **does not expire**. |
| **Growth** | Pre-paid annual credits from **$4,000/yr**, up to 20% off, higher concurrency; overages bill at Growth rate **+10%**, charged weekly on Mondays. |
| **Enterprise** | Custom. |

Per-minute speech-to-text (pay-as-you-go → Growth): **Flux STT English $0.0077 → $0.0065**;
**Flux STT Multilingual $0.0078 → $0.0068**; **Nova-3 Monolingual $0.0077 → $0.0065**;
**Nova-3 Multilingual $0.0092 → $0.0078**. [S19]

Billed **per second of processed audio, with no rounding to blocks** ("a 14-second file bills as 14
seconds"). **Multichannel bills total processed duration** (10 min stereo = 20 min). [S19]

Add-ons: **Smart Formatting — included, $0**; Redaction $0.0020/min; **Speaker Diarization
$0.0020/min**; Keyterm Prompting $0.0013/min; Entity Detection $0.0017/min; Audio Intelligence
(summarize/topics/sentiment/intents) $0.0003/1k in + $0.0006/1k out. [S19]

Legacy, unchanged: **Nova-2 streaming $0.35/hr, Enhanced $0.99/hr, Base $0.87/hr** (Growth ~12.5% lower). [S19]

> **UNCERTAIN — streaming vs prerecorded rate split.** The public rate card does **not** publish
> separate streaming and prerecorded rates; the same per-minute figure covers both. Yet the pricing
> FAQ says "Monolingual **streaming** is $0.46 an hour … **Pre-recorded runs lower**." No
> prerecorded-specific number is published, so treat streaming = $0.0077/min as solid and
> "prerecorded is cheaper" as an unquantified claim. ($0.0077 × 60 = $0.462, matching the FAQ's $0.46.)

### E2. Concurrency limits and the error you get

Limits are **per project, not per account or per API key** — multiple keys in one project share the
pool, and spreading load across extra projects is a ToS violation (secondary self-serve projects are
throttled to 1 concurrent stream). [S17]

Pay-as-you-go, North America (EU/AU/IN are lower for some rows):

| Workload | Limit |
|---|---|
| Nova-3 / Nova-2 / Nova-1 / Enhanced / Base — **streaming** | **150** concurrent |
| Nova-3 etc. — **prerecorded** | **50** concurrent |
| Flux STT — streaming | 150 concurrent |
| **Speaker Diarization** — streaming / prerecorded | 50 / 50 (EU/AU/IN: 25 / 25) |
| Whisper Cloud — prerecorded | **3** (NA only) |

Growth raises NA streaming to 225 (EU/AU/IN stay 150). Enterprise starts at 300 streaming / 200
prerecorded. Limits were raised "up to 3X" for streaming on 2026-02-11. [S17][S32]

**Errors when exceeded:** [S18]

```json
{ "err_code": "TOO_MANY_REQUESTS", "err_msg": "Too many requests. Please try again later", "request_id": "uuid" }
```
→ HTTP **429**. Docs recommend exponential backoff. Running out of money is a different error:
```json
{ "err_code": "ASR_PAYMENT_REQUIRED",
  "err_msg": "Project does not have enough credits for an ASR request and does not have an overage agreement.",
  "request_id": "uuid" }
```
→ HTTP **402**. Bad key → **401** `INVALID_AUTH`. No model access → **403** `INSUFFICIENT_PERMISSIONS`.
Mid-upload abort → **422** `ASR_UNPROCESSABLE_ENTITY`.

### E3. API keys

- **Console:** `console.deepgram.com` → **Projects** dropdown (top-left) → pick project → **Settings**
  → **API Keys** → **Create a New API Key**. Copy the secret immediately — it is never shown again. [S31]
- **Your first key must be created in the Console**; afterwards you can create keys via
  `POST /v1/projects/{project_id}/keys`. [S31]
- **Scoped keys exist.** Each key gets a **Role** ("Permissions") that bounds what it can call
  (`usage:read`, `usage:write`, `keys:write`, …), plus an **Expiration** (date / duration / never) and
  immutable **Tags** that get stamped onto usage from that key. [S31]
- **Temporary keys are capped at 250/day**; for short-lived client credentials prefer Token-Based
  Auth (JWT, 30 s TTL). [S31][S4]
- **Env var:** `DEEPGRAM_API_KEY`. This is the name used in Deepgram's own official docs and SDK
  examples (e.g. `API_KEY = os.getenv("DEEPGRAM_API_KEY")`). It is a convention rather than a
  protocol requirement, but it is the name to use. [S7]

### E4. Is a project ID required?

**No — not for transcription.** `POST /v1/listen` and `wss://.../v1/listen` authenticate with the API
key alone; no project ID appears in either request. A project ID is only needed for **management**
endpoints (`/v1/projects/{project_id}/keys`, usage, billing, members). Every key is already bound to
exactly one project, so the project is implied. [S2][S1][S31]

### E5. Latency claims for streaming

| Component | Typical | Source |
|---|---|---|
| Network transit | 20–200 ms | [S26] |
| **Transcription latency** | **150–300 ms** ("optimized to deliver 300 ms or less") | [S26] |
| **Total end-to-end transcript latency** | **200–500 ms** | [S26] |
| End-of-turn detection (Flux only) | 100–500 ms | [S26] |

Nova-3 is described as "delivering **sub-300 ms** streaming latency". Practical buffer guidance:
**20–100 ms of audio per chunk**; bigger buffers add latency, smaller ones add overhead. Do not use
`start`/`duration` for precision timing — "not guaranteed to be accurate at millisecond-level
precision". [S26]

### E6. Reconnect / resumption and max session duration

- **No session resumption.** On a dropped connection "your application must initialize a new WebSocket
  connection and start a new streaming session". There is no resume token, no offset handshake, no
  replay. [S25]
- **You must buffer audio yourself** while reconnecting, then push the backlog (the SDK guidance calls
  this out explicitly, with the caveat that the 1.25x realtime cap means a large backlog will lag). [S25]
- **Timestamps restart at 0** on every new connection — "every time you reconnect… the timestamps on
  your audio begin from 00:00:00 again." You must maintain a starting offset and add it yourself. [S25]
- **Must send audio within 10 s of the new socket opening**, or it closes. [S25]
- **Max duration for a single WebSocket: NOT DOCUMENTED. UNCERTAIN.** No published cap exists in the
  current docs. The only documented limits that end a session are the **~10 s no-audio timeout**
  (`NET-0001`), the **1.25x realtime** send cap, and concurrency. Docs frame long streams purely as a
  "be ready to reconnect" problem ("a day-long virtual conference"), implying multi-hour single
  connections are expected to work. [S25][S7]

### E7. Official Rust SDK?

**Crate: `deepgram`** — [crates.io/crates/deepgram](https://crates.io/crates/deepgram)

| | |
|---|---|
| Latest version | **0.14.0**, published **2026-10-20** (0.13.0 on 2026-10-12; rapid release cadence) |
| Repo | `github.com/deepgram/deepgram-rust-sdk` — **inside the official `deepgram` GitHub org** |
| License | MIT |
| Downloads | ~603k all-time, ~100k recent |
| Feature flags | `listen`, `speak`, `read`, `manage`, `agent` (all default); opt-in `connect-diagnostics`, `rustls-tls-native-roots` |
| Under the hood | `tokio-tungstenite` + `rustls` — i.e. the same stack you'd hand-roll |

**Officially maintained? Read carefully — the answer is "not officially supported".**

- The crate description reads **"Community Rust SDK for Deepgram's automated speech recognition
  APIs."** (It said "Official Rust SDK" up to 0.6.x; that wording was dropped.)
- The README states: *"This SDK is currently **Community owned** but is moving to a stable `1.0`
  version soon."*
- The docs' main SDK list (home page) names only **Python, JavaScript, Go, .NET, Java** — **no Rust**.
- **But** the docs' **SDK feature matrix does list `Rust`** for streaming and prerecorded parameters,
  and publishes by Deepgram staff (`dg-coreylweathers`, and the repo owner is the deepgram org).

So: maintained *in* the official org by people close to Deepgram, but self-declared community-owned
and excluded from the officially-supported SDK list. Plan for occasional breaking changes (there is a
`Migrating-v0.12-to-v0.13` doc, and 0.12 → 0.14 landed within 13 days). [S20][S34][S35][S36]

**Is raw `tokio-tungstenite` viable?** **Yes — it is genuinely simple, and arguably the lower-risk
choice here.** The protocol surface you need is small: [S1][S6][S7][S8][S9]

1. `connect_async` to `wss://api.deepgram.com/v1/listen?model=nova-3&language=en&encoding=linear16&sample_rate=16000&interim_results=true&smart_format=true&utterance_end_ms=1000&vad_events=true&endpointing=300`
2. Handshake header `Authorization: Token <KEY>` (native client — no subprotocol workaround needed).
3. `Message::Binary` for 20–100 ms PCM chunks; `Message::Text` for `{"type":"KeepAlive"}` every 3–5 s.
4. Demux incoming `Message::Text` JSON on `type` ∈ `Results | Metadata | UtteranceEnd | SpeechStarted | Error`.
5. On stop: `{"type":"Finalize"}` → wait for `from_finalize` results → `{"type":"CloseStream"}` → wait for `Metadata` → close.
6. Reconnect wrapper + timestamp offset for long sessions.

That is roughly what the SDK does internally, and doing it yourself removes a fast-moving 0.x
dependency from your tree. The one thing the SDK gives you for free that's mildly annoying to
reimplement is `connect-diagnostics` (per-phase DNS/TCP/TLS/upgrade timings).

---

## F. Sanity check — the core uncertainty

> **"Do we need to give Deepgram a full mp3 file, or can it do live streaming?"**

**Live streaming — no mp3 required.** These are two independent products over two transports, and the
live one is a first-class API:

- **Prerecorded** = you have a finished file; you `POST` it (or its URL) and get one JSON document back.
- **Live streaming** = you hold a WebSocket open while the user speaks and receive a stream of
  `Results` messages, most of them partial.

An mp3 file is only ever an *input format for the batch path*. For live dictation you never produce a
file at all.

**What a live-streaming integration actually requires from the client:**

1. **Capture the mic continuously while the user speaks** and keep the socket open for the whole
   session — not just at the end. Start the session at record-start, not at record-stop.
2. **Convert to a supported raw format** — typically 16 kHz mono `linear16` (s16le) — and declare it:
   `encoding=linear16&sample_rate=16000&channels=1`. (Or send containerized audio and declare nothing.)
3. **Send small binary chunks on a paced loop** — 20–100 ms of audio per frame, at roughly realtime.
   You cannot catch up faster than 1.25× realtime, and the first frame must land within 10 s of connect.
4. **Handle the three result classes distinctly:**
   - `is_final: false` → replace the in-progress line (cumulative from `start`, revised each time);
   - `is_final: true` → append to committed text;
   - `speech_final: true` / `UtteranceEnd` → a turn boundary; use it to close a segment or trigger
     downstream work. (`UtteranceEnd` needs `interim_results=true` + `utterance_end_ms`.
     Ignore `last_word_end: -1`.)
5. **Keep the socket alive through silence** — `{"type":"KeepAlive"}` every 3–5 s, as a **text** frame,
   or the server kills the connection at ~10 s with `NET-0001`.
6. **Finalize on stop, then close** — `Finalize` to flush the tail (this is what recovers the last
   word that's still sitting in the server's buffer), then `CloseStream`, then wait for the `Metadata`
   frame before dropping the socket. Skipping `Finalize` is the usual cause of "the last sentence went
   missing".
7. **Reconnect with a buffer** for long sessions — new socket, re-send backlog, and add a timestamp
   offset because word times restart at zero on every connection.

For a Rust desktop dictation app specifically: `tokio-tungstenite` + `Authorization: Token` header +
`linear16`/16 kHz mono chunks is a complete, sufficient implementation, and it makes perceived latency
a function of when the user stops talking rather than of how long the upload takes.

---

## G. Precision addendum (follow-up questions)

### G1. Streaming diarization, exactly

**(a) Recommended syntax.** `diarize_model=<value>`, added to the WS query string — same form as any
other param, no body, no JSON. Documented values are **only** these enums:

| Value | Streaming | Batch |
|---|---|---|
| `latest` | ✅ resolves to **v1** | ✅ resolves to **v2** |
| `v1` | ✅ pins v1 streaming diarizer | ✅ |
| `v2` | ❌ **"returns a validation error"** on streaming | ✅ |

There is **no documented support for passing a raw model UUID / model id** as `diarize_model`
(`diarize_info.model_uuid` is *returned*, not accepted). **UNCERTAIN** whether an undocumented UUID
value works — use `latest` or `v1`. [S14][S2]

**(b) Does plain `diarize=true` still work on streaming Nova?** **Yes.** Verbatim: *"The deprecated
`diarize=true` parameter also continues to work for streaming and routes to the v1 diarizer."* It is
marked `deprecated: true` in both API references, so prefer `diarize_model`. **Never send both** —
"requests that set both are rejected." [S14][S1][S2]

**(c) Which models support streaming diarization?** Nova family (Nova-1, Nova-2, Nova-3), plus
`enhanced` and `base`. Docs: *"Diarization is compatible with all Nova batch models (Nova-1, Nova-2,
Nova-3) as well as enhanced and base. **Whisper is not supported.**"* The rate-limit table also has an
explicit **"Speaker Diarization — Streaming"** row, so the streaming path is real and independently
rate-limited. [S14][S17]

**Flux: not supported as far as the docs go — do not rely on it.** The diarization page's chip row
claims `Streaming:Flux`, but the **Flux Feature Overview matrix contains no diarization row at all**
(it lists profanity filter, numerals, redaction, word timestamps, keyterm, encoding, sample rate,
EOT — no diarization), and Flux's output schema is turn-based (`TurnInfo`) rather than word arrays
under `alternatives`. **Treat Flux diarization as unsupported / UNCERTAIN.** [S14][S37]

**(d) Streaming response with diarization on.** The shape is *not* the prerecorded
`results.channels[0].alternatives[0].words[]` — streaming has **no `results` wrapper and no `channels`
array**. The word array lives at `channel.alternatives[0].words[]`, and each word gains `speaker`:

```json
{
  "type": "Results",
  "channel_index": [0, 1],
  "duration": 1.04,
  "start": 0.0,
  "is_final": true,
  "speech_final": true,
  "channel": {
    "alternatives": [
      {
        "transcript": "hello",
        "confidence": 0.97,
        "words": [
          { "word": "hello", "start": 15.259043, "end": 15.338787, "confidence": 0.9721591, "speaker": 0 }
        ]
      }
    ]
  },
  "metadata": {
    "request_id": "…",
    "model_info": { "name": "general-nova-3", "version": "…", "arch": "nova-3" },
    "model_uuid": "…",
    "diarize_info": { "model_uuid": "9a1c8b3e-…", "arch": "v1" }
  }
}
```

- Speaker ids are **integers starting at 0**, assigned **per word**, and they are per **channel**
  (`channel_index`) when multichannel is on.
- Streaming returns `speaker` **only**; `speaker_confidence` is prerecorded-only.
- `metadata.diarize_info` (`{model_uuid, arch}`) is present only when a diarizer actually ran — never
  `null` or `{}`, just absent. Use it to debug `diarize_model=latest` resolving to v1 vs v2. [S14]
- **Stability across a session: NOT DOCUMENTED. UNCERTAIN.** Nothing states that `speaker: 0` in
  result 1 is the same human as `speaker: 0` in result 40. Two things are documented that bear on it:
  the diarizer runs per-stream (metadata reports one `diarize_info` per response), and reconnecting
  creates an entirely **new session** — so anything you learn about speaker numbering must be assumed
  to reset on reconnect. [S14][S25]

### G2. `dictation` × `smart_format`, exactly

**Quoted requirements (Dictation page, verbatim):**

> "To enable Dictation, set the `dictation` parameter to `true` in your API request. Note that the
> `punctuate` parameter must also be enabled for Dictation to work."
>
> "The Punctuation feature must be enabled for Dictation to work. Be sure to add
> `dictation=true&punctuate=true` to your request." [S12]

**Relationship to `smart_format` (Smart Formatting page, verbatim, under "Additional Formatters"):**

> "These formatters are **not** included in Smart Formatting but may be enabled individually:
> Measurements … Dictation." [S13]

- `punctuate=true` — **required**, documented, non-negotiable.
- `smart_format=true` together with `dictation=true` — **no documented conflict or precedence rule.**
  `smart_format` implies `punctuate`, so `dictation=true&smart_format=true` satisfies the punctuation
  requirement without an explicit `punctuate=true`; but since the docs only ever show
  `dictation=true&punctuate=true`, **send all three** to be safe: `dictation=true&punctuate=true&smart_format=true`.
  **UNCERTAIN** which formatter wins if a spoken command and a smart-format rule collide (e.g. you
  literally say "period" at the end of a number) — no doc addresses precedence. Test it.
- English only — the page's chip row reads `English (all available regions)`. [S12]
- Streaming variant: the `dictation` param is declared in the **streaming** AsyncAPI, and the chips
  read `Streaming:Nova Streaming:Flux`; but the **SDK matrix lists `dictation` as GA for prerecorded
  only**. So: real param on streaming, **no published streaming example, evidence of streaming
  behaviour is thin → UNCERTAIN**. [S1][S20][S12]

### G3. `endpointing` vs `utterance_end_ms` on streaming Nova-3

| | `endpointing` | `utterance_end_ms` |
|---|---|---|
| Type | integer ms (or `false`) | integer ms |
| Default | **`10`** (enabled by default) | **`1000`** |
| Min | **not documented** (10 is the default; docs' examples use 100–500) → **UNCERTAIN** | **`1000`** |
| Max | **not documented** → **UNCERTAIN** | **`5000`** |
| Step | n/a | "Any integer value within range" |
| `false` | **disables endpointing** (results then follow Deepgram's chunking cadence) | not applicable |
| Requires | nothing | **`interim_results=true`** |
| Emits | a flag on a normal `Results` message: `"speech_final": true` | a **separate message**: `{"type":"UtteranceEnd","channel":[0,1],"last_word_end":2.395}` |
| Trigger | VAD detects a pause long enough to be a speech endpoint → that range is finalized | a gap of N ms after the **last finalized word**, measured from interims/finals |
| Blind spot | — | gaps *inside* a single final result are not detected; needs another interim to close the gap |

[S10][S11]

**The two are independent.** `speech_final` says "I finalized this because I heard a pause";
`UtteranceEnd` is a separate server-side gap-detection event with its own clock that starts at
`last_word_end`. One trap: the endpointing page's own example shows `"is_final": false` **with**
`"speech_final": true` — do not assume `speech_final` implies `is_final`. Use `is_final` for
commit/append, `speech_final` and `UtteranceEnd` for turn boundaries. [S10]

**"No endpoint, because we only ever send `Finalize`":** with continuous speech and no pause, endpointing
never fires, so finals arrive with `speech_final: false` (or the field absent — it is not in the
schema's `required` list) and **no `UtteranceEnd` is ever emitted**, because that event needs a
post-final gap. Your `Finalize` then returns Results that may carry `"from_finalize": true` — which is
a *different* signal from `speech_final` — and `CloseStream` finally yields `Metadata`. **UNCERTAIN in
detail:** docs do not state whether a Finalize-induced final sets `speech_final`; the Finalize page
only promises `from_finalize`. **Design implication:** if you drive a dictation UI purely off
"stop → Finalize", treat `from_finalize` (and, failing that, the `Metadata` close) as your
end-of-transcript signal rather than waiting for `speech_final`/`UtteranceEnd`. [S8][S9][S10][S11][S1]

### G4. Literal URL for a Rust desktop client

16 kHz mono PCM, Nova-3, English, punctuation + smart format + dictation, interim results,
endpointing, utterance end, diarization on:

```
wss://api.deepgram.com/v1/listen?model=nova-3&language=en&encoding=linear16&sample_rate=16000&channels=1&punctuate=true&smart_format=true&dictation=true&interim_results=true&endpointing=300&utterance_end_ms=1000&diarize_model=latest
```

Built only from documented params and legal values:

| Param | Value | Why that value |
|---|---|---|
| `model` | `nova-3` | 2026 default for transcription; required explicitly (no streaming default) [S15][S1] |
| `language` | `en` | explicit; default is `en` but don't rely on it [S1] |
| `encoding` | `linear16` | raw PCM — required for headerless audio [S21] |
| `sample_rate` | `16000` | required whenever `encoding` is set [S22] |
| `channels` | `1` | mono mic; matches `multichannel=false` [S1] |
| `punctuate` | `true` | required by `dictation` [S12] |
| `smart_format` | `true` | dates/times/currency/URLs; free; implies punctuate [S13][S19] |
| `dictation` | `true` | spoken "comma"/"period" → punctuation [S12] |
| `interim_results` | `true` | needed for live text **and** required by `utterance_end_ms` [S16][S11] |
| `endpointing` | `300` | 300 ms pause = end of speech; 10 ms default is far too twitchy for dictation [S10] |
| `utterance_end_ms` | `1000` | valid minimum; max 5000 [S11] |
| `diarize_model` | `latest` | current syntax; **do not** also send `diarize=true` [S14] |

Optional additions: `&vad_events=true` (adds `SpeechStarted` messages), `&keyterm=<term>` (Nova-3
only), `&tag=dictation`. Omit `encoding`/`sample_rate` entirely if you ever send containerized audio
(e.g. WAV) instead of PCM. [S1][S24]

Handshake in Rust — the URL is unchanged and the header is the only non-default part; the idiomatic
`tokio-tungstenite` form is to build a `Request` via the `IntoClientRequest` trait and add one header
(exact helper names vary by tungstenite version, so treat the snippet as illustrative rather than
copy-paste-exact — **UNCERTAIN** on the precise `generate_key` path in your pinned version):

```rust
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

let mut req = url.into_client_request()?;           // url = the wss:// string above
req.headers_mut().insert(
    "Authorization",
    format!("Token {DEEPGRAM_API_KEY}").parse()?,
);
let (ws, _resp) = tokio_tungstenite::connect_async(req).await?;
```

**Cost note:** this URL turns on diarization, which is a **$0.0020/min** add-on and drops your
streaming concurrency from 150 to **50** on pay-as-you-go. If you don't actually need speaker labels
for single-user dictation, drop `diarize_model`. [S19][S17]

---

## Sources

All fetched 2026-11-01.

**Speech-to-text API reference (authoritative for parameters and payloads)**
1. https://developers.deepgram.com/reference/speech-to-text/listen-streaming.md — embedded AsyncAPI 2.6.0 for `/v1/listen`: full query-param set, defaults, all client/server message schemas
2. https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded.md — `POST /v1/listen` query params, defaults, response types
3. https://developers.deepgram.com/reference/regional-endpoints.md — EU / AU / India endpoints, WS URLs, Whisper exclusion
4. https://developers.deepgram.com/reference/authentication.md — `Authorization: Token <key>`, Bearer JWT
5. https://developers.deepgram.com/docs/using-the-sec-websocket-protocol.md — `Sec-WebSocket-Protocol: token, <KEY>`, and when it is required
6. https://developers.deepgram.com/docs/lower-level-websockets.md — `SendBinary` vs `SendMessage`, 10-second no-audio termination, common disconnect causes
7. https://developers.deepgram.com/docs/audio-keep-alive.md — KeepAlive payload, 3–5 s cadence, 10 s timeout, `NET-0001`, text-frame requirement, word-timing behaviour
8. https://developers.deepgram.com/docs/finalize.md — Finalize payload, `channel` field, `from_finalize`
9. https://developers.deepgram.com/docs/close-stream.md — CloseStream payload, flush → results → Metadata → terminate
10. https://developers.deepgram.com/docs/endpointing.md — units, default 10 ms, `endpointing=false`, `speech_final`
11. https://developers.deepgram.com/docs/utterance-end.md — `utterance_end_ms` min 1000 / max 5000, interim dependency, `UtteranceEnd` JSON, `last_word_end: -1`
12. https://developers.deepgram.com/docs/dictation.md — what dictation really does, `punctuate` requirement, English-only, command table
13. https://developers.deepgram.com/docs/smart-format.md — capabilities, implicit punctuation, 3-second streaming finalization, `no_delay`, dictation listed as *not* included
14. https://developers.deepgram.com/docs/diarization.md — `diarize` deprecated, `diarize_model` matrix, `speaker` vs `speaker_confidence`, model compatibility, `diarize_info`
15. https://developers.deepgram.com/docs/models-languages-overview.md — all model names, per-model language lists, Nova-3 multilingual, recommendations
16. https://developers.deepgram.com/docs/interim-results.md — interim semantics and two real partial-transcript examples
17. https://developers.deepgram.com/reference/api-rate-limits.md — per-project concurrency by plan and region, diarization/Whisper rows
18. https://developers.deepgram.com/docs/errors.md — 429 / 402 / 401 / 403 / 422 payloads
19. https://deepgram.com/pricing — rate card, $200 credit, per-second billing, add-on pricing, concurrency FAQ
20. https://developers.deepgram.com/sdks/sdk-features.md — per-parameter SDK support incl. `Rust` column; `vad_events` = Beta
21. https://developers.deepgram.com/docs/encoding.md — encoding enum, raw-vs-containerized rule, requires `sample_rate`
22. https://developers.deepgram.com/docs/sample-rate.md — `sample_rate` semantics, AMR constraints
23. https://developers.deepgram.com/docs/supported-audio-formats.md — "100+" formats incl. MP3/WAV/FLAC/Opus/WebM
24. https://developers.deepgram.com/docs/determining-your-audio-format-for-live-streaming-audio.md — containerized vs raw decision rule and failure mode
25. https://developers.deepgram.com/docs/recovering-from-connection-errors-and-timeouts-when-live-streaming-audio.md — no resumption, buffer-and-replay, timestamp reset, **1.25x realtime cap**, 10 s first-audio rule
26. https://developers.deepgram.com/docs/measuring-streaming-latency.md — 150–300 ms transcription, 200–500 ms total, 20–100 ms buffers
27. https://developers.deepgram.com/docs/multilingual-code-switching.md — `language=multi`, `language_hint`, `endpointing=100`, per-word `language`, streaming final example
28. https://developers.deepgram.com/docs/multichannel-vs-diarization.md — how diarization and multichannel compose; per-channel speaker numbering
29. https://developers.deepgram.com/docs/paragraphs.md — `paragraphs` response object shape
30. https://developers.deepgram.com/docs/speech-started.md — `vad_events`, `SpeechStarted` JSON
31. https://developers.deepgram.com/docs/create-additional-api-keys.md — console path, roles/scopes/expiration/tags, 250 temp keys/day
32. https://developers.deepgram.com/changelog/llms.txt + https://developers.deepgram.com/changelog/2026/10/27.md — recency checks: India GA 2026-09-15, concurrency raised 2026-02-11, `dg-token` callback verification removed 2026-10-27, mid-stream keyterms 2026-10-12
33. https://developers.deepgram.com/llms.txt — doc index (how the `.md` mirrors were discovered)
34. https://crates.io/api/v1/crates/deepgram — `deepgram` 0.14.0, 2026-10-20, MIT, "Community Rust SDK", repo/homepage
35. https://raw.githubusercontent.com/deepgram/deepgram-rust-sdk/main/README.md — "Community owned", feature flags, `connect-diagnostics`, rustls notes
36. https://developers.deepgram.com/home.md — official SDK list (Python, JS, Go, .NET, Java — no Rust)
