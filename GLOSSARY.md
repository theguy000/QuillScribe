# QuillScribe

A desktop dictation app: the user presses a global shortcut, speaks, and text is typed or pasted into whatever application has focus. Speech-to-text is delegated to a selectable cloud provider or a bundled local Whisper model.

## Language

**Dictation**:
The product's core act — the user speaks and text lands in another application. Spans one or more utterances.
_Avoid_: transcription (that is the output, not the act), recording

**Utterance**:
One complete press-to-talk recording, from pressing the recording shortcut to pressing it again. Produces exactly one history entry.
_Avoid_: dictation, session, clip, take

**Spoken punctuation**:
A formatter that turns spoken words such as "comma" and "new paragraph" into marks. Deepgram exposes this as `dictation=true`; never call it "dictation" here, because that word already means the product's core act.
_Avoid_: dictation mode, voice punctuation

**Provider**:
The speech-to-text service selected in Settings → Engine. Currently OpenAI, Groq, Gemini and Deepgram.
_Avoid_: engine (except as the tab's display label), backend, API

## Execution shapes

**Batch transcription**:
Every provider's current shape. The whole utterance is recorded first, then sent to the provider as a single request after the user stops.
_Avoid_: file mode, upload mode, one-shot

**Live session**:
An execution shape where audio is sent to the provider while the user is still speaking, and text arrives during the utterance. Opt-in for Deepgram; every other provider is batch-only.
_Avoid_: streaming (too broad — Gemini's chunked upload also "streams" yet remains batch), realtime mode

**Recorded mode**:
Deepgram's default execution shape: the finished utterance is uploaded to the prerecorded endpoint when recording stops. Chosen as the default because QuillScribe only delivers text at that moment anyway.
_Avoid_: file mode, batch Deepgram

## Live session vocabulary

**Interim segment**:
Text that the provider may still revise. It replaces the in-progress line and is never committed to the transcript.
_Avoid_: partial, preview, draft

**Final segment**:
Text the provider has committed to. It is appended to the transcript and never rewritten.
_Avoid_: confirmed, stable, locked

**Turn boundary**:
A signal that the speaker paused, marking where one final group ends and the next begins. It is not a commit signal and must not be confused with one.
_Avoid_: endpoint, utterance end

## Chrome

**Overlay**:
The small always-on-top window shown while recording: level bars, a timer, and a stop button. It has no room for text.
_Avoid_: widget, HUD, indicator, bubble
