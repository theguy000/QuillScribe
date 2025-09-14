#!/usr/bin/env python3
"""
QuillScribe Application Runner
Launch the beautiful voice-to-text transcription app
"""

import sys
import os
import warnings

# Suppress the pkg_resources deprecation warning from ctranslate2
warnings.filterwarnings("ignore", message="pkg_resources is deprecated", category=UserWarning)

# Handle both development and frozen executable environments
if getattr(sys, 'frozen', False):
    # Running as frozen executable (PyInstaller)
    # Add the directory containing the executable to path (append to avoid shadowing)
    app_dir = os.path.dirname(sys.executable)
    sys.path.append(app_dir)
else:
    # Running from source
    # Add src directory to path (append to avoid shadowing standard libraries)
    sys.path.append(os.path.join(os.path.dirname(__file__), 'src'))

from quillscribe.main import main

if __name__ == "__main__":
    sys.exit(main())
