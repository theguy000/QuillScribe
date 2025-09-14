"""
Setup script for QuillScribe - Beautiful Voice-to-Text Transcription App
"""

from setuptools import setup, find_packages
from pathlib import Path

# Read long description from README
this_directory = Path(__file__).parent
long_description = (this_directory / "README.md").read_text(encoding="utf-8") if (this_directory / "README.md").exists() else ""

setup(
    name="quillscribe",
    version="1.0.0",
    author="QuillScribe Team",
    author_email="support@quillscribe.com",
    description="Beautiful voice-to-text transcription app with minimal, elegant UI",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/quillscribe/quillscribe",
    packages=find_packages("src"),
    package_dir={"": "src"},
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: End Users/Desktop",
        "Topic :: Multimedia :: Sound/Audio :: Speech",
        "Topic :: Office/Business :: Office Suites",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Operating System :: OS Independent",
    ],
    python_requires=">=3.9",
    install_requires=[
        "PySide6>=6.5.0",
        "sounddevice>=0.4.6",
        "numpy>=1.21.0",
        "openai>=1.3.0",
        "faster-whisper>=0.10.0",
        "pyperclip>=1.8.2",
        "requests>=2.31.0",
    ],
    extras_require={
        "dev": [
            "pytest>=7.0.0",
            "black>=23.0.0",
            "flake8>=6.0.0",
        ],
        "build": [
            "cx-freeze>=6.15.0",
        ],
    },
    entry_points={
        "console_scripts": [
            "quillscribe=quillscribe.main:main",
        ],
        "gui_scripts": [
            "quillscribe-gui=quillscribe.main:main",
        ],
    },
    include_package_data=True,
    keywords="speech-to-text voice transcription whisper ai",
    project_urls={
        "Bug Reports": "https://github.com/quillscribe/quillscribe/issues",
        "Source": "https://github.com/quillscribe/quillscribe",
        "Documentation": "https://github.com/quillscribe/quillscribe#readme",
    },
)
