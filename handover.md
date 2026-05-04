# QuillScribe Lite — Handover Notes

## Project Overview

QuillScribe is a voice-to-text transcription app. The `quillscribe-lite` branch replaces the Tauri (web) frontend with a native Slint UI, while keeping the same Rust backend logic (audio capture, Whisper transcription, config, etc.).

## Current Branch

`quillscribe-lite` — native Slint UI, no Tauri dependency.

## Recent Work: System Tray Fix

### Problem
The system tray icon was not visible on Linux. The app built and ran fine, but no tray icon appeared in the system tray.

### Root Cause
On Linux, `tray-icon` uses `libappindicator` which relies on GLib's D-Bus integration. The tray icon was being built and stored, but the **GLib main context was never iterated**, so `libappindicator` could never complete its D-Bus registration with the desktop shell.

The main branch (Tauri) doesn't have this problem because Tauri's event loop internally pumps the GLib context.

### Fix Applied
1. **`Cargo.toml`** — Added `glib = "0.18"` as a Linux-specific dependency.
2. **`src/tray.rs`** — Added `glib::MainContext::default().iteration(false)` in the tray event polling loop to pump the GLib main context each iteration.
3. **`src/lib.rs`** — `gtk::init()` must be called on the **main thread** before `tray::setup_tray()`, because `TrayIconBuilder::build()` internally uses GTK/libappindicator which requires GTK to be initialized.

### Key Lesson
- `gtk::init()` must happen on the main thread (before tray icon creation)
- `glib::MainContext::iteration()` must happen on the polling thread (to process D-Bus events)
- Calling `gtk::init()` on the wrong thread causes a panic: `GTK has not been initialized. Call gtk::init first.`

## Architecture

### Module Layout (`src/`)
| Module | Purpose |
|--------|---------|
| `lib.rs` | Main entry point, Slint UI wiring, callbacks |
| `audio.rs` | Audio capture via `cpal` |
| `commands.rs` | `AppState` struct (shared state) |
| `config.rs` | Config read/write, settings persistence |
| `hotkey.rs` | Global hotkey registration via `global-hotkey` |
| `output.rs` | Transcription output (clipboard, paste) |
| `sound.rs` | Notification sound playback via `rodio` |
| `statistics.rs` | Transcription history/stats |
| `tray.rs` | System tray icon via `tray-icon` |
| `whisper.rs` | Whisper transcription (local + API) |
| `window.rs` | Window management (drag, minimize, hide-to-tray, icons) |

### Key Dependencies
- **UI**: `slint` with `backend-winit` + `renderer-femtovg`
- **Tray**: `tray-icon` 0.17 (uses `libappindicator` on Linux)
- **Hotkey**: `global-hotkey` 0.6
- **Audio**: `cpal` 0.15
- **Transcription**: `whisper-rs` 0.16 (local), `reqwest` (API)
- **Linux-specific**: `gtk` 0.18, `glib` 0.18

### UI Framework
Slint UI files are in `ui/` directory, compiled via `slint-build` in `build.rs`. The `include_modules!()` macro generates the `App` struct with typed getters/setters/callbacks.

## Known Issues / TODOs

- **Overlay mode**: Not fully implemented for Slint (main branch has Tauri overlay window)
- **Auto-update**: `self_update` crate is a dependency but not wired up yet
- **History panel**: `on_load_history` callback exists but doesn't feed data into UI
- **Shortcut recording**: `on_settings_start_recording_shortcut` / `on_settings_stop_recording_shortcut` are stubs
- **Tray model submenu**: Main branch has a "Use Model" submenu in tray; current branch has a TODO comment
- **ALSA warnings**: Jack/ALSA errors on startup are harmless (audio fallback works)

## Build & Run

```bash
cargo run
```

Linux requires: `gtk3`, `libappindicator` (or `libayatana-appindicator`), `xdotool`

## Main Branch Reference

The `main` branch uses Tauri 2.x with a Svelte frontend (`src/` for JS, `src-tauri/` for Rust). The `quillscribe-lite` branch is a full rewrite using Slint for the UI layer, keeping the same backend modules.
