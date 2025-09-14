"""
Enhanced build script for QuillScribe using cx_Freeze
Creates a standalone executable with all dependencies
"""

import sys
import os
from cx_Freeze import setup, Executable
from pathlib import Path

# Build options
build_options = {
    'packages': [
        'quillscribe',  # Add the main package
        'PySide6',
        'sounddevice',
        'numpy', 
        'openai',
        'faster_whisper',
        'pyperclip',
        'requests'
    ],
    'includes': [
        'quillscribe.main',
        'quillscribe.audio_manager',
        'quillscribe.whisper_manager', 
        'quillscribe.settings_dialog',
        'quillscribe.config_manager',
        'quillscribe.output_manager',
        'quillscribe.sound_manager',
        'quillscribe.icon_manager',
        'quillscribe.model_manager'
    ],
    'excludes': [
        'tkinter',
        'unittest',
        'email',
        'html',
        'http',
        'xml',
        'pydoc',
        'doctest',
        'argparse',
        'subprocess',
        'multiprocessing'
    ],
    'path': ['src'],  # Add src directory to Python path
    'include_files': [
        # Include all icons
        ('src/quillscribe/icons/', 'icons/'),
        # Include sounds
        ('src/sounds/', 'sounds/'),
        ('src/quillscribe/app_logo.ico', 'app_logo.ico'),
        ('src/quillscribe/logo.png', 'logo.png'),
        # Include any additional assets
    ],
    'zip_include_packages': '*',
    'zip_exclude_packages': [],
    'optimize': 2,
    'build_exe': 'build/QuillScribe',
}

# Create the executable
base = None
if sys.platform == "win32":
    base = "Win32GUI"  # Hide console window

executable = Executable(
    script='run.py',
    base=base,
    target_name='QuillScribe.exe',
    icon='src/quillscribe/app_logo.ico',  # Use ICO format for Windows
    shortcut_name='QuillScribe',
    shortcut_dir='DesktopFolder'
)

setup(
    name='QuillScribe',
    version='1.0.0',
    description='Beautiful Voice-to-Text Transcription App',
    author='QuillScribe Team',
    options={'build_exe': build_options},
    executables=[executable]
)
