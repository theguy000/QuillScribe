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

# Add src directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'src'))

from quillscribe.main import main

if __name__ == "__main__":
    sys.exit(main())
