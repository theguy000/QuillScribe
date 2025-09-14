"""
Model Manager for QuillScribe
Handles downloading and managing Whisper models from HuggingFace
"""

import os
import requests
from pathlib import Path
from typing import Dict, List, Optional, Callable
from PySide6.QtCore import QObject, Signal, QThread


class ModelDownloader(QThread):
    """Background thread for downloading models"""
    
    progress_updated = Signal(int)  # Progress percentage
    status_updated = Signal(str)    # Status message
    download_complete = Signal(str) # Model name when complete
    download_error = Signal(str)    # Error message
    
    def __init__(self, model_name: str, model_url: str, save_path: str):
        super().__init__()
        self.model_name = model_name
        self.model_url = model_url
        self.save_path = save_path
        self.cancelled = False
    
    def cancel_download(self):
        """Cancel the download"""
        self.cancelled = True
    
    def run(self):
        """Download the model"""
        try:
            self.status_updated.emit(f"Starting download: {self.model_name}")
            
            # Create directory if it doesn't exist
            os.makedirs(os.path.dirname(self.save_path), exist_ok=True)
            
            # Download with progress tracking (with timeout to prevent hanging)
            response = requests.get(self.model_url, stream=True, timeout=30)
            response.raise_for_status()
            
            total_size = int(response.headers.get('content-length', 0))
            downloaded = 0
            
            with open(self.save_path, 'wb') as f:
                chunk_count = 0
                for chunk in response.iter_content(chunk_size=8192):
                    if self.cancelled:
                        f.close()
                        os.remove(self.save_path)
                        self.status_updated.emit("Download cancelled")
                        return
                    
                    if chunk:
                        f.write(chunk)
                        downloaded += len(chunk)
                        chunk_count += 1
                        
                        # Update progress every 10 chunks to avoid UI spam
                        if total_size > 0 and chunk_count % 10 == 0:
                            progress = int((downloaded / total_size) * 100)
                            self.progress_updated.emit(progress)
                            
                            # Update status with human-readable sizes
                            downloaded_mb = downloaded / (1024 * 1024)
                            total_mb = total_size / (1024 * 1024)
                            self.status_updated.emit(
                                f"Downloading {self.model_name}: {downloaded_mb:.1f}MB / {total_mb:.1f}MB"
                            )
                            
                            # Allow other threads to process
                            self.msleep(1)
            
            self.status_updated.emit(f"Download complete: {self.model_name}")
            self.download_complete.emit(self.model_name)
            
        except Exception as e:
            # Clean up partial download
            if os.path.exists(self.save_path):
                os.remove(self.save_path)
            self.download_error.emit(f"Download failed: {str(e)}")


class ModelManager(QObject):
    """Manages Whisper model downloads and availability"""
    
    model_status_changed = Signal(str, str)  # model_name, status
    download_progress = Signal(str, int)     # model_name, progress
    
    def __init__(self, config_manager):
        super().__init__()
        self.config_manager = config_manager
        self.models_dir = config_manager.get_model_cache_dir()
        self.current_downloads = {}  # model_name -> ModelDownloader
        
        # HuggingFace model URLs (these are example URLs - in production use proper HF API)
        self.model_urls = {
            "ggml-tiny.bin": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            "ggml-base.bin": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            "ggml-small.bin": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            "ggml-medium.bin": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            "ggml-large-v3.bin": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
        }
        
        # Model sizes for progress estimation
        self.model_sizes = {
            "ggml-tiny.bin": 37 * 1024 * 1024,      # 37MB
            "ggml-base.bin": 113 * 1024 * 1024,     # 113MB  
            "ggml-small.bin": 340 * 1024 * 1024,    # 340MB
            "ggml-medium.bin": 1100 * 1024 * 1024,  # 1.1GB
            "ggml-large-v3.bin": 2300 * 1024 * 1024 # 2.3GB
        }
    
    def get_model_path(self, model_name: str) -> str:
        """Get the full path for a model"""
        return str(self.models_dir / model_name)
    
    def is_model_available(self, model_name: str) -> bool:
        """Check if a model is downloaded and available"""
        model_path = self.get_model_path(model_name)
        return os.path.exists(model_path) and os.path.getsize(model_path) > 0
    
    def get_model_status(self, model_name: str) -> str:
        """Get the status of a model"""
        if model_name in self.current_downloads:
            return "downloading"
        elif self.is_model_available(model_name):
            return "available"
        else:
            return "not_downloaded"
    
    def get_available_models(self) -> List[Dict[str, str]]:
        """Get list of all models with their status"""
        models = []
        
        model_info = [
            ("ggml-tiny.bin", "Tiny - 37MB (Fastest)"),
            ("ggml-base.bin", "Base - 113MB (Good balance)"),
            ("ggml-small.bin", "Small - 340MB (Better quality)"),
            ("ggml-medium.bin", "Medium - 1.1GB (High quality)"),
            ("ggml-large-v3.bin", "Large - 2.3GB (Best quality)")
        ]
        
        for model_file, display_name in model_info:
            status = self.get_model_status(model_file)
            models.append({
                "file": model_file,
                "name": display_name,
                "status": status,
                "size": self.model_sizes.get(model_file, 0)
            })
        
        return models
    
    def start_download(self, model_name: str) -> bool:
        """Start downloading a model"""
        if model_name in self.current_downloads:
            return False  # Already downloading
        
        if self.is_model_available(model_name):
            return False  # Already available
        
        if model_name not in self.model_urls:
            return False  # Unknown model
        
        # Create downloader
        model_url = self.model_urls[model_name]
        save_path = self.get_model_path(model_name)
        
        downloader = ModelDownloader(model_name, model_url, save_path)
        
        # Connect signals
        downloader.progress_updated.connect(
            lambda progress: self.download_progress.emit(model_name, progress)
        )
        downloader.status_updated.connect(
            lambda status: self.model_status_changed.emit(model_name, status)
        )
        downloader.download_complete.connect(self.on_download_complete)
        downloader.download_error.connect(self.on_download_error)
        
        # Start download
        self.current_downloads[model_name] = downloader
        downloader.start()
        
        self.model_status_changed.emit(model_name, "downloading")
        return True
    
    def cancel_download(self, model_name: str):
        """Cancel a model download"""
        if model_name in self.current_downloads:
            downloader = self.current_downloads[model_name]
            downloader.cancel_download()
            downloader.wait()  # Wait for thread to finish
            del self.current_downloads[model_name]
            self.model_status_changed.emit(model_name, "not_downloaded")
    
    def on_download_complete(self, model_name: str):
        """Handle download completion"""
        if model_name in self.current_downloads:
            del self.current_downloads[model_name]
        
        self.model_status_changed.emit(model_name, "available")
    
    def on_download_error(self, error_msg: str):
        """Handle download error"""
        # Find which model failed by checking the error message
        for model_name in list(self.current_downloads.keys()):
            if model_name in error_msg:
                del self.current_downloads[model_name]
                self.model_status_changed.emit(model_name, "error")
                break
    
    def delete_model(self, model_name: str) -> bool:
        """Delete a downloaded model"""
        if model_name in self.current_downloads:
            return False  # Can't delete while downloading
        
        model_path = self.get_model_path(model_name)
        if os.path.exists(model_path):
            try:
                os.remove(model_path)
                self.model_status_changed.emit(model_name, "not_downloaded")
                return True
            except Exception as e:
                print(f"Error deleting model: {e}")
                return False
        
        return True
    
    def get_total_models_size(self) -> int:
        """Get total size of downloaded models"""
        total_size = 0
        for model_name in self.model_sizes:
            if self.is_model_available(model_name):
                model_path = self.get_model_path(model_name)
                total_size += os.path.getsize(model_path)
        return total_size
    
    def cleanup_incomplete_downloads(self):
        """Clean up any incomplete downloads on startup"""
        for model_name in self.model_urls:
            model_path = self.get_model_path(model_name)
            if os.path.exists(model_path):
                # Check if file is complete (basic size check)
                expected_size = self.model_sizes.get(model_name, 0)
                actual_size = os.path.getsize(model_path)
                
                # If file is significantly smaller than expected, it's probably incomplete
                if expected_size > 0 and actual_size < expected_size * 0.9:
                    print(f"Cleaning up incomplete download: {model_name}")
                    os.remove(model_path)
