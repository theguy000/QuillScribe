"""
Output Manager for QuillScribe
Handles clipboard operations and auto-paste functionality
"""

import pyperclip
import time
from typing import Optional
from PySide6.QtCore import QObject, Signal, QTimer
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QClipboard


class OutputManager(QObject):
    """Manages transcription output - clipboard, paste, display"""
    
    operation_complete = Signal(str)  # Success message
    operation_failed = Signal(str)    # Error message
    
    def __init__(self, config_manager):
        super().__init__()
        self.config_manager = config_manager
        self.clipboard = QApplication.clipboard()
        
        # Output modes
        self.OUTPUT_MODES = {
            0: "copy_only",
            1: "paste_only", 
            2: "copy_and_paste",
            3: "display_only"
        }
    
    def process_transcription(self, text: str):
        """Process transcription based on user settings"""
        if not text or not text.strip():
            self.operation_failed.emit("No transcription text to process")
            return
        
        text = text.strip()
        
        # Get user settings
        output_mode = self.config_manager.get_setting("output/mode", 2)
        silent_mode = self.config_manager.get_setting("output/silent_mode", False)
        auto_clear = self.config_manager.get_setting("output/auto_clear", False)
        
        mode_name = self.OUTPUT_MODES.get(output_mode, "copy_and_paste")
        
        try:
            if mode_name == "copy_only":
                self._copy_to_clipboard(text)
                self.operation_complete.emit(f"Copied: {self._truncate_text(text)}")
                
            elif mode_name == "paste_only":
                self._paste_to_active_app(text)
                # For paste-only mode: ALWAYS clear clipboard immediately after pasting
                time.sleep(0.1)  # Brief delay to ensure paste completed
                self._clear_last_clipboard_entry(text)
                if not silent_mode:
                    self.operation_complete.emit(f"Pasted: {self._truncate_text(text)}")
                else:
                    self.operation_complete.emit("Pasted to active application")
                    
            elif mode_name == "copy_and_paste":
                self._copy_to_clipboard(text)
                time.sleep(0.15)  # Slightly longer delay for clipboard synchronization
                self._paste_to_active_app(text)
                if not silent_mode:
                    self.operation_complete.emit(f"Copied & Pasted: {self._truncate_text(text)}")
                else:
                    self.operation_complete.emit("Copied to clipboard and pasted")
                    
            elif mode_name == "display_only":
                if not silent_mode:
                    self.operation_complete.emit(f"Transcribed: {self._truncate_text(text)}")
                else:
                    self.operation_complete.emit("Transcription completed")
            
            # Auto-clear clipboard if enabled (for copy-only and copy-and-paste modes)
            if auto_clear and mode_name in ["copy_only", "copy_and_paste"]:
                auto_clear_delay = self.config_manager.get_setting("output/auto_clear_delay", 5)  # Default 5 seconds
                delay_ms = int(auto_clear_delay * 1000)  # Convert to milliseconds
                QTimer.singleShot(delay_ms, lambda: self._clear_last_clipboard_entry(text))
                
        except Exception as e:
            self.operation_failed.emit(f"Output error: {str(e)}")
    
    def _copy_to_clipboard(self, text: str):
        """Copy text to system clipboard"""
        try:
            # Use Qt clipboard
            self.clipboard.setText(text, QClipboard.Mode.Clipboard)
            
            # Fallback to pyperclip
            pyperclip.copy(text)
            
            # Verify copy operation
            if self.clipboard.text(QClipboard.Mode.Clipboard) != text:
                raise Exception("Clipboard verification failed")
                
        except Exception as e:
            raise Exception(f"Failed to copy to clipboard: {str(e)}")
    
    def _paste_to_active_app(self, text: str):
        """Paste text to the currently active application"""
        try:
            # First copy to clipboard to ensure text is available
            self._copy_to_clipboard(text)
            
            # Small delay to ensure clipboard is ready
            time.sleep(0.1)
            
            # Try Qt widget first if available
            from PySide6.QtWidgets import QApplication
            active_widget = QApplication.focusWidget()
            
            if active_widget:
                # If there's an active Qt widget, paste directly
                try:
                    if hasattr(active_widget, 'paste'):
                        active_widget.paste()
                        return
                    elif hasattr(active_widget, 'insertPlainText'):
                        active_widget.insertPlainText(text)
                        return
                except Exception:
                    pass  # Fall through to system-wide paste
            
            # For external applications, use platform-specific paste
            import sys
            if sys.platform == "win32":
                self._paste_windows()
            elif sys.platform == "darwin":
                self._paste_macos()
            else:
                self._paste_linux()
                
        except Exception as e:
            # Don't raise exception, just ensure text is in clipboard
            print(f"Warning: Paste operation failed, text copied to clipboard instead: {str(e)}")
            # Make sure clipboard has the text at least
            try:
                self._copy_to_clipboard(text)
            except Exception:
                pass
    
    def _paste_windows(self):
        """Paste using Windows API"""
        try:
            import win32api
            import win32con
            import time
            
            # Send Ctrl+V
            win32api.keybd_event(win32con.VK_CONTROL, 0, 0, 0)  # Ctrl down
            win32api.keybd_event(ord('V'), 0, 0, 0)             # V down
            time.sleep(0.02)  # Slightly longer delay for reliability
            win32api.keybd_event(ord('V'), 0, win32con.KEYEVENTF_KEYUP, 0)  # V up
            win32api.keybd_event(win32con.VK_CONTROL, 0, win32con.KEYEVENTF_KEYUP, 0)  # Ctrl up
            
        except ImportError:
            # Fallback: use SendInput API with ctypes (more reliable than keybd_event)
            try:
                import ctypes
                from ctypes import wintypes, Structure, c_ulong, c_ushort, c_short, byref
                
                # Define Windows structures for SendInput
                class KEYBDINPUT(Structure):
                    _fields_ = [("wVk", c_ushort),
                               ("wScan", c_ushort),
                               ("dwFlags", c_ulong),
                               ("time", c_ulong),
                               ("dwExtraInfo", ctypes.POINTER(c_ulong))]
                
                class INPUT(Structure):
                    _fields_ = [("type", c_ulong),
                               ("ki", KEYBDINPUT)]
                
                # Constants
                VK_CONTROL = 0x11
                VK_V = 0x56
                KEYEVENTF_KEYUP = 0x0002
                INPUT_KEYBOARD = 1
                
                def send_key_input(vk_code, key_up=False):
                    extra = c_ulong(0)
                    ii_ = INPUT()
                    ii_.type = INPUT_KEYBOARD
                    ii_.ki = KEYBDINPUT(vk_code, 0, KEYEVENTF_KEYUP if key_up else 0, 0, ctypes.pointer(extra))
                    ctypes.windll.user32.SendInput(1, ctypes.pointer(ii_), ctypes.sizeof(ii_))
                
                # Send Ctrl+V using SendInput (more reliable)
                send_key_input(VK_CONTROL, False)  # Ctrl down
                send_key_input(VK_V, False)        # V down
                time.sleep(0.02)
                send_key_input(VK_V, True)         # V up
                send_key_input(VK_CONTROL, True)   # Ctrl up
                
            except Exception as e:
                # Final fallback: use basic keybd_event
                try:
                    import ctypes
                    VK_CONTROL = 0x11
                    VK_V = 0x56
                    KEYEVENTF_KEYUP = 0x0002
                    
                    ctypes.windll.user32.keybd_event(VK_CONTROL, 0, 0, 0)  # Ctrl down
                    ctypes.windll.user32.keybd_event(VK_V, 0, 0, 0)        # V down
                    time.sleep(0.02)
                    ctypes.windll.user32.keybd_event(VK_V, 0, KEYEVENTF_KEYUP, 0)      # V up
                    ctypes.windll.user32.keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0) # Ctrl up
                except Exception:
                    # If all else fails, at least the text is in clipboard
                    print(f"Warning: Could not simulate paste keystrokes: {e}")
                    pass
    
    def _paste_macos(self):
        """Paste using macOS API"""
        try:
            import subprocess
            # Use AppleScript to send Cmd+V
            script = 'tell application "System Events" to keystroke "v" using command down'
            subprocess.run(['osascript', '-e', script], check=True)
        except Exception:
            pass
    
    def _paste_linux(self):
        """Paste using Linux tools"""
        try:
            import subprocess
            # Try xdotool first
            subprocess.run(['xdotool', 'key', 'ctrl+v'], check=True)
        except Exception:
            try:
                # Try xclip + xdotool
                subprocess.run(['xdotool', 'key', 'ctrl+v'], check=True)
            except Exception:
                pass
    
    def _clear_clipboard(self):
        """Clear the clipboard completely"""
        try:
            self.clipboard.clear(QClipboard.Mode.Clipboard)
        except Exception:
            pass  # Ignore errors when clearing clipboard
    
    def _clear_last_clipboard_entry(self, expected_text: str):
        """Clear clipboard only if it still contains our expected text"""
        try:
            current_clipboard = self.clipboard.text(QClipboard.Mode.Clipboard)
            # Only clear if the clipboard still contains the text we put there
            if current_clipboard == expected_text:
                self.clipboard.clear(QClipboard.Mode.Clipboard)
        except Exception:
            pass  # Ignore errors when clearing clipboard
    
    def _truncate_text(self, text: str, max_length: int = 50) -> str:
        """Truncate text for display purposes"""
        if len(text) <= max_length:
            return text
        return text[:max_length] + "..."
    
    def get_clipboard_content(self) -> str:
        """Get current clipboard content"""
        try:
            return self.clipboard.text(QClipboard.Mode.Clipboard)
        except Exception:
            return ""
    
    def test_clipboard(self) -> bool:
        """Test if clipboard operations work"""
        test_text = "QuillScribe clipboard test"
        try:
            original_content = self.get_clipboard_content()
            self._copy_to_clipboard(test_text)
            
            # Verify
            result = self.get_clipboard_content() == test_text
            
            # Restore original content
            if original_content:
                self.clipboard.setText(original_content, QClipboard.Mode.Clipboard)
            else:
                self.clipboard.clear(QClipboard.Mode.Clipboard)
            
            return result
        except Exception:
            return False
    
    def get_output_mode_description(self, mode: int) -> str:
        """Get human-readable description of output mode"""
        descriptions = {
            0: "Copy to clipboard only",
            1: "Paste to active app only", 
            2: "Copy and paste",
            3: "Display only (no copy/paste)"
        }
        return descriptions.get(mode, "Unknown mode")
