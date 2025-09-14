"""
Sound Manager for QuillScribe
Handles playback of notification sounds using Qt's native audio capabilities
"""

import os
from pathlib import Path
from typing import Optional
from PySide6.QtCore import QObject, QUrl
from PySide6.QtMultimedia import QSoundEffect


class SoundManager(QObject):
    """Manages sound playback for recording events using Qt's QSoundEffect"""
    
    def __init__(self):
        super().__init__()
        self.initialized = False
        self.sounds_enabled = True  # Default to enabled
        self.sounds_path = Path(__file__).parent.parent / "sounds"
        
        # Load sounds using Qt's QSoundEffect
        self.start_sound: Optional[QSoundEffect] = None
        self.stop_sound: Optional[QSoundEffect] = None
        self._load_sounds()
    
    def _load_sounds(self):
        """Load sound files using Qt's QSoundEffect"""
        try:
            start_path = self.sounds_path / "start.wav"
            stop_path = self.sounds_path / "stop.wav"
            
            if start_path.exists():
                self.start_sound = QSoundEffect()
                self.start_sound.setSource(QUrl.fromLocalFile(str(start_path.absolute())))
                print(f"Loaded start sound: {start_path}")
            else:
                print(f"Start sound not found: {start_path}")
                
            if stop_path.exists():
                self.stop_sound = QSoundEffect()
                self.stop_sound.setSource(QUrl.fromLocalFile(str(stop_path.absolute())))
                print(f"Loaded stop sound: {stop_path}")
            else:
                print(f"Stop sound not found: {stop_path}")
            
            self.initialized = True
            print("Sound manager initialized successfully")
                
        except Exception as e:
            print(f"Error loading sounds: {e}")
            self.initialized = False
    
    def set_sounds_enabled(self, enabled: bool):
        """Enable or disable sound playback"""
        self.sounds_enabled = bool(enabled)
    
    def play_start_sound(self):
        """Play the recording start sound"""
        if self.sounds_enabled and self.initialized and self.start_sound:
            try:
                self.start_sound.play()
            except Exception as e:
                print(f"Error playing start sound: {e}")
    
    def play_stop_sound(self):
        """Play the recording stop sound"""
        if self.sounds_enabled and self.initialized and self.stop_sound:
            try:
                self.stop_sound.play()
            except Exception as e:
                print(f"Error playing stop sound: {e}")
    
    def cleanup(self):
        """Clean up Qt audio resources"""
        if self.initialized:
            try:
                if self.start_sound:
                    self.start_sound.stop()
                if self.stop_sound:
                    self.stop_sound.stop()
                self.initialized = False
            except Exception as e:
                print(f"Error cleaning up sound manager: {e}")