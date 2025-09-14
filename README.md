# QuillScribe

**Beautiful Voice-to-Text Transcription App**

A minimal, elegant voice-to-text transcription application with a stunning breathing microphone interface. Convert your speech to text with a beautiful, modern UI.

![QuillScribe Demo](https://via.placeholder.com/600x400/4A90E2/FFFFFF?text=QuillScribe+Demo)

## ✨ Features

- **Beautiful Minimal UI** - A clean and focused interface.
- **Stunning Animations** - Dynamic microphone visualization with a breathing animation and an audio-responsive waveform.
- **Dual AI Modes** - Choose between the powerful OpenAI Whisper API or a private, offline local model using `faster-whisper`.
- **Global Hotkeys** - Start and stop recording from anywhere on your desktop (Windows only, `Win+F` by default).
- **Theming** - Customize the look and feel with multiple light and dark themes.
- **Compact Mode** - Switch to a super-compact, always-on-top style UI.
- **Custom Title Bar** - A sleek, custom title bar for a more integrated look (can be disabled).
- **Smart Output** - Copy to clipboard, auto-paste to your active application, or just display the text.
- **Easy Settings** - Modern tabbed settings for audio, transcription, UI, and output.
- **Microphone Control** - Auto-detects active microphone, with options to manually select and test.
- **Silent Mode** - Hide transcription text for privacy.

## Quick Start

### Installation

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/quillscribe/quillscribe.git
    cd quillscribe
    ```

2.  **Install dependencies:**
    ```bash
    pip install -r requirements.txt
    ```

3.  **Run the application:**
    ```bash
    python run.py
    ```

### Alternative: Package Installation

```bash
pip install quillscribe
quillscribe
```

## Setup Guide

### Option 1: OpenAI Whisper API (Recommended)

1.  Get your API key from [OpenAI Platform](https://platform.openai.com/api-keys).
2.  Open QuillScribe → Click **Settings**.
3.  In the **Whisper** tab, select **OpenAI Whisper API**.
4.  Enter your API key.
5.  Click **Save Settings**.

**Pros:** Fast, high-quality, works immediately.
**Cons:** Requires internet, may have usage costs.

### Option 2: Local `faster-whisper` Model

1.  Open QuillScribe → Click **Settings**.
2.  In the **Whisper** tab, select **Local Whisper.cpp (Private, works offline)**.
3.  Use the dropdowns to select a model category and a specific model. Smaller models are faster but less accurate.
4.  The application will handle downloading the model if it's not already present.
5.  Click **Save Settings**.

**Pros:** Private, works offline, no ongoing costs.
**Cons:** Requires model download, can be slower and less accurate than the API depending on the model and your hardware.

## Usage

1.  **Start Recording:** Click the microphone or use the global hotkey (`Win+F` by default on Windows).
2.  **Watch the Magic:** The microphone animates with a live audio waveform.
3.  **Stop Recording:** Click the microphone again or use the hotkey.
4.  **Get Results:** Text is automatically copied/pasted based on your settings.

### Output Modes

-   **Copy Only** - Copies transcription to the clipboard.
-   **Paste Only** - Pastes directly to the active application.
-   **Copy & Paste** - Does both.
-   **Display Only** - Shows text in the app without copying or pasting.

## Requirements

-   **Python 3.9+**
-   **Windows 10/11** (Primary target, with global hotkeys)
-   **Microphone** (Built-in or external)
-   **Internet connection** (For API mode and initial model downloads)

### Dependencies

-   `PySide6` - Modern Qt6 UI framework.
-   `sounddevice` - Real-time audio capture.
-   `numpy` - Audio processing.
-   `openai` - Whisper API integration.
-   `faster-whisper` - Efficient local Whisper model implementation.
-   `pyperclip` - Clipboard operations.

## Troubleshooting

### Audio Issues

-   **No microphone detected:** Check Windows microphone permissions. Ensure the microphone is connected and enabled. Use the "Refresh" and "Test" buttons in the Audio settings. Try the "Auto-select active microphone" feature.
-   **Poor audio quality:** Use the **Test** button in settings to check your audio levels. Reduce background noise and speak closer to the microphone.

### Transcription Issues

-   **API errors:** Verify your API key is correct and that you have a working internet connection.
-   **Local model issues:** Ensure you have enough disk space for the model download. If transcription is slow, try a smaller model (e.g., `tiny` or `base`).

## Project Structure

```
quillscribe/
├── src/quillscribe/
│   ├── __init__.py
│   ├── main.py              # Main application & UI
│   ├── audio_manager.py     # Audio capture & processing
│   ├── whisper_manager.py   # Whisper integration (API & local)
│   ├── settings_dialog.py   # Settings panel
│   ├── config_manager.py    # Configuration handling
│   └── ...
├── requirements.txt
├── setup.py
├── run.py                   # Application launcher
└── README.md
```

## Development

### Running from Source

```bash
git clone https://github.com/quillscribe/quillscribe.git
cd quillscribe
python -m venv venv
source venv/bin/activate  # Windows: venv\Scripts\activate
pip install -r requirements.txt
python run.py
```

### Building Executable

```bash
pip install cx-freeze
python setup.py build
```

## Contributing

We welcome contributions! Please fork the repository, create a feature branch, and submit a pull request.

## License

This project is licensed under the MIT License.

---

**Made with care by the QuillScribe Team**

*Transform your voice into text with elegance and simplicity.*