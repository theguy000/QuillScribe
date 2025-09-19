"""
Whisper Manager for QuillScribe
Handles both OpenAI Whisper API and prebuilt whisper.cpp from HuggingFace
"""

import numpy as np
from typing import Optional
from PySide6.QtCore import QObject, Signal, QThread
import tempfile
import os
import wave

# Try importing faster-whisper (stable C++ implementation)
try:
    from faster_whisper import WhisperModel
    FASTER_WHISPER_AVAILABLE = True
    WHISPER_TYPE = "faster"
    print("Faster-Whisper available for local processing")
except ImportError as e:
    FASTER_WHISPER_AVAILABLE = False
    WHISPER_TYPE = None
    print(f"Faster-Whisper not available ({e}), only API mode will work")

# Try importing OpenAI
try:
    import openai
    OPENAI_AVAILABLE = True
except ImportError:
    OPENAI_AVAILABLE = False
    print("OpenAI package not available, only local mode will work")


class TranscriptionWorker(QThread):
    """Worker thread for handling transcription to avoid blocking UI"""
    
    transcription_complete = Signal(str)
    transcription_error = Signal(str)
    
    def __init__(self, whisper_manager, audio_data: np.ndarray):
        super().__init__()
        self.whisper_manager = whisper_manager
        self.audio_data = audio_data
    
    def run(self):
        """Run transcription in background thread"""
        try:
            result = self.whisper_manager._transcribe_sync(self.audio_data)
            if result:
                self.transcription_complete.emit(result)
            else:
                self.transcription_error.emit("Transcription returned empty result")
        except Exception as e:
            self.transcription_error.emit(str(e))


class WhisperManager(QObject):
    """Manages Whisper transcription using API or local whisper.cpp"""
    
    transcription_ready = Signal(str)
    transcription_error = Signal(str)
    model_loading = Signal(str)  # Progress message
    
    def __init__(self):
        super().__init__()
        self.mode = "api"  # "api" or "local"
        self.api_key = ""
        self.local_model = None
        self.local_model_path = ""
        self.is_loading = False
        
        # Available models for local mode (faster-whisper format)
        self.available_local_models = [
            "tiny",
            "tiny.en",
            "base", 
            "base.en",
            "small",
            "small.en",
            "medium",
            "medium.en",
            "large-v1",
            "large-v2", 
            "large-v3",
            "large-v3-turbo",
            "turbo",
            # Distilled models (only these are supported by faster-whisper)
            "distil-small.en",
            "distil-medium.en",
            "distil-large-v2",
            "distil-large-v3",
            "distil-large-v3.5"
        ]
        
        self.current_worker = None
    
    def set_mode(self, mode: str):
        """Set transcription mode: 'api' or 'local'"""
        if mode in ["api", "local"]:
            self.mode = mode
        else:
            raise ValueError("Mode must be 'api' or 'local'")
    
    def set_api_key(self, api_key: str):
        """Set OpenAI API key"""
        self.api_key = api_key
        if OPENAI_AVAILABLE:
            openai.api_key = api_key
    
    def set_local_model(self, model_name: str):
        """Set local faster-whisper model name"""
        # Normalize legacy ggml filenames to faster-whisper canonical names
        legacy_map = {
            "ggml-tiny.bin": "tiny",
            "ggml-tiny.en.bin": "tiny.en",
            "ggml-base.bin": "base",
            "ggml-base.en.bin": "base.en",
            "ggml-small.bin": "small",
            "ggml-small.en.bin": "small.en",
            "ggml-medium.bin": "medium",
            "ggml-medium.en.bin": "medium.en",
            # historical large names map to proper versions
            "ggml-large.bin": "large-v2",
            "ggml-large-v1.bin": "large-v1",
            "ggml-large-v2.bin": "large-v2",
            "ggml-large-v3.bin": "large-v3",
        }
        normalized = legacy_map.get(model_name.strip(), model_name.strip())
        self.local_model_name = normalized
        # Reset model to force reload
        self.local_model = None
    
    
    def load_local_model(self):
        """Load local whisper model"""
        if not FASTER_WHISPER_AVAILABLE:
            raise RuntimeError("No local Whisper available")
        
        if self.is_loading:
            return
            
        if not self.local_model_name:
            raise ValueError("No model name specified")
        
        try:
            self.is_loading = True
            self.model_loading.emit(f"Loading model: {self.local_model_name}")
            
            # Load the model using faster-whisper
            if WHISPER_TYPE == "faster":
                # Choose device (CPU for compatibility, GPU if available)
                device = "cpu"  # Can be changed to "cuda" if GPU available
                compute_type = "int8"  # Good balance of speed and quality
                
                self.local_model = WhisperModel(
                    self.local_model_name, 
                    device=device,
                    compute_type=compute_type
                )
            else:
                raise RuntimeError("No supported local Whisper implementation available")
            
            self.model_loading.emit("Model loaded successfully")
            
        except Exception as e:
            self.model_loading.emit(f"Error loading model: {str(e)}")
            raise
        finally:
            self.is_loading = False
    
    
    def is_ready(self) -> bool:
        """Check if the manager is ready for transcription"""
        if self.mode == "api":
            return OPENAI_AVAILABLE and bool(self.api_key)
        else:
            # For local mode, check if we have faster-whisper available and a model name
            return (FASTER_WHISPER_AVAILABLE and 
                    bool(self.local_model_name))
    
    def transcribe_audio(self, audio_data: np.ndarray):
        """Transcribe audio data (async)"""
        if not self.is_ready():
            if self.mode == "api":
                error_msg = "API mode not ready. Check API key."
            else:
                # More detailed error message for local mode
                if not FASTER_WHISPER_AVAILABLE:
                    error_msg = "Faster-Whisper not available. Please install faster-whisper package or use API mode."
                elif not self.local_model_name:
                    error_msg = "No local model selected. Please select a model in settings."
                else:
                    error_msg = "Local mode not ready. Unknown error."
                    
            print(f"Transcription error details: {error_msg}")
            self.transcription_error.emit(error_msg)
            return
        
        # Cancel previous transcription if running
        if self.current_worker and self.current_worker.isRunning():
            self.current_worker.quit()
            self.current_worker.wait()
        
        # Start new transcription worker
        self.current_worker = TranscriptionWorker(self, audio_data)
        self.current_worker.transcription_complete.connect(self.transcription_ready.emit)
        self.current_worker.transcription_error.connect(self.transcription_error.emit)
        self.current_worker.start()
    
    def _transcribe_sync(self, audio_data: np.ndarray) -> Optional[str]:
        """Synchronous transcription (runs in worker thread)"""
        if self.mode == "api":
            return self._transcribe_api(audio_data)
        else:
            return self._transcribe_local(audio_data)
    
    def _transcribe_api(self, audio_data: np.ndarray) -> Optional[str]:
        """Transcribe using OpenAI API"""
        if not OPENAI_AVAILABLE:
            raise RuntimeError("OpenAI package not available")
        
        try:
            # Save audio to temporary WAV file
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as temp_file:
                # Convert float32 to int16
                audio_int16 = (audio_data * 32767).astype(np.int16)
                
                # Write WAV file
                with wave.open(temp_file.name, 'wb') as wav_file:
                    wav_file.setnchannels(1)  # Mono
                    wav_file.setsampwidth(2)  # 16-bit
                    wav_file.setframerate(16000)  # 16kHz
                    wav_file.writeframes(audio_int16.tobytes())
                
                temp_path = temp_file.name
            
            # Transcribe using OpenAI API
            client = openai.OpenAI(api_key=self.api_key)
            
            with open(temp_path, "rb") as audio_file:
                transcript = client.audio.transcriptions.create(
                    model="whisper-1",
                    file=audio_file,
                    response_format="text"
                )
            
            # Clean up temporary file
            os.unlink(temp_path)
            
            return transcript.strip()
            
        except Exception as e:
            # Clean up temp file if it exists
            if 'temp_path' in locals() and os.path.exists(temp_path):
                os.unlink(temp_path)
            raise e
    
    def _transcribe_local(self, audio_data: np.ndarray) -> Optional[str]:
        """Transcribe using local faster-whisper"""
        if not FASTER_WHISPER_AVAILABLE:
            raise RuntimeError("No local Whisper available")
        
        # Load model if not already loaded
        if self.local_model is None and self.local_model_name:
            self.load_local_model()
        
        if self.local_model is None:
            raise RuntimeError("No local model loaded")
        
        try:
            # Ensure audio is in the right format (float32, 16kHz, mono)
            if audio_data.dtype != np.float32:
                audio_data = audio_data.astype(np.float32)
            
            # Transcribe using faster-whisper
            if WHISPER_TYPE == "faster":
                segments, info = self.local_model.transcribe(
                    audio_data,
                    beam_size=5,
                    language=None,  # Auto-detect language
                    condition_on_previous_text=False
                )
                
                # Combine all segments into one text
                transcription = ""
                for segment in segments:
                    transcription += segment.text
                
                return transcription.strip()
            else:
                raise RuntimeError("No supported local Whisper implementation available")
                
        except Exception as e:
            raise e
    
    
    def get_available_models(self):
        """Get list of available models"""
        if self.mode == "api":
            return ["whisper-1"]  # OpenAI API model
        else:
            # For faster-whisper, all models are available on-demand
            return self.available_local_models
    
    def get_model_categories(self):
        """Get available model categories for filtering"""
        return ["All", "Tiny", "Base", "Small", "Medium", "Large", "Distilled"]
    
    def get_models_by_category(self, category: str):
        """Get LOCAL models filtered by category (never returns API models like whisper-1)"""
        # Always use local models list, never include API models
        local_models = self.available_local_models.copy()
        
        if category == "All":
            return local_models
        elif category == "Tiny":
            return [model for model in local_models if "tiny" in model]
        elif category == "Base":
            return [model for model in local_models if "base" in model]
        elif category == "Small":
            return [model for model in local_models if "small" in model]
        elif category == "Medium":
            return [model for model in local_models if "medium" in model]
        elif category == "Large":
            return [model for model in local_models if "large" in model or model == "turbo"]
        elif category == "Distilled":
            return [model for model in local_models if "distil" in model]
        else:
            return []
    
    def get_model_info(self, model_name: str) -> dict:
        """Get information about a specific model"""
        model_info = {
            "whisper-1": {"size": "API", "memory": "N/A", "speed": "Fast", "quality": "High"},
            "tiny": {"size": "37 MB", "memory": "~200 MB RAM", "speed": "Very Fast", "quality": "Low"},
            "tiny.en": {"size": "37 MB", "memory": "~200 MB RAM", "speed": "Very Fast", "quality": "Low (English only)"},
            "base": {"size": "113 MB", "memory": "~500 MB RAM", "speed": "Fast", "quality": "Medium"},
            "base.en": {"size": "113 MB", "memory": "~500 MB RAM", "speed": "Fast", "quality": "Medium (English only)"},
            "small": {"size": "340 MB", "memory": "~1.2 GB RAM", "speed": "Medium", "quality": "Good"},
            "small.en": {"size": "340 MB", "memory": "~1.2 GB RAM", "speed": "Medium", "quality": "Good (English only)"},
            "medium": {"size": "1.1 GB", "memory": "~3.0 GB RAM", "speed": "Slow", "quality": "High"},
            "medium.en": {"size": "1.1 GB", "memory": "~3.0 GB RAM", "speed": "Slow", "quality": "High (English only)"},
            "large-v1": {"size": "2.3 GB", "memory": "~5.2 GB RAM", "speed": "Very Slow", "quality": "Highest"},
            "large-v2": {"size": "2.3 GB", "memory": "~5.2 GB RAM", "speed": "Very Slow", "quality": "Highest"},
            "large-v3": {"size": "2.3 GB", "memory": "~5.2 GB RAM", "speed": "Very Slow", "quality": "Highest"},
            "large-v3-turbo": {"size": "2.3 GB", "memory": "~5.0 GB RAM", "speed": "Fast", "quality": "Highest"},
            "turbo": {"size": "2.3 GB", "memory": "~5.0 GB RAM", "speed": "Very Fast", "quality": "Highest"},
            # Distilled models (only supported ones)
            "distil-small.en": {"size": "240 MB", "memory": "~900 MB RAM", "speed": "Medium", "quality": "Good (English only)"},
            "distil-medium.en": {"size": "800 MB", "memory": "~2.5 GB RAM", "speed": "Medium", "quality": "High (English only)"},
            "distil-large-v2": {"size": "1.5 GB", "memory": "~3.5 GB RAM", "speed": "Medium", "quality": "Highest"},
            "distil-large-v3": {"size": "1.5 GB", "memory": "~3.5 GB RAM", "speed": "Medium", "quality": "Highest"},
            "distil-large-v3.5": {"size": "1.5 GB", "memory": "~3.5 GB RAM", "speed": "Medium", "quality": "Highest"},
        }
        
        return model_info.get(model_name, {"size": "Unknown", "memory": "Unknown", "speed": "Unknown", "quality": "Unknown"})
