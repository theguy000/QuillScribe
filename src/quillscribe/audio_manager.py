"""
Audio Manager for QuillScribe
Handles real-time audio capture and level monitoring
"""

import numpy as np
import sounddevice as sd
from typing import Optional, List
from PySide6.QtCore import QObject, Signal, QTimer
import threading
import queue


class AudioManager(QObject):
    """Manages audio input and real-time level monitoring"""
    
    audio_level_changed = Signal(float)
    transcription_ready = Signal(str)
    
    def __init__(self, sample_rate: int = 16000, channels: int = 1):
        super().__init__()
        self.sample_rate = sample_rate
        self.channels = channels
        self.is_recording = False
        self.audio_buffer = []
        self.audio_queue = queue.Queue()
        
        # Level monitoring
        self.level_timer = QTimer()
        self.level_timer.timeout.connect(self.update_audio_level)
        self.level_timer.start(50)  # Update every 50ms
        
        # Current audio level
        self.current_level = 0.0
        self.level_smoothing = 0.7  # Smoothing factor for level updates
        
        # Get available devices
        self.update_available_devices()
        
    def update_available_devices(self):
        """Update list of available audio input devices"""
        try:
            devices = sd.query_devices()
            self.available_devices = []
            
            for i, device in enumerate(devices):
                if device['max_input_channels'] > 0:
                    self.available_devices.append({
                        'id': i,
                        'name': device['name'],
                        'channels': device['max_input_channels'],
                        'sample_rate': device['default_samplerate']
                    })
        except Exception as e:
            print(f"Error querying audio devices: {e}")
            self.available_devices = []
    
    def get_available_devices(self) -> List[dict]:
        """Get list of available audio input devices"""
        return self.available_devices
    
    def set_input_device(self, device_id: Optional[int] = None):
        """Set the audio input device"""
        try:
            if device_id is not None:
                sd.default.device[0] = device_id  # Input device
            else:
                sd.default.device[0] = None  # Use system default
                
            # If currently monitoring, restart with new device
            if hasattr(self, 'stream') and self.stream.active:
                self.stop_monitoring()
                self.start_monitoring()
        except Exception as e:
            print(f"Error setting audio device: {e}")
    
    def audio_callback(self, indata, frames, time, status):
        """Callback for audio stream"""
        if status:
            print(f"Audio callback status: {status}")
        
        # Always calculate level for real-time monitoring
        if indata is not None and len(indata) > 0:
            # Calculate RMS level for visualization
            rms = np.sqrt(np.mean(indata.flatten() ** 2))
            
            # Smooth the level changes
            self.current_level = (self.level_smoothing * self.current_level + 
                                (1 - self.level_smoothing) * rms)
        
        if self.is_recording:
            # Add to buffer
            audio_data = indata.copy()
            self.audio_buffer.append(audio_data)
    
    def update_audio_level(self):
        """Update audio level signal for UI"""
        # Normalize level (typical speech is around 0.01-0.1 RMS)
        # Increased sensitivity for better visualization
        normalized_level = min(1.0, self.current_level * 20)
        
        # Debug output can be enabled for troubleshooting
        # if normalized_level > 0.01:
        #     print(f"Audio level: raw={self.current_level:.6f}, normalized={normalized_level:.3f}")
        
        self.audio_level_changed.emit(normalized_level)
    
    def start_monitoring(self):
        """Start audio level monitoring (without recording)"""
        if hasattr(self, 'stream') and self.stream.active:
            return
            
        try:
            # Start audio stream for monitoring only
            self.stream = sd.InputStream(
                callback=self.audio_callback,
                channels=self.channels,
                samplerate=self.sample_rate,
                dtype=np.float32,
                blocksize=1024
            )
            self.stream.start()
            
        except Exception as e:
            print(f"Error starting audio monitoring: {e}")
            # Try to get device info for debugging
            try:
                device_info = sd.query_devices(kind='input')
                print(f"Default input device: {device_info}")
            except:
                pass
            raise
    
    def start_recording(self):
        """Start audio recording"""
        if self.is_recording:
            return
            
        try:
            # Make sure monitoring is active
            self.start_monitoring()
            
            # Now start recording
            self.is_recording = True
            self.audio_buffer = []
            
        except Exception as e:
            print(f"Error starting recording: {e}")
            self.is_recording = False
            raise
    
    def stop_recording(self) -> Optional[np.ndarray]:
        """Stop audio recording and return recorded data (keep monitoring)"""
        if not self.is_recording:
            return None
            
        try:
            self.is_recording = False
            
            # Combine audio buffer
            audio_data = None
            if self.audio_buffer:
                audio_data = np.concatenate(self.audio_buffer, axis=0)
                # Convert to mono if needed
                if audio_data.ndim > 1:
                    audio_data = np.mean(audio_data, axis=1)
            
            # Clear buffer but keep monitoring active for animation
            self.audio_buffer = []
            
            return audio_data
            
        except Exception as e:
            print(f"Error stopping recording: {e}")
            return None
    
    def stop_monitoring(self):
        """Stop audio monitoring completely"""
        try:
            self.is_recording = False
            
            # Stop and close stream
            if hasattr(self, 'stream'):
                self.stream.stop()
                self.stream.close()
            
            # Reset level
            self.current_level = 0.0
            self.audio_level_changed.emit(0.0)
            
        except Exception as e:
            print(f"Error stopping monitoring: {e}")
    
    def test_microphone(self, device_id: Optional[int] = None) -> bool:
        """Test if microphone is working"""
        try:
            # Temporary test recording
            test_duration = 0.5  # seconds
            
            with sd.InputStream(
                device=device_id,
                channels=self.channels,
                samplerate=self.sample_rate,
                dtype=np.float32
            ) as stream:
                data, _ = stream.read(int(test_duration * self.sample_rate))
                
            # Check if we got any reasonable audio data
            rms = np.sqrt(np.mean(data ** 2))
            return rms > 0.0001  # Very low threshold for detection
            
        except Exception as e:
            print(f"Microphone test failed: {e}")
            return False
    
    def get_default_device_info(self) -> dict:
        """Get information about the default audio device"""
        try:
            device_info = sd.query_devices(kind='input')
            return {
                'name': device_info['name'],
                'channels': device_info['max_input_channels'],
                'sample_rate': device_info['default_samplerate'],
                'id': sd.default.device[0]
            }
        except Exception as e:
            print(f"Error getting default device info: {e}")
            return {
                'name': 'Unknown',
                'channels': 1,
                'sample_rate': 16000,
                'id': None
            }
    
    def get_active_device_id(self) -> Optional[int]:
        """Get the ID of the currently active input device with audio activity"""
        try:
            # First try to detect device with actual audio activity
            active_device_with_audio = self._detect_device_with_audio()
            if active_device_with_audio is not None:
                return active_device_with_audio
            
            # Fallback to system default device
            default_device = sd.query_devices(kind='input')
            if default_device:
                # Find the device ID that matches the default device name
                for device in self.available_devices:
                    if device['name'] == default_device['name']:
                        return device['id']
                        
                # If not found in our list, try to get directly from sounddevice
                all_devices = sd.query_devices()
                for i, device in enumerate(all_devices):
                    if (device['max_input_channels'] > 0 and 
                        device['name'] == default_device['name']):
                        return i
            return None
        except Exception as e:
            print(f"Error getting active device ID: {e}")
            return None
    
    def _detect_device_with_audio(self) -> Optional[int]:
        """Detect which microphone device currently has audio activity"""
        try:
            devices_with_audio = []
            
            # Test each available input device for audio activity
            for device in self.available_devices:
                device_id = device['id']
                try:
                    # Quick audio test - record for 0.3 seconds
                    with sd.InputStream(
                        device=device_id,
                        channels=1,
                        samplerate=16000,
                        dtype=np.float32,
                        blocksize=1024
                    ) as stream:
                        # Read a small amount of audio data
                        audio_data, _ = stream.read(int(0.3 * 16000))
                        
                        # Calculate RMS level
                        if len(audio_data) > 0:
                            rms = np.sqrt(np.mean(audio_data.flatten() ** 2))
                            # If there's significant audio activity (above noise floor)
                            if rms > 0.001:  # Threshold for detecting actual audio
                                devices_with_audio.append((device_id, rms, device['name']))
                                print(f"Device {device['name']} has audio activity: {rms:.6f}")
                        
                except Exception as e:
                    # Device might be in use or not accessible
                    print(f"Could not test device {device['name']}: {e}")
                    continue
            
            # Return the device with the highest audio activity
            if devices_with_audio:
                # Sort by RMS level (highest first)
                devices_with_audio.sort(key=lambda x: x[1], reverse=True)
                active_device_id = devices_with_audio[0][0]
                active_device_name = devices_with_audio[0][2]
                print(f"Auto-detected active microphone: {active_device_name}")
                return active_device_id
                
            return None
            
        except Exception as e:
            print(f"Error detecting device with audio: {e}")
            return None