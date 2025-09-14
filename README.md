# QuillScribe

**Beautiful Voice-to-Text Transcription App**

A minimal, elegant voice-to-text transcription application with a stunning breathing microphone interface. Convert your speech to text with just 2 buttons and a beautiful, modern UI.

![QuillScribe Demo](https://via.placeholder.com/600x400/4A90E2/FFFFFF?text=QuillScribe+Demo)

## ✨ Features

- **Beautiful Minimal UI** - Just 2 buttons: Record/Stop and Settings
- **Breathing Animation** - Dynamic microphone visualization with audio-responsive animation  
- **Dual AI Modes** - Choose between OpenAI Whisper API or local Whisper.cpp
- **Smart Output** - Copy to clipboard, auto-paste, or display only
- **Easy Settings** - Modern tabbed settings for audio, transcription, and output
- **Silent Mode** - Hidden transcription for privacy
- **Real-time Processing** - Low latency speech recognition

## Quick Start

### Installation

1. **Clone the repository:**
   ```bash
   git clone https://github.com/quillscribe/quillscribe.git
   cd quillscribe
   ```

2. **Install dependencies:**
   ```bash
   pip install -r requirements.txt
   ```

3. **Run the application:**
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

1. Get your API key from [OpenAI Platform](https://platform.openai.com/api-keys)
2. Open QuillScribe → Click **Settings**
3. Select **OpenAI Whisper API** 
4. Enter your API key
5. Click **Save Settings**

**Pros:** Fast, high-quality, works immediately  
**Cons:** Requires internet, usage costs

### Option 2: Local Whisper.cpp

1. Download Whisper models from HuggingFace
2. Open QuillScribe → Click **Settings**
3. Select **Local Whisper.cpp**
4. Choose your model (tiny/base/small/medium/large)
5. Click **Load Model**

**Pros:** Private, offline, no costs  
**Cons:** Requires model download, slower processing

## Usage

1. **Start Recording:** Click the microphone or use the configured shortcut
2. **Watch the Magic:** The microphone shows audio waveform animation
3. **Stop Recording:** Click the microphone again or use the shortcut
4. **Get Results:** Text is automatically copied/pasted based on your settings

### Output Modes

- **Copy Only** - Copies transcription to clipboard
- **Paste Only** - Pastes directly to active application  
- **Copy & Paste** - Both copy and paste
- **Display Only** - Shows text without copying

## UI Overview

```
┌─────────────────────────────────────┐
│            QuillScribe              │
│                                     │
│              ●◐◑◒                  │
│           (Animated Mic)            │
│                                     │
│   [Click to Record]   [Settings]    │
│                                     │
│         Ready to transcribe         │
└─────────────────────────────────────┘
```

## Requirements

- **Python 3.9+**
- **Windows 10/11** (Primary target)
- **Microphone** (Built-in or external)
- **Internet connection** (For API mode)

### Dependencies

- `PySide6` - Modern Qt6 UI framework
- `sounddevice` - Real-time audio capture
- `numpy` - Audio processing
- `openai` - Whisper API integration
- `whispercpp` - Local Whisper.cpp support
- `pyperclip` - Clipboard operations

## Troubleshooting

### Audio Issues

**No microphone detected:**
1. Check Windows microphone permissions
2. Ensure microphone is connected and enabled
3. Try selecting a different device in Settings

**Poor audio quality:**
1. Test microphone with **Test Microphone** button
2. Reduce background noise
3. Speak closer to microphone

### Transcription Issues

**API errors:**
1. Verify API key is correct
2. Check internet connection
3. Ensure sufficient API credits

**Local model issues:**
1. Download the correct model format
2. Ensure sufficient disk space
3. Try a smaller model (tiny/base)

## Project Structure

```
quillscribe/
├── src/quillscribe/
│   ├── __init__.py
│   ├── main.py              # Main application & UI
│   ├── audio_manager.py     # Audio capture & processing
│   ├── whisper_manager.py   # Whisper integration
│   ├── settings_dialog.py   # Settings panel
│   └── config_manager.py    # Configuration handling
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

We welcome contributions! Please see our contributing guidelines:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Links

- **Bug Reports:** [GitHub Issues](https://github.com/quillscribe/quillscribe/issues)
- **Documentation:** [GitHub Wiki](https://github.com/quillscribe/quillscribe/wiki)
- **Discussions:** [GitHub Discussions](https://github.com/quillscribe/quillscribe/discussions)

## Acknowledgments

- **OpenAI** for the Whisper model and API
- **Whisper.cpp** community for the efficient C++ implementation
- **Qt/PySide6** for the beautiful UI framework
- All contributors and users of QuillScribe

---

**Made with care by the QuillScribe Team**

*Transform your voice into text with elegance and simplicity.*
