"""
QuillScribe Main Application
Beautiful voice-to-text transcription with minimal, elegant UI
"""

import sys
import math
from typing import Optional
from PySide6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QPushButton, QLabel, QGraphicsDropShadowEffect, QSizePolicy, QMessageBox
)
from PySide6.QtCore import Qt, QPropertyAnimation, QEasingCurve, QRect, Signal, Property, QTimer, QEvent, QSize, QAbstractNativeEventFilter, QAbstractEventDispatcher
from PySide6.QtGui import QPainter, QPen, QBrush, QColor, QFont, QIcon, QPixmap, QShortcut, QKeySequence

try:
    import ctypes  # type: ignore
    from ctypes import wintypes  # type: ignore
except Exception:
    ctypes = None
    wintypes = None

# Windows hotkey constants
WM_HOTKEY = 0x0312
MOD_ALT = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT = 0x0004
MOD_WIN = 0x0008


class WindowsHotkeyEventFilter(QAbstractNativeEventFilter):
    """Native event filter to listen for WM_HOTKEY messages on Windows."""

    def __init__(self, id_to_callback: dict[int, callable]):
        super().__init__()
        self.id_to_callback = id_to_callback

    def nativeEventFilter(self, eventType, message):  # noqa: N802 (Qt signature)
        if ctypes is None or sys.platform != "win32":
            return False, 0
        try:
            if eventType in ("windows_generic_MSG", "windows_dispatcher_MSG"):
                # Handle different types of message parameter that PySide6 might pass
                if hasattr(message, '__int__'):
                    # If message is an integer pointer
                    msg_ptr = ctypes.cast(int(message), ctypes.POINTER(wintypes.MSG))
                elif hasattr(message, 'value'):
                    # If message is a sip.voidptr 
                    msg_ptr = ctypes.cast(message.value, ctypes.POINTER(wintypes.MSG))
                else:
                    # Try direct cast as fallback
                    msg_ptr = ctypes.cast(message, ctypes.POINTER(wintypes.MSG))
                
                msg = msg_ptr.contents
                if msg.message == WM_HOTKEY:
                    hotkey_id = int(msg.wParam)
                    callback = self.id_to_callback.get(hotkey_id)
                    if callback:
                        try:
                            callback()
                        except Exception as e:
                            print(f"Hotkey callback error: {e}")
                    return True, 0
        except Exception as e:
            # Only print this occasionally to avoid spam
            import time
            if not hasattr(self, '_last_error_time') or time.time() - self._last_error_time > 5:
                print(f"DEBUG: Exception in nativeEventFilter: {e}")
                self._last_error_time = time.time()
            return False, 0
        return False, 0


class WindowsGlobalHotkeyManager:
    """Registers a global hotkey using Win32 RegisterHotKey and routes WM_HOTKEY to callbacks."""

    def __init__(self, window: QMainWindow):
        if ctypes is None or sys.platform != "win32":
            raise RuntimeError("Windows hotkey manager requires Windows and ctypes")
        self.window = window
        self.user32 = ctypes.windll.user32
        self.id_counter = 1
        self.id_to_callback: dict[int, callable] = {}
        self.registered_ids: set[int] = set()
        self.event_filter = WindowsHotkeyEventFilter(self.id_to_callback)
        dispatcher = QAbstractEventDispatcher.instance()
        if dispatcher is not None:
            dispatcher.installNativeEventFilter(self.event_filter)

    def _parse_shortcut(self, shortcut_text: str) -> tuple[int, int] | None:
        if not shortcut_text:
            return None
        
        # Split on + and clean up tokens
        tokens = [t.strip() for t in shortcut_text.split("+") if t.strip()]
        if not tokens:
            return None
            
        mods = 0
        vk = None
        
        # Process all tokens except the last one as modifiers
        for token in tokens[:-1]:
            t = token.lower()
            if t in ("win", "windows", "meta", "super"):
                mods |= MOD_WIN
            elif t in ("ctrl", "control"):
                mods |= MOD_CONTROL
            elif t == "alt":
                mods |= MOD_ALT
            elif t == "shift":
                mods |= MOD_SHIFT
        
        # Process the last token as the key
        key = tokens[-1].strip()
        key_lower = key.lower()
        
        # Function keys F1..F24
        if key_lower.startswith("f") and key_lower[1:].isdigit():
            n = int(key_lower[1:])
            if 1 <= n <= 24:
                vk = 0x70 + (n - 1)  # VK_F1 = 0x70
        elif len(key) == 1:
            # Special handling for certain characters that have different VK codes
            special_char_map = {
                "`": 0xC0,  # VK_OEM_3 (backtick/tilde key)
                "~": 0xC0,  # VK_OEM_3 (backtick/tilde key)
                "=": 0xBB,  # VK_OEM_PLUS
                "+": 0xBB,  # VK_OEM_PLUS
                "-": 0xBD,  # VK_OEM_MINUS
                "_": 0xBD,  # VK_OEM_MINUS
                "[": 0xDB,  # VK_OEM_4
                "{": 0xDB,  # VK_OEM_4
                "]": 0xDD,  # VK_OEM_6
                "}": 0xDD,  # VK_OEM_6
                "\\": 0xDC, # VK_OEM_5
                "|": 0xDC,  # VK_OEM_5
                ";": 0xBA,  # VK_OEM_1
                ":": 0xBA,  # VK_OEM_1
                "'": 0xDE,  # VK_OEM_7
                '"': 0xDE,  # VK_OEM_7
                ",": 0xBC,  # VK_OEM_COMMA
                "<": 0xBC,  # VK_OEM_COMMA
                ".": 0xBE,  # VK_OEM_PERIOD
                ">": 0xBE,  # VK_OEM_PERIOD
                "/": 0xBF,  # VK_OEM_2
                "?": 0xBF,  # VK_OEM_2
            }
            
            if key in special_char_map:
                vk = special_char_map[key]
            else:
                vk = ord(key.upper())
        else:
            special_map = {
                "space": 0x20,
                "tab": 0x09,
                "enter": 0x0D,
                "return": 0x0D,
                "escape": 0x1B,
                "esc": 0x1B,
                "backspace": 0x08,
                "insert": 0x2D,
                "delete": 0x2E,
                "home": 0x24,
                "end": 0x23,
                "pageup": 0x21,
                "pagedown": 0x22,
                "left": 0x25,
                "up": 0x26,
                "right": 0x27,
                "down": 0x28,
            }
            vk = special_map.get(key_lower)
        
        if vk is None:
            return None
            
        return mods, vk

    def register_hotkey(self, shortcut_text: str, callback: callable) -> bool:
        self.unregister_all()
        parsed = self._parse_shortcut(shortcut_text)
        if parsed is None:
            return False
        mods, vk = parsed
        hotkey_id = self.id_counter
        self.id_counter += 1
        hwnd = int(self.window.winId())
        
        res = int(self.user32.RegisterHotKey(hwnd, hotkey_id, mods, vk))
        
        if res == 0:
            # Get the last error code for debugging
            last_error = ctypes.windll.kernel32.GetLastError()
            error_messages = {
                1409: "Hot key is already registered",
                87: "The parameter is incorrect", 
                1413: "Invalid hotkey"
            }
            error_msg = error_messages.get(last_error, f"Unknown error {last_error}")
            print(f"Failed to register hotkey '{shortcut_text}': {error_msg}")
            return False
            
        self.id_to_callback[hotkey_id] = callback
        self.registered_ids.add(hotkey_id)
        return True

    def unregister_all(self):
        if ctypes is None or sys.platform != "win32":
            return
        hwnd = int(self.window.winId())
        for hotkey_id in list(self.registered_ids):
            try:
                self.user32.UnregisterHotKey(hwnd, hotkey_id)
            except Exception:
                pass
            self.registered_ids.discard(hotkey_id)
            self.id_to_callback.pop(hotkey_id, None)

    def cleanup(self):
        self.unregister_all()
        dispatcher = QAbstractEventDispatcher.instance()
        if dispatcher is not None and self.event_filter is not None:
            try:
                dispatcher.removeNativeEventFilter(self.event_filter)
            except Exception:
                pass

from .audio_manager import AudioManager
from .whisper_manager import WhisperManager
from .settings_dialog import SettingsDialog, UISettingsDialog
from .output_manager import OutputManager
from .config_manager import ConfigManager
from .sound_manager import SoundManager
from .icon_manager import get_icon, get_button_icon, get_white_button_icon


class BreathingMicrophone(QWidget):
    """Beautiful breathing microphone widget with smooth animations"""
    
    # Signal for when microphone is clicked
    clicked = Signal()
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedSize(200, 200)
        self.scale_factor = 1.0
        self.audio_level = 0.0
        self.is_recording = False
        self.show_waveform = True
        self.level_smoothed = 0.0
        self.animation_strength = 3.0  # Default amplification factor
        
        # Make the widget clickable
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        
        # Animation setup (disabled breathing; mic remains static)
        self.animation = QPropertyAnimation(self, b"scaleFactor")
        self.animation.setDuration(1500)
        self.animation.setEasingCurve(QEasingCurve.Type.InOutSine)
        self.animation.setLoopCount(-1)

        # Wave animation state for circular waveform while recording
        self.wave_phase = 0.0
        self.wave_timer = QTimer(self)
        self.wave_timer.setInterval(33)  # ~30 FPS
        self.wave_timer.timeout.connect(self.advance_wave)
        
        # Drop shadow effect
        shadow = QGraphicsDropShadowEffect()
        shadow.setBlurRadius(20)
        shadow.setColor(QColor(0, 0, 0, 50))
        shadow.setOffset(0, 5)
        self.setGraphicsEffect(shadow)
    
    # Property for animation
    def getScaleFactor(self):
        return self.scale_factor
    
    def setScaleFactor(self, value):
        if isinstance(value, (int, float)):
            self.scale_factor = float(value)
            self.update()  # Trigger repaint
    
    scaleFactor = Property(float, getScaleFactor, setScaleFactor)
    
    def set_show_waveform(self, value: bool):
        """Enable/disable waveform rendering around the microphone"""
        self.show_waveform = bool(value)
        self.update()
    
    def set_animation_strength(self, value: float):
        """Set the animation amplification strength"""
        self.animation_strength = max(1.0, min(10.0, float(value)))
        self.update()

    def start_idle_breathing(self):
        """Mic stays static when idle (no breathing)."""
        self.animation.stop()
        self.scale_factor = 1.0
        self.update()
    
    def start_recording_breathing(self):
        """Enable circular waveform while recording; mic remains static."""
        self.is_recording = True
        self.animation.stop()
        self.scale_factor = 1.0
        self.wave_timer.start()
        self.update()  # Force visual refresh
    
    def stop_recording(self):
        """Stop recording and clear waveform (mic stays static)."""
        self.is_recording = False
        self.wave_timer.stop()
        self.animation.stop()
        self.update()  # Force visual refresh
    
    def update_audio_level(self, level: float):
        """Update level (used for waveform only); do not scale mic with level"""
        # Additional gentle smoothing for visual stability
        level = max(0.0, min(1.0, level))
        self.level_smoothed = (0.85 * self.level_smoothed) + (0.15 * level)
        self.audio_level = self.level_smoothed
        # Intentionally avoid tying scale to audio level to prevent goofy movement

    def advance_wave(self):
        """Advance waveform phase for smooth motion while recording."""
        # Speed lightly influenced by amplified level
        amplified_level = min(1.0, self.audio_level * self.animation_strength)
        speed = 0.15 + (amplified_level * 0.5)
        self.wave_phase = (self.wave_phase + speed) % (2 * math.pi)
        self.update()
    
    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Get the center and radius
        center_x = self.width() // 2
        center_y = self.height() // 2
        base_radius = min(self.width(), self.height()) // 3
        radius = int(base_radius * self.scale_factor)
        
        # Create gradient background
        if self.is_recording:
            # Recording state - brand purple hues
            outer_color = QColor(128, 0, 128, 30)  # Purple, subtle
            middle_color = QColor(128, 0, 128, 60)
            inner_color = QColor(128, 0, 128, 100)
            mic_color = QColor(255, 255, 255)
        else:
            # Idle state - elegant grey gradient  
            outer_color = QColor(128, 128, 128, 20)
            middle_color = QColor(100, 100, 100, 40)
            inner_color = QColor(80, 80, 80, 80)
            mic_color = QColor(220, 220, 220)
        
        # Draw concentric circles for depth
        painter.setBrush(QBrush(outer_color))
        painter.setPen(Qt.PenStyle.NoPen)
        painter.drawEllipse(center_x - radius - 20, center_y - radius - 20, 
                          (radius + 20) * 2, (radius + 20) * 2)
        
        painter.setBrush(QBrush(middle_color))
        painter.drawEllipse(center_x - radius - 10, center_y - radius - 10,
                          (radius + 10) * 2, (radius + 10) * 2)
        
        painter.setBrush(QBrush(inner_color))
        painter.drawEllipse(center_x - radius, center_y - radius, radius * 2, radius * 2)
        
        # Draw microphone icon
        painter.setPen(QPen(mic_color, 3, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
        painter.setBrush(QBrush(mic_color))
        
        # Microphone body (capsule)
        mic_width = radius // 2
        mic_height = radius // 1.5
        mic_x = center_x - mic_width // 2
        mic_y = center_y - mic_height // 2 - 5
        
        painter.drawRoundedRect(mic_x, mic_y, mic_width, mic_height, 8, 8)
        
        # Microphone stand
        stand_y = mic_y + mic_height
        painter.drawLine(center_x, stand_y, center_x, stand_y + 15)
        painter.drawLine(center_x - 8, stand_y + 15, center_x + 8, stand_y + 15)
        
        # Circular waveform around mic while recording
        if self.is_recording and self.show_waveform:
            num_bars = 64
            inner_offset = 10
            base_ring_radius = radius + inner_offset
            # Visual parameters responsive to amplified level
            amplified_level = min(1.0, self.audio_level * self.animation_strength)
            max_bar_length = 14 + int(10 * amplified_level)
            # Ensure waveform fits fully inside the widget (no clipping)
            available_half = min(self.width(), self.height()) // 2 - 2
            max_possible_len = max(0, available_half - base_ring_radius)
            effective_max_bar_length = min(max_bar_length, max_possible_len)
            # If too small, reduce min length as well
            min_bar_length = 3 if effective_max_bar_length >= 3 else max(0, effective_max_bar_length // 2)
            # Soft glow ring behind waveform (brand purple)
            glow_alpha = int(90 * amplified_level)
            if glow_alpha > 0:
                painter.setBrush(Qt.BrushStyle.NoBrush)
                painter.setPen(QPen(QColor(128, 0, 128, glow_alpha), 8, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
                painter.drawEllipse(center_x - (base_ring_radius + effective_max_bar_length),
                                    center_y - (base_ring_radius + effective_max_bar_length),
                                    2 * (base_ring_radius + effective_max_bar_length),
                                    2 * (base_ring_radius + effective_max_bar_length))
            # Bars
            painter.setBrush(Qt.BrushStyle.NoBrush)

            for i in range(num_bars):
                theta = (2 * math.pi * i) / num_bars
                # Wave pattern around circle; amplitude scales with amplified audio level
                wave = 0.5 * (1.0 + math.sin(self.wave_phase + theta * 2.0))  # 0..1
                # Apply animation strength amplification, but cap at 1.0 to prevent overshooting
                amplified_level = min(1.0, self.audio_level * self.animation_strength)
                length = min_bar_length + (effective_max_bar_length - min_bar_length) * amplified_level * wave

                start_r = base_ring_radius
                end_r = base_ring_radius + length

                x1 = center_x + int(start_r * math.cos(theta))
                y1 = center_y + int(start_r * math.sin(theta))
                x2 = center_x + int(end_r * math.cos(theta))
                y2 = center_y + int(end_r * math.sin(theta))
                # Per-bar alpha and width for nicer depth (using amplified level)
                bar_alpha = 40 + int(60 * wave * amplified_level)
                bar_width = 2 + int(2 * amplified_level)
                painter.setPen(QPen(QColor(128, 0, 128, bar_alpha), bar_width, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
                painter.drawLine(x1, y1, x2, y2)
    
    def mousePressEvent(self, event):
        """Handle mouse press events to make microphone clickable"""
        if event.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()
        super().mousePressEvent(event)


class ModernButton(QPushButton):
    """Beautiful modern button with hover effects"""
    
    def __init__(self, text: str, primary: bool = False, parent=None):
        super().__init__(text, parent)
        self.primary = primary
        self.apply_theme(is_dark=False, compact=False)
        
        # Hover animation
        self.animation = QPropertyAnimation(self, b"geometry")
        self.animation.setDuration(150)
        self.animation.setEasingCurve(QEasingCurve.Type.OutCubic)

    def apply_theme(self, is_dark: bool, compact: bool = False):
        if self.primary:
            # Primary button (Record/Stop)
            stylesheet = """
                QPushButton {
                    background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                        stop:0 #4A90E2, stop:1 #357ABD);
                    color: white;
                    border: none;
                    border-radius: 25px;
                    font-size: 16px;
                    font-weight: 600;
                    padding: 12px 24px;
                    min-width: 120px;
                }
                QPushButton:hover {
                    background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                        stop:0 #5BA0F2, stop:1 #4A90E2);
                }
                QPushButton:pressed {
                    background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                        stop:0 #357ABD, stop:1 #2E6BA0);
                }
            """
        else:
            # Secondary button (Settings)
            if compact:
                font_size = "11px"
                border_radius = "8px"
                padding = "6px 10px"
                min_width = "60px"
            else:
                font_size = "16px"
                border_radius = "12px"
                padding = "12px 24px"
                min_width = "120px"

            if is_dark:
                stylesheet = f"""
                    QPushButton {{
                        background: qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 #3c3c3c, stop:1 #2c2c2c);
                        color: #f0f0f0;
                        border: 2px solid #555555;
                        border-radius: {border_radius};
                        font-size: {font_size};
                        font-weight: 500;
                        padding: {padding};
                        min-width: {min_width};
                        text-align: center;
                    }}
                    QPushButton:hover {{ background: qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 #4c4c4c, stop:1 #3c3c3c); border-color: #777777; }}
                    QPushButton:pressed {{ background: qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 #2c2c2c, stop:1 #1c1c1c); }}
                """
            else:
                stylesheet = f"""
                    QPushButton {{
                        background: qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 #f8f9fa, stop:1 #e9ecef);
                        color: #495057;
                        border: 2px solid #dee2e6;
                        border-radius: {border_radius};
                        font-size: {font_size};
                        font-weight: 500;
                        padding: {padding};
                        min-width: {min_width};
                        text-align: center;
                    }}
                    QPushButton:hover {{ background: qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 #ffffff, stop:1 #f1f3f4); border-color: #adb5bd; color: #495057; }}
                    QPushButton:pressed {{ background: qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 #e9ecef, stop:1 #dee2e6); }}
                """
        self.setStyleSheet(stylesheet)
        
        # Drop shadow
        shadow = QGraphicsDropShadowEffect()
        shadow.setBlurRadius(10)
        shadow.setColor(QColor(0, 0, 0, 30))
        shadow.setOffset(0, 2)
        self.setGraphicsEffect(shadow)


class QuillScribeMainWindow(QMainWindow):
    """Main application window with beautiful minimal UI"""
    
    # Theme color definitions (same as SettingsDialog)
    THEMES = {
        "white": {"primary": "#ffffff", "secondary": "#f8f9fa"},
        "warm_gray": {"primary": "#f5f5f5", "secondary": "#fafafa"},
        "soft_beige": {"primary": "#f8f6f0", "secondary": "#fefefe"},
        "blue_gray": {"primary": "#f0f2f5", "secondary": "#f8fafc"},
        "warm_taupe": {"primary": "#f7f3f0", "secondary": "#faf9f7"},
        "soft_sage": {"primary": "#f7f9f6", "secondary": "#f8faf9"},
        # Dark theme variations
        "dark_charcoal": {"primary": "#2c2c2c", "secondary": "#1e1e1e"},
        "dark_blue": {"primary": "#1a1f2e", "secondary": "#13182a"},
        "dark_purple": {"primary": "#2d1b3d", "secondary": "#241736"},
        "dark_forest": {"primary": "#1e2a1e", "secondary": "#152015"},
        "dark_burgundy": {"primary": "#2a1a1a", "secondary": "#1f1212"}
    }
    
    def __init__(self):
        super().__init__()
        self.config_manager = ConfigManager()
        self.audio_manager = AudioManager()
        self.whisper_manager = WhisperManager()
        self.output_manager = OutputManager(self.config_manager)
        self.sound_manager = SoundManager()
        self.settings_dialog = None
        self.is_recording = False
        self.compact_mode = False
        self.is_dark = False
        self._drag_active = False
        self._drag_offset = None
        self._app_shortcut = None
        
        self.setup_ui()
        self.setup_connections()
        self.load_settings()
        self.center_window()
        self.install_drag_filters()
        
        # Start audio monitoring for microphone animation
        try:
            self.audio_manager.start_monitoring()
        except Exception as e:
            print(f"Warning: Could not start audio monitoring: {e}")

        # Register global/app shortcut
        try:
            self._ensure_hotkey_manager()
            self.apply_hotkey_setting()
        except Exception as e:
            print(f"Warning: Could not set up hotkey: {e}")

    def _ensure_hotkey_manager(self):
        """Create platform-specific hotkey manager if not yet created."""
        if hasattr(self, "hotkey_manager") and self.hotkey_manager is not None:
            return
        self.hotkey_manager = None
        try:
            if sys.platform == "win32":
                self.hotkey_manager = WindowsGlobalHotkeyManager(self)
        except Exception:
            self.hotkey_manager = None
        
    def setup_ui(self):
        """Create the beautiful UI"""
        self.setWindowTitle("QuillScribe")
        self.setFixedSize(400, 500)
        
        # Check if custom titlebar is enabled (default to True for backward compatibility)
        custom_titlebar = bool(self.config_manager.get_setting("ui/custom_titlebar", True))
        if custom_titlebar:
            # Use frameless window for custom titlebar
            self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window)
        else:
            # Use standard window with system titlebar
            self.setWindowFlags(Qt.WindowType.Window)
        
        # Set window icon (ICO format only - guaranteed to be present)
        try:
            from pathlib import Path
            # Handle both development and frozen executable environments
            if getattr(sys, 'frozen', False):
                # Running as frozen executable - use PyInstaller's temporary directory
                ico_path = Path(sys._MEIPASS) / "app_logo.ico"
            else:
                # Running from source
                ico_path = Path(__file__).parent / "app_logo.ico"
            
            self.setWindowIcon(QIcon(str(ico_path)))
        except Exception as e:
            print(f"Error: Could not load window icon: {e}")
        
        # Central widget
        central_widget = QWidget()
        central_widget.setObjectName("centralWidget")
        self.setCentralWidget(central_widget)
        
        # Main layout
        layout = QVBoxLayout(central_widget)
        self.main_layout = layout
        
        # Store custom titlebar flag for later use
        self.custom_titlebar_enabled = custom_titlebar
        
        if custom_titlebar:
            # Custom titlebar mode: remove spacing and margins to accommodate titlebar
            layout.setSpacing(0)
            layout.setContentsMargins(0, 0, 0, 0)
            
            # Create and add custom titlebar
            self.create_custom_titlebar()
            layout.addWidget(self.custom_titlebar)
            
            # Main content area with original spacing
            content_widget = QWidget()
            content_layout = QVBoxLayout(content_widget)
            content_layout.setSpacing(30)
            content_layout.setContentsMargins(40, 20, 40, 40)
            layout.addWidget(content_widget)
            
            # Update main_layout to point to content layout for adding widgets
            self.main_layout = content_layout
        else:
            # Standard titlebar mode: use standard spacing and margins
            layout.setSpacing(30)
            layout.setContentsMargins(40, 20, 40, 40)
            
            # No custom titlebar needed
            self.custom_titlebar = None
        
        # Topbar with right-aligned close button (visible only in compact mode)
        self.topbar = QHBoxLayout()
        self.topbar.setContentsMargins(0, 8, 8, 0)  # Add right margin
        self.topbar.addStretch()
        self.close_button = QPushButton("×")
        self.close_button.setFixedSize(24, 24)
        self.close_button.setVisible(False)
        self.close_button.clicked.connect(self.close)
        # Ensure the text is perfectly centered
        self.close_button.setFont(QFont("Arial", 16, QFont.Weight.Bold))
        self.topbar.addWidget(self.close_button)
        
        # Apply initial close button styling
        self.apply_close_button_theme(False)
        layout.addLayout(self.topbar)
        
        # Title - properly centered
        title = QLabel("QuillScribe")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        title.setStyleSheet("""
            QLabel {
                color: #2c3e50;
                font-size: 28px;
                font-weight: 300;
                margin-bottom: 20px;
                text-align: center;
            }
        """)
        layout.addWidget(title)
        self.title_label = title
        
        # Breathing microphone
        self.microphone = BreathingMicrophone()
        mic_layout = QHBoxLayout()
        mic_layout.addStretch()
        mic_layout.addWidget(self.microphone)
        mic_layout.addStretch()
        layout.addLayout(mic_layout)
        
        layout.addStretch()
        
        # Settings button only
        self.settings_button = ModernButton("Settings", primary=False)
        self.settings_button.setIcon(get_button_icon('settings', 16))
        self.settings_button.setIconSize(QSize(16, 16))
        # Ensure button doesn't expand horizontally beyond its content
        self.settings_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        
        
        button_layout = QHBoxLayout()
        button_layout.addStretch()
        button_layout.addWidget(self.settings_button, 0, Qt.AlignmentFlag.AlignCenter)
        button_layout.addStretch()
        layout.addLayout(button_layout, 0)
        
        # Status label
        self.status_label = QLabel("Click microphone or press shortcut to start recording")
        self.status_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.status_label.setStyleSheet("""
            QLabel {
                color: #6c757d;
                font-size: 14px;
                margin-top: 10px;
            }
        """)
        layout.addWidget(self.status_label)
        
        # Window styling
        self.setStyleSheet("""
            QMainWindow {
                background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                    stop:0 #ffffff, stop:1 #f8f9fa);
            }
        """)
    
    def create_custom_titlebar(self):
        """Create a custom titlebar with perfectly centered title"""
        self.custom_titlebar = QWidget()
        self.custom_titlebar.setFixedHeight(32)
        self.custom_titlebar.setStyleSheet("""
            QWidget {
                background: #f0f0f0;
                border-bottom: 1px solid #d0d0d0;
            }
        """)
        
        titlebar_layout = QHBoxLayout(self.custom_titlebar)
        titlebar_layout.setContentsMargins(0, 0, 0, 0)
        titlebar_layout.setSpacing(0)
        titlebar_layout.setAlignment(Qt.AlignmentFlag.AlignVCenter)
        
        # Left section: icon + spacer (fixed width to balance right section)
        left_section = QWidget()
        left_section.setFixedWidth(100)  # Fixed width to match right section
        left_layout = QHBoxLayout(left_section)
        left_layout.setContentsMargins(8, 0, 0, 0)
        left_layout.setSpacing(8)
        
        # Window icon (ICO format only - guaranteed to be present)
        icon_label = QLabel()
        try:
            from pathlib import Path
            ico_path = Path(__file__).parent / "app_logo.ico"
            pixmap = QPixmap(str(ico_path)).scaled(16, 16, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.SmoothTransformation)
            icon_label.setPixmap(pixmap)
        except Exception as e:
            print(f"Error: Could not load titlebar icon: {e}")
        
        icon_label.setFixedSize(16, 16)
        icon_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        icon_label.setStyleSheet("""
            QLabel {
                background: transparent;
                border: none;
                padding: 0px;
                margin: 0px;
                text-decoration: none;
            }
        """)
        left_layout.addWidget(icon_label)
        left_layout.addStretch()
        titlebar_layout.addWidget(left_section)
        
        # Center section: title (expandable)
        center_section = QWidget()
        center_layout = QHBoxLayout(center_section)
        center_layout.setContentsMargins(0, 0, 0, 0)
        center_layout.setSpacing(0)
        
        self.titlebar_title = QLabel("QuillScribe")
        self.titlebar_title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.titlebar_title.setStyleSheet("""
            QLabel {
                color: #2c3e50;
                font-size: 13px;
                font-weight: 500;
                padding: 0px;
                background: transparent;
            }
        """)
        center_layout.addWidget(self.titlebar_title)
        titlebar_layout.addWidget(center_section)
        
        # Right section: window controls (fixed width to balance left section)
        right_section = QWidget()
        right_section.setFixedWidth(100)  # Fixed width to match left section
        right_layout = QHBoxLayout(right_section)
        right_layout.setContentsMargins(0, 0, 0, 0)
        right_layout.setSpacing(0)
        right_layout.addStretch()
        
        # Minimize button
        self.minimize_btn = QPushButton("−")
        self.minimize_btn.setFixedSize(46, 32)  # Match titlebar height exactly
        self.minimize_btn.setStyleSheet("""
            QPushButton {
                background: transparent;
                border: none;
                color: #2c3e50;
                font-size: 16px;
                font-weight: bold;
                text-align: center;
                margin: 0px;
                padding: 0px;
            }
            QPushButton:hover {
                background: #e0e0e0;
            }
            QPushButton:pressed {
                background: #d0d0d0;
            }
        """)
        self.minimize_btn.clicked.connect(self.showMinimized)
        right_layout.addWidget(self.minimize_btn)
        
        # Close button
        self.titlebar_close_btn = QPushButton("×")
        self.titlebar_close_btn.setFixedSize(46, 32)  # Match titlebar height exactly
        self.titlebar_close_btn.setStyleSheet("""
            QPushButton {
                background: transparent;
                border: none;
                color: #2c3e50;
                font-size: 16px;
                font-weight: bold;
                text-align: center;
                margin: 0px;
                padding: 0px;
            }
            QPushButton:hover {
                background: #e81123;
                color: white;
            }
            QPushButton:pressed {
                background: #c50e1f;
                color: white;
            }
        """)
        self.titlebar_close_btn.clicked.connect(self.close)
        right_layout.addWidget(self.titlebar_close_btn)
        
        titlebar_layout.addWidget(right_section)
        
        # Make titlebar draggable
        self.custom_titlebar.mousePressEvent = self.titlebar_mouse_press
        self.custom_titlebar.mouseMoveEvent = self.titlebar_mouse_move
        self.custom_titlebar.mouseReleaseEvent = self.titlebar_mouse_release
        self._titlebar_drag_active = False
        self._titlebar_drag_offset = None
    
    def titlebar_mouse_press(self, event):
        """Handle titlebar mouse press for dragging"""
        if event.button() == Qt.MouseButton.LeftButton:
            self._titlebar_drag_active = True
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self._titlebar_drag_offset = global_pos - self.frameGeometry().topLeft()
    
    def titlebar_mouse_move(self, event):
        """Handle titlebar mouse move for dragging"""
        if self._titlebar_drag_active and event.buttons() & Qt.MouseButton.LeftButton:
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self.move(global_pos - self._titlebar_drag_offset)
    
    def titlebar_mouse_release(self, event):
        """Handle titlebar mouse release"""
        if event.button() == Qt.MouseButton.LeftButton:
            self._titlebar_drag_active = False
    
    def setup_connections(self):
        """Connect signals and slots"""
        self.microphone.clicked.connect(self.toggle_recording)
        self.settings_button.clicked.connect(self.show_settings)
        
        # Audio manager connections
        self.audio_manager.audio_level_changed.connect(self.microphone.update_audio_level)
        
        # Whisper manager connections  
        self.whisper_manager.transcription_ready.connect(self.handle_transcription)
        self.whisper_manager.transcription_error.connect(self.handle_transcription_error)
        
        # Output manager connections
        self.output_manager.operation_complete.connect(self.update_status)
        self.output_manager.operation_failed.connect(self.handle_output_error)
    
    def center_window(self):
        """Center the window on screen"""
        screen = QApplication.primaryScreen().availableGeometry()
        size = self.geometry()
        self.move(
            (screen.width() - size.width()) // 2,
            (screen.height() - size.height()) // 2
        )
    
    def toggle_recording(self):
        """Toggle between recording and stop states"""
        if not self.is_recording:
            self.start_recording()
        else:
            self.stop_recording()
    
    def start_recording(self):
        """Start voice recording"""
        # Play start sound
        self.sound_manager.play_start_sound()
        
        self.is_recording = True
        self.status_label.setText("Recording... (Click microphone or press shortcut to stop)")
        self.microphone.start_recording_breathing()
        
        # Start audio capture
        try:
            self.audio_manager.start_recording()
        except Exception as e:
            self.handle_recording_error(f"Failed to start recording: {str(e)}")
            self.is_recording = False
            self.status_label.setText("Click microphone or press shortcut to start recording")
            self.microphone.stop_recording()
    
    def stop_recording(self):
        """Stop voice recording"""
        self.is_recording = False
        self.status_label.setText("Processing...")
        self.microphone.stop_recording()
        
        # Stop audio capture and process
        audio_data = self.audio_manager.stop_recording()
        if audio_data is not None and len(audio_data) > 0:
            self.whisper_manager.transcribe_audio(audio_data)
        else:
            # Play stop sound even when no audio data
            self.sound_manager.play_stop_sound()
            self.status_label.setText("No audio data recorded - Click microphone or press shortcut to try again")
    
    def handle_transcription(self, text: str):
        """Handle transcription result"""
        if text:
            # Process the transcription through output manager
            self.output_manager.process_transcription(text)
        else:
            # Play stop sound even when no speech detected
            self.sound_manager.play_stop_sound()
            self.status_label.setText("No speech detected - Click microphone or press shortcut to try again")
    
    def handle_transcription_error(self, error: str):
        """Handle transcription errors"""
        # Play stop sound on error too
        self.sound_manager.play_stop_sound()
        self.status_label.setText(f"Error: {error}")
        print(f"Transcription error: {error}")
    
    def handle_output_error(self, error: str):  
        """Handle output operation errors"""
        self.status_label.setText(f"Output error: {error}")
        print(f"Output error: {error}")
    
    def update_status(self, message: str):
        """Update status label with message"""
        # Play stop sound when transcription is complete
        self.sound_manager.play_stop_sound()
        
        self.status_label.setText(f"{message} - Click microphone or press shortcut to record again")
    
    def handle_recording_error(self, error: str):
        """Handle recording errors"""
        self.status_label.setText(f"Recording error: {error}")
        print(f"Recording error: {error}")
        
    def show_settings(self):
        """Show settings dialog"""
        if self.compact_mode:
            dialog = UISettingsDialog(self, self.config_manager)
            dialog.settings_saved.connect(self.load_settings)
            dialog.exec()
        else:
            if not self.settings_dialog:
                # Pass our config manager to ensure settings are shared
                self.settings_dialog = SettingsDialog(self, self.config_manager)
                # Connect to settings saved signal to reload settings when changed
                self.settings_dialog.settings_saved.connect(self.load_settings)
            else:
                # If dialog already exists, refresh device list when opening settings
                if hasattr(self.settings_dialog, 'audio_tab'):
                    self.settings_dialog.audio_tab.refresh_devices()
            self.settings_dialog.exec()
    
    def load_settings(self):
        """Load and apply saved settings"""
        try:
            # Load Whisper settings
            whisper_mode = self.config_manager.get_setting("whisper/mode", "api")
            self.whisper_manager.set_mode(whisper_mode)
            
            if whisper_mode == "api":
                api_key = self.config_manager.get_setting("whisper/api_key", "")
                if api_key:
                    self.whisper_manager.set_api_key(api_key)
            else:
                # Local model will be set below in the local model section
                pass
                    
            # Load audio settings
            device_id = self.config_manager.get_setting("audio/device_id")
            if device_id is not None:
                self.audio_manager.set_input_device(device_id)
            
            # Load sound settings
            sounds_enabled = self.config_manager.get_setting("audio/sounds_enabled", True)
            self.sound_manager.set_sounds_enabled(sounds_enabled)
            # Load UI visualization settings
            show_waveform = self.config_manager.get_setting("ui/show_waveform", True)
            self.microphone.set_show_waveform(bool(show_waveform))
            
            # Load animation strength setting
            animation_strength = self.config_manager.get_setting("ui/animation_strength", 3.0)
            self.microphone.set_animation_strength(float(animation_strength))
            
            # Apply compact mode
            compact = bool(self.config_manager.get_setting("ui/compact_mode", False))
            if compact != self.compact_mode:
                self.apply_compact_mode(compact)
            else:
                # Ensure consistent UI after startup
                self.apply_compact_mode(compact)
                
            # Set local model name if available
            if whisper_mode == "local":
                local_model = self.config_manager.get_setting("whisper/local_model", "")
                if local_model:
                    # Convert old file names to new model names
                    self.whisper_manager.set_local_model(local_model)
            
            # Apply recording shortcut
            self.apply_hotkey_setting()
            
            # Apply theme
            theme = self.config_manager.get_setting("ui/theme", "white")
            self.apply_theme(theme)
            
            # Apply custom titlebar setting (requires restart to take effect)
            custom_titlebar = bool(self.config_manager.get_setting("ui/custom_titlebar", True))
            if hasattr(self, 'custom_titlebar_enabled') and custom_titlebar != self.custom_titlebar_enabled:
                # Setting has changed - show popup that restart is required
                self.show_restart_required_popup()
                
        except Exception as e:
            print(f"Error loading settings: {e}")

    def show_restart_required_popup(self):
        """Show a popup dialog informing user that restart is required for title bar changes"""
        # Get current setting to show specific message
        custom_titlebar = bool(self.config_manager.get_setting("ui/custom_titlebar", True))
        action = "enabled" if custom_titlebar else "disabled"
        
        msg_box = QMessageBox(self)
        msg_box.setWindowTitle("Restart Required")
        msg_box.setIcon(QMessageBox.Icon.Information)
        msg_box.setText("Title Bar Setting Changed")
        msg_box.setInformativeText(f"The custom title bar has been {action}. Please restart QuillScribe for the changes to take effect.")
        msg_box.setStandardButtons(QMessageBox.StandardButton.Ok)
        
        # Apply theme styling to the message box
        if hasattr(self, 'is_dark') and self.is_dark:
            msg_box.setStyleSheet("""
                QMessageBox {
                    background-color: #2c2c2c;
                    color: #ffffff;
                }
                QMessageBox QLabel {
                    color: #ffffff;
                }
                QPushButton {
                    background-color: #4A90E2;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    padding: 8px 16px;
                    font-size: 13px;
                }
                QPushButton:hover {
                    background-color: #5BA0F2;
                }
            """)
        else:
            msg_box.setStyleSheet("""
                QMessageBox {
                    background-color: #ffffff;
                    color: #2c3e50;
                }
                QPushButton {
                    background-color: #4A90E2;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    padding: 8px 16px;
                    font-size: 13px;
                }
                QPushButton:hover {
                    background-color: #357ABD;
                }
            """)
        
        msg_box.exec()

    def apply_hotkey_setting(self):
        """Register the configured recording shortcut; fallback to app-level shortcut if global fails."""
        # Clear existing app shortcut
        if self._app_shortcut is not None:
            try:
                self._app_shortcut.activated.disconnect()
            except Exception:
                pass
            self._app_shortcut.setParent(None)
            self._app_shortcut = None

        shortcut_text = self.config_manager.get_setting("shortcuts/record_toggle", "Win+F")
        if not isinstance(shortcut_text, str) or not shortcut_text:
            shortcut_text = "Win+F"
        
        # Try global first on Windows
        registered_globally = False
        if getattr(self, "hotkey_manager", None) is not None:
            try:
                registered_globally = self.hotkey_manager.register_hotkey(shortcut_text, self.toggle_recording)
            except Exception as e:
                print(f"Hotkey registration error: {e}")
                registered_globally = False

        if not registered_globally:
            # Fallback: in-app shortcut (works when app is focused)
            # Map Win->Meta for Qt
            qt_seq_text = shortcut_text.replace("Win+", "Meta+").replace("Windows+", "Meta+")
            if "Meta+" in qt_seq_text and sys.platform == "win32":
                # Many Win+ combos are reserved; provide a safer fallback
                # Use Ctrl+Alt+<last key>
                parts = shortcut_text.split("+")
                last_key = parts[-1].strip() if parts else "F"
                qt_seq_text = f"Ctrl+Alt+{last_key}"
            try:
                self._app_shortcut = QShortcut(QKeySequence(qt_seq_text), self)
                self._app_shortcut.activated.connect(self.toggle_recording)
            except Exception as e:
                print(f"Failed to set app shortcut '{qt_seq_text}': {e}")
    
    def apply_close_button_theme(self, is_dark: bool):
        """Apply theme-appropriate styling to the close button for compact mode"""
        if is_dark:
            style = """
                QPushButton {
                    background: rgba(255, 255, 255, 0.1);
                    color: #ffffff;
                    border: 1px solid rgba(255, 255, 255, 0.2);
                    border-radius: 12px;
                    text-align: center;
                    padding: 0px;
                    margin: 0px;
                    text-align: center;
                }
                QPushButton:hover {
                    background: rgba(255, 255, 255, 0.2);
                    border-color: rgba(255, 255, 255, 0.4);
                }
                QPushButton:pressed {
                    background: rgba(255, 255, 255, 0.3);
                }
            """
        else:
            style = """
                QPushButton {
                    background: rgba(0, 0, 0, 0.05);
                    color: #495057;
                    border: 1px solid rgba(0, 0, 0, 0.1);
                    border-radius: 12px;
                    text-align: center;
                    padding: 0px;
                    margin: 0px;
                    text-align: center;
                }
                QPushButton:hover {
                    background: rgba(0, 0, 0, 0.1);
                    border-color: rgba(0, 0, 0, 0.2);
                    color: #212529;
                }
                QPushButton:pressed {
                    background: rgba(0, 0, 0, 0.15);
                }
            """
        self.close_button.setStyleSheet(style)
    
    def apply_theme(self, theme_name):
        """Apply the selected theme to the main window background and text colors"""
        theme = self.THEMES.get(theme_name, self.THEMES["white"])
        primary_color = theme["primary"]
        secondary_color = theme["secondary"]
        
        # Determine if this is a dark theme
        r = int(primary_color.lstrip('#')[0:2], 16)
        g = int(primary_color.lstrip('#')[2:4], 16)
        b = int(primary_color.lstrip('#')[4:6], 16)
        luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255
        is_dark = luminance < 0.5
        self.is_dark = is_dark
        
        if is_dark:
            text_primary = "#ffffff"
            text_secondary = "#e0e0e0"
            text_muted = "#b0b0b0"
        else:
            text_primary = "#2c3e50"
            text_secondary = "#495057"
            text_muted = "#6c757d"
        
        self.setStyleSheet(f"""
            QMainWindow {{
                background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                    stop:0 {primary_color}, stop:1 {secondary_color});
                color: {text_primary};
            }}
            QWidget#centralWidget {{
                background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                    stop:0 {primary_color}, stop:1 {secondary_color});
                color: {text_primary};
            }}
        """)
        
        # Apply theme to settings button
        self.settings_button.apply_theme(self.is_dark, self.compact_mode)
        if self.is_dark:
            self.settings_button.setIcon(get_white_button_icon('settings', 16))
        else:
            self.settings_button.setIcon(get_button_icon('settings', 16))
        
        # Apply theme to close button
        self.apply_close_button_theme(self.is_dark)

        

        # Update text colors for existing widgets
        if hasattr(self, 'title_label'):
            self.title_label.setStyleSheet(f"""
                QLabel {{
                    color: {text_primary};
                    font-size: 28px;
                    font-weight: 300;
                    margin-bottom: 10px;
                    background-color: transparent;
                    text-align: center;
                }}
            """)
        
        # Update custom titlebar colors
        if hasattr(self, 'custom_titlebar') and self.custom_titlebar is not None:
            # Determine titlebar colors based on theme
            if is_dark:
                titlebar_bg = "#3c3c3c"
                titlebar_border = "#555555"
                titlebar_text = "#ffffff"
                titlebar_btn_color = "#ffffff"
                titlebar_btn_hover = "#555555"
            else:
                titlebar_bg = "#f0f0f0"
                titlebar_border = "#d0d0d0"
                titlebar_text = "#2c3e50"
                titlebar_btn_color = "#2c3e50"
                titlebar_btn_hover = "#e0e0e0"
            
            self.custom_titlebar.setStyleSheet(f"""
                QWidget {{
                    background: {titlebar_bg};
                    border-bottom: 1px solid {titlebar_border};
                }}
                QLabel {{
                    background: transparent;
                    border: none;
                    padding: 0px;
                    margin: 0px;
                    text-decoration: none;
                }}
            """)
            
            if hasattr(self, 'titlebar_title'):
                self.titlebar_title.setStyleSheet(f"""
                    QLabel {{
                        color: {titlebar_text};
                        font-size: 14px;
                        font-weight: 400;
                        padding: 0px 20px;
                    }}
                """)
            
            if hasattr(self, 'minimize_btn'):
                self.minimize_btn.setStyleSheet(f"""
                    QPushButton {{
                        background: transparent;
                        border: none;
                        color: {titlebar_btn_color};
                        font-size: 16px;
                        font-weight: bold;
                        text-align: center;
                        margin: 0px;
                        padding: 0px;
                    }}
                    QPushButton:hover {{
                        background: {titlebar_btn_hover};
                    }}
                    QPushButton:pressed {{
                        background: {titlebar_bg if is_dark else '#d0d0d0'};
                    }}
                """)
            
            if hasattr(self, 'titlebar_close_btn'):
                self.titlebar_close_btn.setStyleSheet(f"""
                    QPushButton {{
                        background: transparent;
                        border: none;
                        color: {titlebar_btn_color};
                        font-size: 16px;
                        font-weight: bold;
                        text-align: center;
                        margin: 0px;
                        padding: 0px;
                    }}
                    QPushButton:hover {{
                        background: #e81123;
                        color: white;
                    }}
                    QPushButton:pressed {{
                        background: #c50e1f;
                        color: white;
                    }}
                """)
        
        if hasattr(self, 'status_label'):
            font_size = "11px" if self.compact_mode else "14px"
            margin = "4px" if self.compact_mode else "10px"
            self.status_label.setStyleSheet(f"""
                QLabel {{
                    color: {text_muted};
                    font-size: {font_size};
                    margin-top: {margin};
                    background-color: transparent;
                }}
            """)
    
    def apply_compact_mode(self, enabled: bool):
        """Apply or remove super-compact UI mode."""
        self.compact_mode = enabled
        if enabled:
            # Hide custom titlebar in compact mode (already frameless)
            if hasattr(self, 'custom_titlebar') and self.custom_titlebar is not None:
                self.custom_titlebar.setVisible(False)
            # Slightly larger to allow bigger mic view box and bottom-aligned settings
            self.setFixedSize(220, 240)
            # Tight spacing
            self.main_layout.setContentsMargins(8, 8, 8, 8)
            self.main_layout.setSpacing(6)
            # Hide big title, shrink mic and labels, enlarge viewbox by using more space
            self.title_label.setVisible(False)
            # Bigger microphone animation view box in compact mode
            self.microphone.setFixedSize(150, 150)
            # Completely remove text result/status display in compact mode
            self.status_label.setVisible(False)
            self.status_label.setStyleSheet("QLabel { color: #6c757d; font-size: 11px; margin-top: 4px; }")
            # Shrink settings button
            self.settings_button.apply_theme(self.is_dark, True)
            # Show centered close button with proper theme
            self.close_button.setVisible(True)
            self.apply_close_button_theme(self.is_dark)
            # Re-show to apply window flag changes
            self.show()
        else:
            # Show custom titlebar in normal mode
            if hasattr(self, 'custom_titlebar') and self.custom_titlebar is not None:
                self.custom_titlebar.setVisible(True)
            # Keep frameless window flags for custom titlebar
            self.setFixedSize(400, 532)  # Slightly taller to account for custom titlebar
            self.main_layout.setContentsMargins(40, 20, 40, 40)
            self.main_layout.setSpacing(30)
            self.title_label.setVisible(True)
            self.microphone.setFixedSize(200, 200)
            self.status_label.setVisible(True)
            self.status_label.setStyleSheet("QLabel { color: #6c757d; font-size: 14px; margin-top: 10px; }")
            # Restore settings button default style
            self.settings_button.apply_theme(self.is_dark, False)
            self.close_button.setVisible(False)
            # Re-show to apply window flag changes
            self.show()

    def install_drag_filters(self):
        """Install event filters to enable drag-anywhere in compact mode."""
        try:
            # Central widget and all children
            if self.centralWidget() is not None:
                self.centralWidget().installEventFilter(self)
            for child in self.findChildren(QWidget):
                child.installEventFilter(self)
        except Exception:
            pass
    
    
    def closeEvent(self, event):
        """Handle application close event"""
        try:
            minimize_on_close = bool(self.config_manager.get_setting("ui/minimize_on_close", True))
        except Exception:
            minimize_on_close = True

        if minimize_on_close:
            event.ignore()
            self.showMinimized()
            return

        # Proceed with actual close
        try:
            if self.is_recording:
                self.stop_recording()
            self.audio_manager.stop_monitoring()
            self.sound_manager.cleanup()
            if hasattr(self, "hotkey_manager") and self.hotkey_manager is not None:
                try:
                    self.hotkey_manager.cleanup()
                except Exception:
                    pass
            pos = self.pos()
            self.config_manager.set_setting("ui/window_x", pos.x())
            self.config_manager.set_setting("ui/window_y", pos.y())
            self.config_manager.save_settings()
        finally:
            event.accept()

    # Drag-anywhere support for compact mode
    def mousePressEvent(self, event):
        if self.compact_mode and event.button() == Qt.MouseButton.LeftButton:
            self._drag_active = True
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self._drag_offset = global_pos - self.frameGeometry().topLeft()
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        if self.compact_mode and self._drag_active and event.buttons() & Qt.MouseButton.LeftButton:
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self.move(global_pos - self._drag_offset)
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        if self.compact_mode and event.button() == Qt.MouseButton.LeftButton:
            self._drag_active = False
        super().mouseReleaseEvent(event)

    def eventFilter(self, obj, event):
        """Enable dragging from any child widget while in compact mode."""
        if not self.compact_mode:
            return super().eventFilter(obj, event)
        if event.type() == QEvent.Type.MouseButtonPress and event.button() == Qt.MouseButton.LeftButton:
            self._drag_active = True
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self._drag_offset = global_pos - self.frameGeometry().topLeft()
            return False
        if event.type() == QEvent.Type.MouseMove and self._drag_active and event.buttons() & Qt.MouseButton.LeftButton:
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self.move(global_pos - self._drag_offset)
            return True
        if event.type() == QEvent.Type.MouseButtonRelease:
            self._drag_active = False
            return False
        return super().eventFilter(obj, event)


def main():
    """Main application entry point"""
    app = QApplication(sys.argv)
    
    # Set application properties
    app.setApplicationName("QuillScribe")
    app.setApplicationVersion("1.0.0")
    app.setOrganizationName("QuillScribe")
    
    # Windows-specific taskbar configuration
    if sys.platform == "win32" and ctypes is not None:
        try:
            # Set Windows App Model ID for proper taskbar grouping and icon display
            from ctypes import windll
            windll.shell32.SetCurrentProcessExplicitAppUserModelID("QuillScribe.VoiceTranscription.1.0")
        except Exception as e:
            print(f"Warning: Could not set Windows App Model ID: {e}")
    
    # Set application icon (ICO format only - guaranteed to be present)
    try:
        from pathlib import Path
        # Handle both development and frozen executable environments
        if getattr(sys, 'frozen', False):
            # Running as frozen executable - use PyInstaller's temporary directory
            ico_path = Path(sys._MEIPASS) / "app_logo.ico"
        else:
            # Running from source
            ico_path = Path(__file__).parent / "app_logo.ico"
        
        app.setWindowIcon(QIcon(str(ico_path)))
    except Exception as e:
        print(f"Error: Could not load application icon: {e}")
    
    # Create and show main window
    window = QuillScribeMainWindow()
    window.show()
    
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
