<p align="center">
  <img src="public/logo-light.svg" alt="QuillScribe" width="280" />
</p>

<p align="center">
  <strong>Voice-powered transcription — local or cloud, always private.</strong>
</p>

<p align="center">
  <a href="https://github.com/theguy000/QuillScribe/releases/latest"><img src="https://img.shields.io/github/v/release/theguy000/QuillScribe?label=latest" alt="Release" /></a>
  <img src="https://img.shields.io/github/license/theguy000/QuillScribe" alt="License" />
  <img src="https://img.shields.io/github/actions/workflow/status/theguy000/QuillScribe/release.yml?label=build" alt="Build" />
</p>

---

## Install

**Linux (one-click):**

```bash
curl -fsSL https://raw.githubusercontent.com/theguy000/QuillScribe/main/install.sh | bash
```

The installer uses the native Linux tarball by default and writes install metadata so QuillScribe can safely perform in-app updates. If the tarball is unavailable, it falls back to the AppImage.

To force a specific Linux package format:

```bash
curl -fsSL https://raw.githubusercontent.com/theguy000/QuillScribe/main/install.sh | QUILLSCRIBE_INSTALL_FORMAT=appimage bash
curl -fsSL https://raw.githubusercontent.com/theguy000/QuillScribe/main/install.sh | QUILLSCRIBE_INSTALL_FORMAT=tarball bash
```

**Windows:** Download the recommended `windows-x86_64-setup.exe` installer from the [latest release](https://github.com/theguy000/QuillScribe/releases/latest). A portable ZIP is also available.

---

## Features

- **Local & cloud transcription** — Whisper models offline, or OpenAI / Groq APIs
- **Global hotkey** — Record from any application
- **Flexible output** — Clipboard, auto-type, or both
- **Floating overlay** — Always-on-top recording timer
- **System tray** — Background mode with quick-access menu
- **History** — Browse and search past transcriptions
- **12 themes** — 6 light + 6 dark
- **Auto-updates** — Seamless OTA for installer-managed Linux tarball installs and supported desktop platforms

## Tech Stack

Slint · Rust · whisper.cpp · cpal

## Development

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) ≥ 1.77.2
- System deps: `libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libgl1-mesa-dev` (Linux)
- CMake & Clang (for whisper.cpp build)

### Quick Start

```bash
cargo run
```

### Build

```bash
cargo build --release
```

### Lint

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## Architecture

Slint UI rendered via FemtoVG, Rust backend for audio, transcription, and system integration. Direct Rust callbacks (no IPC layer).

TODO: If QuillScribe becomes popular enough to justify the maintenance cost, reduce idle memory usage by splitting the always-running background service from the on-demand GUI and lazy-loading heavier audio, transcription, and networking components.

| Module | Role |
|---|---|
| `audio.rs` | Microphone capture, device enumeration |
| `whisper.rs` | Model management, transcription |
| `commands.rs` | Command handlers |
| `config.rs` | Settings persistence |
| `output.rs` | Clipboard & auto-type |
| `tray.rs` | System tray |
| `hotkey.rs` | Global shortcuts |
| `window.rs` | Window & overlay management |

## License

MIT
