# Where live typing will and will not write

Live typing puts each Final segment into the application that held focus when
the user started recording. The decision lives in `src/verdict.rs` as a pure
function over a description of the target, and the platform only supplies that
description (`target_window` in `src/delivery.rs`).

## The rules

- **Masked and secret inputs are refused.** Nothing is written into a password
  box or an API key field. The refusal covers the whole utterance, because the
  utterance belongs to one window and a window whose caret is in a secret input
  should not receive the words at all.
- **QuillScribe's own API key fields are refused the same way**, because they are
  masked inputs. The app knows them directly rather than through a role.
- **Refused text is not dropped.** It takes the same route as any fragment that
  never reached the document: it is owed at stop and put on the clipboard, with a
  status line saying the masked field was refused.
- **A dictated new line becomes a space in a terminal.** A pasted newline in a
  shell can run whatever was just dictated. Everywhere else the newline stays a
  newline.
- **Terminals and elevated windows are written to.** The operator cleared both
  (ticket 05). An elevated window on Windows silently discards synthetic input,
  so a fragment can be counted as delivered while nothing appears.

## How a target is recognised

| Platform | Secret input | Terminal |
| --- | --- | --- |
| Windows | UI Automation `IsPassword` on the **focused element**, plus the app's own key fields by name | The window's class name |
| Linux (X11) | The app's own key fields by name | The window's class name |
| Everywhere else | Nothing is refused | Nothing is detected |

The Windows role query asks for the focused element rather than the window,
because a password box sits inside an ordinary window class. It runs once, at
hotkey time, before the socket opens. It needs the app's own accessibility tree
enabled for QuillScribe's window; if the Windows accessibility settings turn the
whole tree off, the app's key fields fall back to nothing and will be typed into.

## The limitation, stated plainly

**Where the platform cannot report a role, the app types rather than refusing.**
This is deliberate, so a missing answer never blocks a user who is dictating
into an ordinary field in an application that does not implement accessibility
(some terminals, some Electron apps, and older native toolkits).

The consequences, so nobody meets them as a surprise:

- **On Linux, a password box outside QuillScribe is not detected.** There is no
  role query without a desktop-specific accessibility bridge, so a focused
  password box in another application receives the dictation. The terminal rule
  and the app's own key fields still work, because both read only the window
  class and the element name.
- **On Wayland live typing does not run at all**, because the focused window
  cannot be read, so this limitation cannot arise there. That fallback is
  ticket 16's.
- **On Windows, a target that answers no accessibility queries** reads as an
  ordinary field. The app types into it.
- **The terminal rule is a class-name list**, not a proof. A terminal that reports
  an unfamiliar class receives its newlines unchanged, which is the same
  behaviour as before this rule existed.

The refusal list is built in and not user editable, so this adds no settings
screen (ticket 05).
