# QuillScribe

<p align="center">
  <img src="public/logo-light.svg" alt="QuillScribe Logo" width="280" />
</p>

<p align="center">
  A voice-powered transcription desktop app with local and cloud speech-to-text support.
</p>

---

## Features

- **Voice Recording** — Real-time audio capture with live level monitoring and device selection
- **Local Transcription** — Offline speech-to-text via Whisper models (Tiny through Large, including distilled variants)
- **Cloud Transcription** — OpenAI and Groq API support for fast cloud-based transcription
- **Flexible Output** — Copy to clipboard, simulate typing into the active window, or both
- **Global Hotkey** — Start and stop recording from any application
- **12 Themes** — 6 light and 6 dark themes with instant switching
- **Floating Overlay** — Recording timer that stays visible over other windows
- **System Tray** — Minimize to tray with quick-access context menu
- **Transcription History** — Browse and search past sessions (30-day window)
- **Auto-Updates** — Built-in OTA update mechanism via GitHub Releases
- **Cross-Platform** — Windows and Linux (deb, AppImage)

## Tech Stack

| Layer | Technology |
|---|---|
| Frontend | Svelte 5 (runes) + Vite |
| Desktop Framework | Tauri 2 |
| Backend | Rust |
| Audio Capture | cpal |
| Local Transcription | whisper-rs (whisper.cpp bindings) |
| Build Targets | NSIS (Windows), deb / AppImage (Linux) |

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v22 recommended)
- [Rust](https://www.rust-lang.org/tools/install) (MSRV 1.77.2)
- Tauri 2 system dependencies — see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)

### Install & Run

```bash
# Install frontend dependencies
npm install

# Run the full desktop app (Vite dev server + Tauri)
npx tauri dev

# Or run frontend only (no Rust backend)
npm run dev
```

### Build

```bash
# Production frontend build
npm run build

# Desktop installer (NSIS on Windows, deb/AppImage on Linux)
npx tauri build
```

### Rust Checks

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo check --manifest-path src-tauri/Cargo.toml
```

## Architecture

QuillScribe uses Tauri's two-process model:

- **Frontend process** — Svelte 5 app rendered in a WebView
- **Backend process** — Rust binary handling audio, transcription, and system integration
- **IPC** — Frontend calls Rust via `invoke()` commands; backend pushes updates via Tauri events

### Windows

| Window | Purpose |
|---|---|
| Main (960x560) | Recording, transcription, history, and settings |
| Overlay (floating) | Transparent always-on-top recording timer |

### Backend Modules

| Module | Responsibility |
|---|---|
| `audio.rs` | Microphone capture, device enumeration, WAV encoding |
| `whisper.rs` | Local model management, cloud API calls, transcription |
| `commands.rs` | ~40 IPC command handlers (the full API surface) |
| `config.rs` | JSON settings persistence |
| `output.rs` | Clipboard, simulated typing, paste-tool detection |
| `tray.rs` | System tray icon and context menu |
| `hotkey.rs` | Global keyboard shortcut registration |
| `sound.rs` | Notification sound playback |
| `window.rs` | Window management and overlay control |
| `statistics.rs` | Transcription history storage |

## Transcription Engines

### Local (Whisper)

Models are downloaded on demand and cached locally. Available sizes range from Tiny (~75 MB) to Large, plus distilled variants for faster inference.

### Cloud

| Provider | Models |
|---|---|
| OpenAI | gpt-4o-transcribe, gpt-4o-mini-transcribe |
| Groq | whisper-large-v3-turbo, whisper-large-v3 |

API keys are configured in the settings dialog and validated before use.

## Themes

6 light themes: White, Warm Gray, Soft Beige, Blue Gray, Warm Taupe, Soft Sage

6 dark themes: Dark Charcoal, Dark Blue, Dark Purple, Dark Forest, Dark Burgundy, Obsidian

Themes are applied via CSS custom properties and synchronized across both windows.

## Project Structure

```
quillscribe-frontend/
├── src/
│   ├── App.svelte              # Root component
│   ├── app.css                 # Global styles and CSS custom properties
│   ├── main.js                 # Main window entry
│   ├── overlay.js              # Overlay window entry
│   └── lib/
│       ├── RecordPanel.svelte
│       ├── TranscriptionPanel.svelte
│       ├── HistoryPanel.svelte
│       ├── SettingsDialog.svelte
│       ├── Sidebar.svelte
│       ├── TitleBar.svelte
│       ├── RecordingOverlay.svelte
│       ├── Toast.svelte
│       ├── QuillIcon.svelte
│       ├── themes.js           # Theme definitions
│       └── overlayStyles.js
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/                    # Rust backend modules
├── public/                     # Logos, favicon, icons
└── .github/workflows/          # CI and release pipelines
```

## Branding

| Asset | Path |
|---|---|
| Logo (dark) | `public/logo.svg` |
| Logo (light) | `public/logo-light.svg` |
| Favicon | `public/favicon.svg` |
