# Todo

Open work from the Deepgram live-typing review. None of it blocks a release.

## Live session behaves like a recorded one

Reported on Windows with the rc2 build: with Deepgram set to live, live typing on, and a key present, text still appears only at stop.

- The app writes `Deepgram session: <url>` on every utterance (`src/deepgram/stream.rs`), but `windows_subsystem = "windows"` in `src/main.rs` means release builds have no console, so nobody sees it.
- Next step: log to `%LOCALAPPDATA%\QuillScribe\logs\` as well as the console, then check whether a socket opens at all.
- If no socket opens, the suspect is the recording tap: `AudioController::set_tap` is set after `start_recording`, and `set_tap(None)` runs before `stop_recording` on the stop path.
- Until this is fixed, treat live typing on Windows as untested.

## Space before a fragment

Every paste that added a leading space recorded only the words, so the ledger thought the document ended on a letter and the next fragment added a second space. The fix was written and discarded with PR #4.

- Re-apply the patch if the double space returns: record the separator in `DeliverySession::record_written`, and paste without a leading space when the window has no ledger entry.
- A window with no history no longer gets a leading space, which is a behaviour change to weigh before shipping.

## Space typed by the user

The ledger only knows what the app typed. If the user types a space between two utterances, or moves the caret, the next paste still assumes the text is tight.

- Needs a document read: the character before the caret, via the UI Automation text pattern on Windows.
- On Linux there is no accessibility connection at all; that would mean AT-SPI over D-Bus, which would also make password detection reliable there.

## Codacy `unsafe-usage`

Ten findings remain on `src/delivery/platform/windows.rs` and `automation.rs`. Each is a raw Win32 call, which the `windows` crate requires to be `unsafe`.

- Codacy only exposes the pattern settings for organisation repositories, and this one is a personal repo.
- To clear them: move the repo to a GitHub organisation, or ignore the findings in the Codacy UI.

## Release tags

- `v0.3.25-rc1` and `v0.3.25-rc2` were deleted along with their tags, and the two release-workflow commits were reverted.
- If a pre-release build is needed again, the Inno Setup step needs a numeric version: it rejects `0.3.25-rc1` in the version resource.
- The AUR workflow fires on any `v*.*.*` tag and fails on a suffix, because Arch `pkgver` allows no hyphen.
