"""
Configuration Manager for QuillScribe
Handles saving and loading application settings
"""

import json
import os
from typing import Any, Optional
from pathlib import Path


class ConfigManager:
    """Manages application configuration and settings"""
    
    def __init__(self):
        self.config_dir = Path.home() / ".config" / "quillscribe"
        self.config_file = self.config_dir / "settings.json"
        self.settings = {}
        
        # Create config directory if it doesn't exist
        self.config_dir.mkdir(parents=True, exist_ok=True)
        
        # Load existing settings
        self.load_settings()
        
        # Set defaults if first run
        self._set_defaults()
    
    def _set_defaults(self):
        """Set default configuration values"""
        defaults = {
            "audio": {
                "device_id": None,  # Use system default
                "sample_rate": 16000,
                "channels": 1,
                "sounds_enabled": True,  # Enable/disable notification sounds
                "auto_select_mic": False  # Auto-select active microphone (disabled by default)
            },
            "whisper": {
                "mode": "api",  # "api" or "local"
                "api_key": "",
                # Use a valid faster-whisper model name by default
                "local_model": "base",
                "local_model_path": ""
            },
            "output": {
                "mode": 2,  # 0=copy, 1=paste, 2=both, 3=display
                "silent_mode": False,
                "auto_clear": False,
                "auto_clear_delay": 5  # Time in seconds before auto-clearing clipboard
            },
            "ui": {
                "window_x": None,
                "window_y": None,
                "theme": "light",
                "show_waveform": True,
                "compact_mode": False,
                "minimize_on_close": True
            },
            "shortcuts": {
                "record_toggle": "Win+F"
            },
            "advanced": {
                "buffer_size": 1024,
                "noise_threshold": 0.01,
                "silence_timeout": 2.0
            }
        }
        
        # Only set defaults for missing keys
        for category, settings in defaults.items():
            if category not in self.settings:
                self.settings[category] = {}
            
            for key, value in settings.items():
                if key not in self.settings[category]:
                    self.settings[category][key] = value
    
    def get_setting(self, path: str, default: Any = None) -> Any:
        """
        Get a setting value using dot notation
        Example: get_setting("audio.device_id", None)
        """
        keys = path.split("/")
        current = self.settings
        
        try:
            for key in keys:
                current = current[key]
            return current
        except (KeyError, TypeError):
            return default
    
    def set_setting(self, path: str, value: Any):
        """
        Set a setting value using dot notation
        Example: set_setting("audio.device_id", 1)
        """
        keys = path.split("/")
        current = self.settings
        
        # Navigate to the parent of the target key
        for key in keys[:-1]:
            if key not in current:
                current[key] = {}
            current = current[key]
        
        # Set the final value
        current[keys[-1]] = value
    
    def load_settings(self):
        """Load settings from file"""
        try:
            if self.config_file.exists():
                with open(self.config_file, 'r', encoding='utf-8') as f:
                    self.settings = json.load(f)
            else:
                self.settings = {}
        except (json.JSONDecodeError, IOError) as e:
            print(f"Error loading settings: {e}")
            self.settings = {}
    
    def save_settings(self):
        """Save settings to file"""
        try:
            with open(self.config_file, 'w', encoding='utf-8') as f:
                json.dump(self.settings, f, indent=2, ensure_ascii=False)
        except IOError as e:
            print(f"Error saving settings: {e}")
    
    def reset_settings(self, category: Optional[str] = None):
        """Reset settings to defaults"""
        if category:
            # Reset specific category
            if category in self.settings:
                del self.settings[category]
        else:
            # Reset all settings
            self.settings = {}
        
        # Re-apply defaults
        self._set_defaults()
        self.save_settings()
    
    def export_settings(self, file_path: str):
        """Export settings to a file"""
        try:
            with open(file_path, 'w', encoding='utf-8') as f:
                json.dump(self.settings, f, indent=2, ensure_ascii=False)
            return True
        except IOError as e:
            print(f"Error exporting settings: {e}")
            return False
    
    def import_settings(self, file_path: str):
        """Import settings from a file"""
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                imported_settings = json.load(f)
            
            # Validate imported settings structure
            if isinstance(imported_settings, dict):
                self.settings = imported_settings
                self._set_defaults()  # Fill in any missing defaults
                self.save_settings()
                return True
            else:
                print("Invalid settings file format")
                return False
        except (json.JSONDecodeError, IOError) as e:
            print(f"Error importing settings: {e}")
            return False
    
    def get_config_dir(self) -> Path:
        """Get the configuration directory path"""
        return self.config_dir
    
    def get_config_file(self) -> Path:
        """Get the configuration file path"""
        return self.config_file
    
    def get_all_settings(self) -> dict:
        """Get all settings as a dictionary"""
        return self.settings.copy()
    
    def validate_api_key(self, api_key: str) -> bool:
        """Validate OpenAI API key format"""
        return (
            isinstance(api_key, str) and
            api_key.startswith("sk-") and
            len(api_key) > 20
        )
    
    def get_log_file(self) -> Path:
        """Get the log file path"""
        return self.config_dir / "quillscribe.log"
