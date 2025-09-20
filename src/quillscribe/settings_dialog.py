"""
Settings Dialog for QuillScribe
Beautiful settings panel with modern UI
"""

from PySide6.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QLabel, QComboBox, 
    QPushButton, QLineEdit, QRadioButton, QButtonGroup,
    QGroupBox, QProgressBar, QTextEdit, QCheckBox, QSlider,
    QTabWidget, QWidget, QFormLayout, QSpacerItem, QSizePolicy,
    QScrollArea, QKeySequenceEdit, QListView, QGraphicsDropShadowEffect,
    QMessageBox
)
from PySide6.QtCore import Qt, QTimer, Signal, QEvent, QSize
from PySide6.QtGui import QFont, QIcon, QPixmap, QPainter, QPen, QColor, QKeySequence, QIntValidator

from .audio_manager import AudioManager
from .whisper_manager import WhisperManager
from .config_manager import ConfigManager
from .icon_manager import get_icon, get_button_icon, get_white_button_icon


class ModernGroupBox(QGroupBox):
    """Beautiful modern group box with theme support"""
    
    def __init__(self, title: str, parent=None):
        super().__init__(title, parent)
        self.apply_default_theme()
    
    def apply_theme(self, primary_color="#ffffff", secondary_color="#f8f9fa"):
        """Apply theme colors to the group box"""
        # Determine if this is a dark theme
        r = int(primary_color.lstrip('#')[0:2], 16)
        g = int(primary_color.lstrip('#')[2:4], 16)
        b = int(primary_color.lstrip('#')[4:6], 16)
        luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255
        is_dark = luminance < 0.5
        
        if is_dark:
            text_color = "#ffffff"
            border_color = "#555555"
        else:
            text_color = "#2c3e50"
            border_color = "#dee2e6"
        
        self.setStyleSheet(f"""
            QGroupBox {{
                font-size: 14px;
                font-weight: 600;
                color: {text_color};
                border: 2px solid {border_color};
                border-radius: 8px;
                margin-top: 10px;
                padding-top: 10px;
                background-color: {primary_color};
            }}
            QGroupBox::title {{
                subcontrol-origin: margin;
                left: 10px;
                padding: 0 8px 0 8px;
                background-color: {primary_color};
                color: {text_color};
            }}
        """)
    
    def apply_default_theme(self):
        """Apply default white theme"""
        self.apply_theme("#ffffff", "#f8f9fa")


class ModernComboBox(QComboBox):
    """Beautiful modern combo box"""
    
    def __init__(self, parent=None):
        super().__init__(parent)
        # Use a list view for better styling control and add subtle shadow
        self.setView(QListView(self))
        self.view().setAlternatingRowColors(True)
        try:
            shadow = QGraphicsDropShadowEffect(self.view())
            shadow.setBlurRadius(18)
            shadow.setOffset(0, 4)
            shadow.setColor(QColor(0, 0, 0, 60))
            self.view().setGraphicsEffect(shadow)
        except Exception:
            pass
        self.setIconSize(QSize(14, 14))
        self.apply_theme(is_dark=False)

    def apply_theme(self, is_dark: bool, accent: str = "#4A90E2", border: str | None = None):
        border = border or ("#555555" if is_dark else "#dee2e6")
        text_color = "#f0f0f0" if is_dark else "#495057"
        bg = "#2c2c2c" if is_dark else "white"
        hover_bg = "#333333" if is_dark else "#f8f9fa"
        alt_bg = "#252525" if is_dark else "#f7f7f7"

        stylesheet = f"""
            QComboBox {{
                border: 1.5px solid {border};
                border-radius: 8px;
                padding: 4px 20px 4px 8px;
                min-height: 26px;
                font-size: 13px;
                background-color: {bg};
                color: {text_color};
                outline: none;
            }}
            QComboBox:hover {{
                border-color: {'#777777' if is_dark else '#adb5bd'};
                background-color: {hover_bg};
            }}
            QComboBox:focus {{
                outline: none;
                border: 1.5px solid {accent};
            }}
            QComboBox::drop-down {{
                border: none;
                background: transparent;
                width: 20px;
            }}
            QComboBox::down-arrow {{
                image: none;
                border-left: 4px solid transparent;
                border-right: 4px solid transparent;
                border-top: 4px solid {text_color};
                width: 0px;
                height: 0px;
                margin: 6px;
            }}
            QComboBox QAbstractItemView {{
                border: 1.5px solid {border};
                border-radius: 8px;
                background-color: {bg};
                color: {text_color};
                selection-background-color: {accent};
                selection-color: white;
                alternate-background-color: {alt_bg};
            }}
            QComboBox QAbstractItemView::item {{
                height: 24px;
                padding: 4px 8px;
            }}
            QComboBox QAbstractItemView::item:hover {{
                background-color: {accent};
                color: white;
                outline: none;
                border: none;
            }}
            QListView {{
                outline: none;
                border: none;
            }}
            QListView::item {{
                outline: none !important;
                border: none !important;
            }}
            QListView::item:focus {{
                outline: none !important;
                border: none !important;
            }}
            QListView::item:selected {{
                outline: none !important;
                border: none !important;
            }}
            QScrollBar:vertical {{
                width: 0px;
            }}
            QComboBox:disabled {{
                color: {'#9aa0a6' if is_dark else '#adb5bd'};
                border-style: dashed;
            }}
        """
        self.setStyleSheet(stylesheet)
        try:
            # Elide long texts on the right to avoid overflow
            if hasattr(self.view(), "setTextElideMode"):
                self.view().setTextElideMode(Qt.TextElideMode.ElideRight)
        except Exception:
            pass


class ModernLineEdit(QLineEdit):
    """Beautiful modern line edit"""
    
    def __init__(self, placeholder: str = "", parent=None):
        super().__init__(parent)
        self.setPlaceholderText(placeholder)
        self.apply_theme(is_dark=False)

    def apply_theme(self, is_dark: bool):
        if is_dark:
            stylesheet = """
                QLineEdit {
                    border: 2px solid #555555;
                    border-radius: 6px;
                    padding: 8px;
                    font-size: 13px;
                    background-color: #2c2c2c;
                    color: #f0f0f0;
                    min-height: 20px;
                    outline: none;
                }
                QLineEdit:hover {
                    border-color: #777777;
                }
                QLineEdit:focus {
                    border-color: #4A90E2;
                }
            """
        else:
            stylesheet = """
                QLineEdit {
                    border: 2px solid #dee2e6;
                    border-radius: 6px;
                    padding: 8px;
                    font-size: 13px;
                    background-color: white;
                    color: black;
                    min-height: 20px;
                    outline: none;
                }
                QLineEdit:hover {
                    border-color: #adb5bd;
                }
                QLineEdit:focus {
                    border-color: #4A90E2;
                }
            """
        self.setStyleSheet(stylesheet)


class ModernKeySequenceEdit(QKeySequenceEdit):
    """Beautiful modern key sequence edit for shortcuts"""
    
    def __init__(self, parent=None):
        super().__init__(parent)
        self.apply_theme(is_dark=False)
        
        # Set maximum sequence length (usually 1 shortcut is enough)
        self.setMaximumSequenceLength(1)
        
        # Connect to sequence changed signal for debugging
        self.keySequenceChanged.connect(self._on_sequence_changed)
        
        # Set placeholder text to help users
        self.clear()

    def apply_theme(self, is_dark: bool):
        if is_dark:
            self.setStyleSheet("""
                QKeySequenceEdit {
                    border: 2px solid #555555;
                    border-radius: 6px;
                    padding: 8px;
                    font-size: 13px;
                    background-color: #2c2c2c;
                    color: #f0f0f0;
                    min-height: 20px;
                    outline: none;
                }
                QKeySequenceEdit:hover {
                    border-color: #777777;
                }
                QKeySequenceEdit:focus {
                    border-color: #4A90E2;
                    border-width: 3px;
                }
            """)
        else:
            self.setStyleSheet("""
                QKeySequenceEdit {
                    border: 2px solid #dee2e6;
                    border-radius: 6px;
                    padding: 8px;
                    font-size: 13px;
                    background-color: white;
                    color: black;
                    min-height: 20px;
                    outline: none;
                }
                QKeySequenceEdit:hover {
                    border-color: #adb5bd;
                }
                QKeySequenceEdit:focus {
                    border-color: #4A90E2;
                    border-width: 3px;
                }
            """)
    
    def _on_sequence_changed(self, sequence):
        """Debug callback for when key sequence changes"""
        pass  # Remove debug output
    
    def focusInEvent(self, event):
        """Override focus in event to show helper text"""
        super().focusInEvent(event)
        # Show visual indication that it's recording
        current_stylesheet = self.styleSheet()
        if "background-color: #2c2c2c" in current_stylesheet: # Dark mode
            self.setStyleSheet(current_stylesheet.replace(
                "border-color: #4A90E2;",
                "border-color: #28a745; background-color: #1a3c22;"
            ))
        else: # Light mode
            self.setStyleSheet(current_stylesheet.replace(
                "border-color: #4A90E2;",
                "border-color: #28a745; background-color: #f8fff9;"
            ))
    
    def focusOutEvent(self, event):
        """Override focus out event to reset styling"""
        super().focusOutEvent(event)
        # Reset to normal styling by re-applying theme
        current_stylesheet = self.styleSheet()
        is_dark = "background-color: #1a3c22" in current_stylesheet or "background-color: #2c2c2c" in current_stylesheet
        self.apply_theme(is_dark)
    
    @staticmethod
    def qt_to_windows_shortcut(qt_sequence: str) -> str:
        """Convert Qt key sequence to Windows hotkey format"""
        if not qt_sequence:
            return ""
        
        # Qt uses "Meta" for Windows key, convert to "Win"
        result = qt_sequence.replace("Meta+", "Win+")
        result = result.replace("Ctrl+", "Ctrl+")
        result = result.replace("Alt+", "Alt+")
        result = result.replace("Shift+", "Shift+")
        
        return result
    
    @staticmethod
    def windows_to_qt_shortcut(windows_sequence: str) -> str:
        """Convert Windows hotkey format to Qt key sequence"""
        if not windows_sequence:
            return ""
        
        # Convert "Win" to "Meta" for Qt
        result = windows_sequence.replace("Win+", "Meta+")
        result = result.replace("Windows+", "Meta+")
        
        return result


class ModernRadioButton(QRadioButton):
    """Beautiful modern radio button"""
    
    def __init__(self, text: str, parent=None):
        super().__init__(text, parent)
        self.apply_theme(is_dark=False)

    def apply_theme(self, is_dark: bool):
        if is_dark:
            stylesheet = """
                QRadioButton {
                    font-size: 13px;
                    color: #f0f0f0;
                    spacing: 8px;
                    outline: none;
                }
                QRadioButton:checked {
                    color: #ffffff;
                    font-weight: 500;
                }
                QRadioButton::indicator {
                    width: 16px;
                    height: 16px;
                }
                QRadioButton::indicator:unchecked {
                    border: 2px solid #555555;
                    border-radius: 8px;
                    background-color: #2c2c2c;
                }
                QRadioButton::indicator:unchecked:hover {
                    border-color: #4A90E2;
                }
                QRadioButton::indicator:checked {
                    border: 2px solid #4A90E2;
                    border-radius: 8px;
                    background-color: #4A90E2;
                }
            """
        else:
            stylesheet = """
                QRadioButton {
                    font-size: 13px;
                    color: #495057;
                    spacing: 8px;
                    outline: none;
                }
                QRadioButton:checked {
                    color: #2c3e50;
                    font-weight: 500;
                }
                QRadioButton::indicator {
                    width: 16px;
                    height: 16px;
                }
                QRadioButton::indicator:unchecked {
                    border: 2px solid #dee2e6;
                    border-radius: 8px;
                    background-color: white;
                }
                QRadioButton::indicator:unchecked:hover {
                    border-color: #4A90E2;
                }
                QRadioButton::indicator:checked {
                    border: 2px solid #4A90E2;
                    border-radius: 8px;
                    background-color: #4A90E2;
                }
            """
        self.setStyleSheet(stylesheet)


class ModernCheckBox(QCheckBox):
    """Beautiful modern check box"""
    
    def __init__(self, text: str, parent=None):
        super().__init__(text, parent)
        self.apply_theme(is_dark=False)

    def apply_theme(self, is_dark: bool):
        if is_dark:
            stylesheet = """
                QCheckBox {
                    font-size: 13px;
                    color: #f0f0f0;
                    spacing: 8px;
                    outline: none;
                }
                QCheckBox::indicator {
                    width: 16px;
                    height: 16px;
                }
                QCheckBox::indicator:unchecked {
                    border: 2px solid #555555;
                    border-radius: 4px;
                    background-color: #2c2c2c;
                }
                QCheckBox::indicator:unchecked:hover {
                    border-color: #4A90E2;
                }
                QCheckBox::indicator:checked {
                    border: 2px solid #4A90E2;
                    border-radius: 4px;
                    background-color: #4A90E2;
                }
                QCheckBox::indicator:checked:hover {
                    border-color: #5BA0F2;
                }
            """
        else:
            stylesheet = """
                QCheckBox {
                    font-size: 13px;
                    color: #495057;
                    spacing: 8px;
                    outline: none;
                }
                QCheckBox::indicator {
                    width: 16px;
                    height: 16px;
                }
                QCheckBox::indicator:unchecked {
                    border: 2px solid #dee2e6;
                    border-radius: 4px;
                    background-color: white;
                }
                QCheckBox::indicator:unchecked:hover {
                    border-color: #4A90E2;
                }
                QCheckBox::indicator:checked {
                    border: 2px solid #4A90E2;
                    border-radius: 4px;
                    background-color: #4A90E2;
                }
            """
        self.setStyleSheet(stylesheet)


class AudioTab(QWidget):
    """Audio settings tab"""
    
    def __init__(self, config_manager: ConfigManager, audio_manager: AudioManager = None, parent=None):
        super().__init__(parent)
        self.config_manager = config_manager
        # Use shared audio manager if provided, otherwise create new one
        self.audio_manager = audio_manager if audio_manager is not None else AudioManager()
        self.setup_ui()
        self.load_settings()
    
    def setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(20)  # Reduced spacing for compact scroll layout
        
        # Microphone selection
        mic_group = ModernGroupBox("Microphone Settings")
        mic_layout = QFormLayout(mic_group)
        mic_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        
        # Ensure form labels are visible
        mic_group.setStyleSheet("""
            QGroupBox {
                font-size: 14px;
                font-weight: 600;
                color: #2c3e50;
                border: 2px solid #dee2e6;
                border-radius: 8px;
                margin-top: 10px;
                padding-top: 10px;
                background-color: white;
            }
            QGroupBox::title {
                subcontrol-origin: margin;
                left: 10px;
                padding: 0 8px 0 8px;
                background-color: white;
                color: #2c3e50;
            }
            QLabel {
                color: #495057;
                font-size: 13px;
                font-weight: 500;
                background-color: transparent;
            }
        """)
        
        # Microphone selection row with refresh button
        mic_selection_layout = QHBoxLayout()
        self.mic_combo = ModernComboBox()
        self.refresh_devices()
        mic_selection_layout.addWidget(self.mic_combo)
        
        # Add refresh button
        self.refresh_button = QPushButton("Refresh")
        self.refresh_button.setIcon(get_white_button_icon('refresh', 16))
        self.refresh_button.setIconSize(QSize(16, 16))
        self.refresh_button.setStyleSheet("""
            QPushButton {
                background: #17a2b8;
                color: white;
                border: none;
                border-radius: 6px;
                padding: 8px 12px;
                font-size: 12px;
            }
            QPushButton:hover {
                background: #138496;
            }
        """)
        self.refresh_button.clicked.connect(self.refresh_devices)
        mic_selection_layout.addWidget(self.refresh_button)
        
        mic_layout.addRow("Select Microphone:", mic_selection_layout)
        
        # Add device change connection
        self.mic_combo.currentIndexChanged.connect(self.on_device_changed)
        
        # Auto-select active microphone section
        auto_select_layout = QHBoxLayout()
        self.auto_select_checkbox = ModernCheckBox("Auto-select active microphone")
        self.auto_select_checkbox.setIcon(get_button_icon('sound', 16))
        self.auto_select_checkbox.setIconSize(QSize(16, 16))
        self.auto_select_checkbox.setChecked(False)  # Default disabled
        self.auto_select_checkbox.toggled.connect(self.on_auto_select_toggled)
        auto_select_layout.addWidget(self.auto_select_checkbox)
        
        # Add "Detect Now" button
        self.detect_now_button = QPushButton("Detect Now")
        self.detect_now_button.setIcon(get_white_button_icon('refresh', 14))
        self.detect_now_button.setIconSize(QSize(14, 14))
        self.detect_now_button.setStyleSheet("""
            QPushButton {
                background: #28a745;
                color: white;
                border: none;
                border-radius: 4px;
                padding: 4px 8px;
                font-size: 11px;
                min-width: 60px;
            }
            QPushButton:hover {
                background: #218838;
            }
            QPushButton:disabled {
                background: #6c757d;
                color: #adb5bd;
            }
        """)
        self.detect_now_button.clicked.connect(self.detect_active_microphone_now)
        self.detect_now_button.setEnabled(False)  # Only enabled when auto-select is on
        auto_select_layout.addWidget(self.detect_now_button)
        auto_select_layout.addStretch()
        
        mic_layout.addRow("", auto_select_layout)
        
        # Help text for auto-select
        auto_select_help = QLabel("Detects microphone with audio activity. Use 'Detect Now' while speaking into your microphone.")
        auto_select_help.setStyleSheet("""
            color: #6c757d; 
            font-size: 11px;
            background-color: transparent;
        """)
        auto_select_help.setWordWrap(True)
        mic_layout.addRow("", auto_select_help)
        
        # Test button
        test_layout = QHBoxLayout()
        self.test_button = QPushButton("Test")
        self.test_button.setIcon(get_white_button_icon('test', 16))
        self.test_button.setIconSize(QSize(16, 16))
        self.test_button.setStyleSheet("""
            QPushButton {
                background: #28a745;
                color: white;
                border: none;
                border-radius: 6px;
                padding: 8px 12px;
                font-size: 13px;
            }
            QPushButton:hover {
                background: #218838;
            }
        """)
        self.test_button.clicked.connect(self.test_microphone)
        test_layout.addWidget(self.test_button)
        test_layout.addStretch()
        mic_layout.addRow("", test_layout)
        
        # Audio level meter
        self.level_label = QLabel("Audio Level:")
        self.level_bar = QProgressBar()
        self.level_bar.setRange(0, 100)
        self.level_bar.setValue(0)  # Initialize to 0
        self.level_bar.setStyleSheet("""
            QProgressBar {
                border: 2px solid #dee2e6;
                border-radius: 6px;
                background-color: #f8f9fa;
                height: 24px;
                text-align: center;
                color: #495057;
                font-weight: bold;
            }
            QProgressBar::chunk {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #28a745, stop:0.6 #28a745, stop:0.8 #ffc107, stop:1 #dc3545);
                border-radius: 4px;
                margin: 1px;
            }
        """)
        mic_layout.addRow(self.level_label, self.level_bar)
        
        layout.addWidget(mic_group)
        
        # Sound settings
        sound_group = ModernGroupBox("Sound Settings")
        sound_layout = QFormLayout(sound_group)
        sound_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        
        # Ensure form labels are visible
        sound_group.setStyleSheet("""
            QGroupBox {
                font-size: 14px;
                font-weight: 600;
                color: #2c3e50;
                border: 2px solid #dee2e6;
                border-radius: 8px;
                margin-top: 10px;
                padding-top: 10px;
                background-color: white;
            }
            QGroupBox::title {
                subcontrol-origin: margin;
                left: 10px;
                padding: 0 8px 0 8px;
                background-color: white;
                color: #2c3e50;
            }
            QLabel {
                color: #495057;
                font-size: 13px;
                font-weight: 500;
                background-color: transparent;
            }
        """)
        
        self.sounds_enabled_checkbox = ModernCheckBox("Enable notification sounds")
        sound_layout.addRow(self.sounds_enabled_checkbox)
        
        # Help text for sounds
        sound_help = QLabel("Play sounds when starting and stopping recording")
        sound_help.setStyleSheet("""
            color: #6c757d; 
            font-size: 11px;
            background-color: transparent;
        """)
        sound_help.setWordWrap(True)
        sound_layout.addRow("", sound_help)
        
        layout.addWidget(sound_group)

        # Refresh timer for level meter
        self.level_timer = QTimer()
        self.level_timer.timeout.connect(self.update_level_meter)
        
        # Microphone testing state
        self.is_testing = False
        self.test_audio_manager = None
        
        # Real-time device monitoring
        self.device_monitor_timer = QTimer()
        self.device_monitor_timer.timeout.connect(self.monitor_device_changes)
        self.device_monitor_timer.start(2000)  # Check every 2 seconds
        self.last_device_list = []
        
        # Add minimal space for scroll layout
        layout.addStretch()
    
    def on_device_changed(self, index):
        """Handle microphone device selection change"""
        if index >= 0:
            device_id = self.mic_combo.currentData()
            # Apply the device change immediately to the audio manager
            self.audio_manager.set_input_device(device_id)
            
            # Stop current monitoring and restart with new device
            try:
                self.audio_manager.stop_monitoring()
                if device_id is not None:
                    self.audio_manager.start_monitoring()
            except Exception as e:
                print(f"Warning: Could not switch to new microphone: {e}")
    
    def refresh_devices(self):
        """Refresh the list of available microphones"""
        # Store current selection
        current_device_id = self.mic_combo.currentData() if self.mic_combo.count() > 0 else None
        
        # Update device list in audio manager
        self.audio_manager.update_available_devices()
        
        # Clear and repopulate combo box
        self.mic_combo.clear()
        devices = self.audio_manager.get_available_devices()
        
        for device in devices:
            self.mic_combo.addItem(f"{device['name']}", device['id'])
        
        # Try to restore previous selection
        if current_device_id is not None:
            for i in range(self.mic_combo.count()):
                if self.mic_combo.itemData(i) == current_device_id:
                    self.mic_combo.setCurrentIndex(i)
                    break
        
        # If no devices found, show helpful message
        if len(devices) == 0:
            self.mic_combo.addItem("No microphones found", None)
        
        # Store current device list for comparison
        self.last_device_list = [device['id'] for device in devices]
    
    def test_microphone(self):
        """Test the selected microphone with continuous level monitoring"""
        if not self.is_testing:
            # Start testing
            self.is_testing = True
            self.test_button.setText("Stop Test")
            self.test_button.setStyleSheet("""
                QPushButton {
                    background: #dc3545;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    padding: 8px 16px;
                    font-size: 13px;
                }
                QPushButton:hover {
                    background: #c82333;
                }
            """)
            
            # Start audio level monitoring
            current_data = self.mic_combo.currentData()
            device_id = current_data if current_data is not None else None
            
            try:
                self.audio_manager.set_input_device(device_id)
                # Start audio monitoring for the test
                self.audio_manager.start_monitoring()
                # Connect to level updates
                self.audio_manager.audio_level_changed.connect(self.update_test_level_meter)
                self.level_timer.start(50)  # Update every 50ms
                
            except Exception as e:
                self.stop_testing()
                self.show_test_error(f"Error starting test: {str(e)}")
        else:
            # Stop testing
            self.stop_testing()
    
    def stop_testing(self):
        """Stop microphone testing"""
        self.is_testing = False
        self.test_button.setText("Test Mic")
        self.test_button.setIcon(get_white_button_icon('test', 16))
        self.test_button.setStyleSheet("""
            QPushButton {
                background: #28a745;
                color: white;
                border: none;
                border-radius: 6px;
                padding: 8px 16px;
                font-size: 13px;
            }
            QPushButton:hover {
                background: #218838;
            }
        """)
        
        # Stop level monitoring
        self.level_timer.stop()
        try:
            self.audio_manager.audio_level_changed.disconnect(self.update_test_level_meter)
        except:
            pass  # Ignore if not connected
        
        # Reset level bar
        self.level_bar.setValue(0)
    
    def update_test_level_meter(self, level: float):
        """Update the level meter during testing"""
        # Convert level to percentage (0-100)
        level_percent = min(100, int(level * 100))
        self.level_bar.setValue(level_percent)
    
    def show_test_error(self, error_msg: str):
        """Show error message for testing"""
        self.test_button.setText("Error")
        self.test_button.setStyleSheet("""
            QPushButton {
                background: #dc3545;
                color: white;
                border: none;
                border-radius: 6px;
                padding: 8px 16px;
                font-size: 13px;
            }
        """)
        # Reset after 3 seconds
        QTimer.singleShot(3000, self.stop_testing)
    
    def update_level_meter(self):
        """Update audio level meter (placeholder for general monitoring)"""
        if not self.is_testing:
            self.level_bar.setValue(0)
    
    def on_auto_select_toggled(self, checked: bool):
        """Handle auto-select checkbox toggle"""
        # Enable/disable the detect now button
        self.detect_now_button.setEnabled(checked)
        
        if checked:
            # Use a longer interval for automatic detection to be less intrusive
            self.device_monitor_timer.start(5000)  # Start monitoring every 5 seconds
            # Don't auto-detect immediately on toggle - let user use "Detect Now" button
        else:
            self.device_monitor_timer.stop()  # Stop monitoring
    
    def monitor_device_changes(self):
        """Monitor for device changes and update dropdown"""
        if not self.auto_select_checkbox.isChecked():
            return
            
        try:
            # Update device list
            self.audio_manager.update_available_devices()
            current_devices = self.audio_manager.get_available_devices()
            current_device_ids = [device['id'] for device in current_devices]
            
            # Check if device list has changed
            if current_device_ids != self.last_device_list:
                print("Device list changed, refreshing...")
                self.refresh_devices()
            
            # Only do intensive audio detection occasionally (every 3rd check ~ 15 seconds)
            if not hasattr(self, '_monitor_counter'):
                self._monitor_counter = 0
            self._monitor_counter += 1
            
            if self._monitor_counter >= 3:
                self._monitor_counter = 0
                # Try to auto-select the active microphone
                self.auto_select_active_microphone()
                
        except Exception as e:
            print(f"Error monitoring device changes: {e}")
    
    def auto_select_active_microphone(self):
        """Automatically select the currently active microphone"""
        if not self.auto_select_checkbox.isChecked():
            return
            
        try:
            # Get the active device ID using the audio manager
            active_device_id = self.audio_manager.get_active_device_id()
            
            if active_device_id is not None:
                # Find matching device in combo box
                for i in range(self.mic_combo.count()):
                    device_id = self.mic_combo.itemData(i)
                    
                    if device_id == active_device_id:
                        if self.mic_combo.currentIndex() != i:
                            device_name = self.mic_combo.itemText(i)
                            print(f"Auto-selecting microphone: {device_name}")
                            # Temporarily disconnect signal to avoid recursion
                            self.mic_combo.currentIndexChanged.disconnect()
                            self.mic_combo.setCurrentIndex(i)
                            self.mic_combo.currentIndexChanged.connect(self.on_device_changed)
                        break
        except Exception as e:
            print(f"Error auto-selecting microphone: {e}")
    
    def detect_active_microphone_now(self):
        """Manually trigger active microphone detection"""
        try:
            # Change button text to indicate detection is in progress
            self.detect_now_button.setText("Detecting...")
            self.detect_now_button.setEnabled(False)
            
            # Force a refresh of available devices first
            self.refresh_devices()
            
            # Try to detect the active microphone
            active_device_id = self.audio_manager.get_active_device_id()
            
            if active_device_id is not None:
                # Find matching device in combo box
                device_found = False
                for i in range(self.mic_combo.count()):
                    device_id = self.mic_combo.itemData(i)
                    
                    if device_id == active_device_id:
                        device_name = self.mic_combo.itemText(i)
                        print(f"Manually detected active microphone: {device_name}")
                        
                        # Temporarily disconnect signal to avoid recursion
                        self.mic_combo.currentIndexChanged.disconnect()
                        self.mic_combo.setCurrentIndex(i)
                        self.mic_combo.currentIndexChanged.connect(self.on_device_changed)
                        device_found = True
                        break
                
                if not device_found:
                    print("Active microphone detected but not found in dropdown")
            else:
                print("No active microphone detected - try speaking into your microphone and click 'Detect Now' again")
            
        except Exception as e:
            print(f"Error during manual detection: {e}")
        finally:
            # Reset button state
            self.detect_now_button.setText("Detect Now")
            self.detect_now_button.setEnabled(True)
    
    # Visualization controls moved to UI tab
    
    def load_settings(self):
        """Load audio settings from config"""
        device_id = self.config_manager.get_setting("audio/device_id")
        if device_id is not None:
            # Find and select the device in combo box
            for i in range(self.mic_combo.count()):
                if self.mic_combo.itemData(i) == device_id:
                    self.mic_combo.setCurrentIndex(i)
                    break
        
        # Load sounds setting
        sounds_enabled = self.config_manager.get_setting("audio/sounds_enabled", True)
        self.sounds_enabled_checkbox.setChecked(bool(sounds_enabled))
        
        # Load auto-select setting
        auto_select_enabled = self.config_manager.get_setting("audio/auto_select_mic", False)
        self.auto_select_checkbox.setChecked(bool(auto_select_enabled))
        
        # Update button state based on auto-select setting
        self.detect_now_button.setEnabled(bool(auto_select_enabled))
        
        # If auto-select is enabled, start monitoring
        if auto_select_enabled:
            self.device_monitor_timer.start(5000)
        # Visualization moved to UI tab
    
    def save_settings(self):
        """Save audio settings to config"""
        device_id = self.mic_combo.currentData()
        self.config_manager.set_setting("audio/device_id", device_id)
        
        # Save sounds setting
        self.config_manager.set_setting("audio/sounds_enabled", self.sounds_enabled_checkbox.isChecked())
        
        # Save auto-select setting
        self.config_manager.set_setting("audio/auto_select_mic", self.auto_select_checkbox.isChecked())
        # Visualization moved to UI tab
    
    def cleanup(self):
        """Clean up timers and resources"""
        try:
            if hasattr(self, 'device_monitor_timer'):
                self.device_monitor_timer.stop()
            if hasattr(self, 'level_timer'):
                self.level_timer.stop()
            self.stop_testing()
        except Exception as e:
            print(f"Error during AudioTab cleanup: {e}")


class WhisperTab(QWidget):
    """Whisper settings tab"""
    
    def __init__(self, config_manager: ConfigManager, parent=None):
        super().__init__(parent)
        self.config_manager = config_manager
        self.whisper_manager = WhisperManager()
        self.setup_ui()
        self.setup_model_connections()
        self.load_settings()
    
    def setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(20)  # Reduced spacing for compact scroll layout
        
        # Mode selection
        mode_group = ModernGroupBox("Transcription Mode")
        mode_layout = QVBoxLayout(mode_group)
        
        self.mode_group = QButtonGroup()
        self.api_radio = ModernRadioButton("OpenAI Whisper API (Fast, requires internet)")
        self.api_radio.setIcon(get_button_icon('api', 16))
        self.api_radio.setIconSize(QSize(16, 16))
        self.local_radio = ModernRadioButton("Local Whisper.cpp (Private, works offline)")
        self.local_radio.setIcon(get_button_icon('local', 16))
        self.local_radio.setIconSize(QSize(16, 16))
        
        self.mode_group.addButton(self.api_radio, 0)
        self.mode_group.addButton(self.local_radio, 1)
        
        mode_layout.addWidget(self.api_radio)
        mode_layout.addWidget(self.local_radio)
        
        # Connect mode change
        self.mode_group.buttonToggled.connect(self.on_mode_changed)
        
        layout.addWidget(mode_group)
        
        # Group boxes will use the ModernGroupBox theming system instead of static styles

        # API settings
        self.api_group = ModernGroupBox("API Settings")
        api_layout = QFormLayout(self.api_group)
        api_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        api_layout.setFormAlignment(Qt.AlignmentFlag.AlignVCenter)
        # API group will use ModernGroupBox theming
        
        # API key input with icon
        api_key_widget = QWidget()
        api_key_layout = QHBoxLayout(api_key_widget)
        api_key_layout.setContentsMargins(0, 0, 0, 0)
        api_key_layout.setSpacing(6)

        api_key_icon = QLabel()
        api_key_icon.setPixmap(get_button_icon('key', 16).pixmap(16, 16))
        api_key_layout.addWidget(api_key_icon)

        self.api_key_edit = ModernLineEdit("sk-...")
        self.api_key_edit.setEchoMode(QLineEdit.EchoMode.Password)
        api_key_layout.addWidget(self.api_key_edit)
        api_key_layout.addStretch()

        # Create properly aligned label for API key
        api_key_label = QLabel("OpenAI API Key:")
        api_key_label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        api_key_label.setMinimumHeight(36)  # Match input box height
        api_key_label.setStyleSheet("""
            QLabel {
                color: #495057;
                font-size: 13px;
                font-weight: 500;
                background-color: transparent;
            }
        """)
        
        api_layout.addRow(api_key_label, api_key_widget)

        # Helper text for API key
        api_help = QLabel("Get your API key from: https://platform.openai.com/api-keys")
        api_help.setStyleSheet("""
            color: #6c757d;
            font-size: 11px;
            background-color: transparent;
            margin-left: 22px;
        """)
        api_help.setWordWrap(True)
        api_layout.addRow("", api_help)
        
        layout.addWidget(self.api_group)
        
        # Local model settings (only show if Faster-Whisper is available)
        from .whisper_manager import FASTER_WHISPER_AVAILABLE

        self.local_group = ModernGroupBox("Local Model Settings")
        local_layout = QFormLayout(self.local_group)
        local_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        local_layout.setFormAlignment(Qt.AlignmentFlag.AlignVCenter)
        # Local group will use ModernGroupBox theming
        
        if not FASTER_WHISPER_AVAILABLE:
            # Show message that local mode is not available
            not_available_label = QLabel("Local models not available.\nFaster-Whisper package not installed.\nPlease install it or use API mode.")
            not_available_label.setStyleSheet("""
                color: #856404;
                background-color: #fff3cd;
                border: 1px solid #ffeaa7;
                border-radius: 6px;
                padding: 10px;
                font-size: 13px;
            """)
            local_layout.addRow(not_available_label)
        
        # Only show model selection UI if Faster-Whisper is available
        if FASTER_WHISPER_AVAILABLE:
            # Category dropdown for filtering models with icon
            category_widget = QWidget()
            category_layout = QHBoxLayout(category_widget)
            category_layout.setContentsMargins(0, 0, 0, 0)
            category_layout.setSpacing(6)

            category_icon = QLabel()
            category_icon.setPixmap(get_button_icon('category', 16).pixmap(16, 16))
            category_layout.addWidget(category_icon)

            self.category_combo = ModernComboBox()
            self.populate_category_combo()
            self.category_combo.currentTextChanged.connect(self.on_category_changed)
            category_layout.addWidget(self.category_combo)
            category_layout.addStretch()

            # Create properly aligned label for category
            category_label = QLabel("Model Category:")
            category_label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
            category_label.setMinimumHeight(36)  # Match combo box height
            category_label.setStyleSheet("""
                QLabel {
                    color: #495057;
                    font-size: 13px;
                    font-weight: 500;
                    background-color: transparent;
                }
            """)
            
            local_layout.addRow(category_label, category_widget)

            # Helper text for category selection
            category_help = QLabel("Choose a category to filter available models by size and performance")
            category_help.setStyleSheet("""
                color: #6c757d;
                font-size: 11px;
                background-color: transparent;
                margin-left: 22px;
            """)
            category_help.setWordWrap(True)
            local_layout.addRow("", category_help)

            # Model dropdown for selecting specific model with icon
            model_widget = QWidget()
            model_layout = QHBoxLayout(model_widget)
            model_layout.setContentsMargins(0, 0, 0, 0)
            model_layout.setSpacing(6)

            model_icon = QLabel()
            model_icon.setPixmap(get_button_icon('brain', 16).pixmap(16, 16))
            model_layout.addWidget(model_icon)

            self.model_combo = ModernComboBox()
            self.populate_model_combo()
            self.model_combo.currentIndexChanged.connect(self.on_model_combo_changed)
            model_layout.addWidget(self.model_combo)
            model_layout.addStretch()

            # Create properly aligned label for model selection
            model_label = QLabel("Select Model:")
            model_label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
            model_label.setMinimumHeight(36)  # Match combo box height
            model_label.setStyleSheet("""
                QLabel {
                    color: #495057;
                    font-size: 13px;
                    font-weight: 500;
                    background-color: transparent;
                }
            """)
            
            local_layout.addRow(model_label, model_widget)

            # Helper text for model selection
            model_help = QLabel("Pick a specific Whisper model for transcription based on your needs")
            model_help.setStyleSheet("""
                color: #6c757d;
                font-size: 11px;
                background-color: transparent;
                margin-left: 22px;
            """)
            model_help.setWordWrap(True)
            local_layout.addRow("", model_help)

            # Removed download UI as per guidance
        
        layout.addWidget(self.local_group)
        
        # Add minimal space for scroll layout
        layout.addStretch()
    
    def refresh_models_display(self):
        """Download UI removed; nothing to refresh here"""
        return
    
    
    def create_integrated_model_widget(self, model_info: dict):
        """Deprecated: download UI removed"""
        return QWidget()
    
    def setup_model_connections(self):
        """Deprecated: download manager removed"""
        return

    def populate_category_combo(self):
        """Fill the category dropdown with available categories"""
        categories = self.whisper_manager.get_model_categories()
        self.category_combo.clear()
        for category in categories:
            self.category_combo.addItem(category)
        # Set default to "All" 
        self.category_combo.setCurrentText("All")
    
    def populate_model_combo(self, category: str = "All"):
        """Fill the model dropdown with models from selected category"""
        current_model = self.model_combo.currentText() if hasattr(self, 'model_combo') else ""
        # Only get LOCAL models, not API models like whisper-1
        models = self.whisper_manager.get_models_by_category(category)
        # Filter out any API-only models
        local_models = [model for model in models if model != "whisper-1"]
        
        self.model_combo.clear()
        for model in local_models:
            # Get model info to show size
            model_info = self.whisper_manager.get_model_info(model)
            size = model_info.get('size', 'Unknown')
            memory = model_info.get('memory', 'Unknown')
            # Display format: "model_name (size, memory)"
            display_text = f"{model} ({size}, {memory})"
            # Store actual model name as item data
            self.model_combo.addItem(display_text, model)
        
        # Try to restore previous selection if it's still available
        if current_model and current_model in local_models:
            # Find the item with matching data (actual model name)
            for i in range(self.model_combo.count()):
                if self.model_combo.itemData(i) == current_model:
                    self.model_combo.setCurrentIndex(i)
                    break
        elif local_models:
            # If no previous selection or it's not available, select first model
            self.model_combo.setCurrentIndex(0)

    def on_category_changed(self, category: str):
        """Handle category dropdown selection change"""
        # Update the model dropdown to show models from selected category
        self.populate_model_combo(category)
    
    def on_model_combo_changed(self, index: int):
        """Handle model dropdown selection change"""
        # Get actual model name from item data
        model_name = self.model_combo.itemData(index)
        if model_name:
            # Persist immediately and update whisper manager
            self.config_manager.set_setting("whisper/local_model", model_name)
            self.whisper_manager.set_local_model(model_name)
    
    def process_ui_events(self):
        """Process UI events to keep interface responsive"""
        from PySide6.QtWidgets import QApplication
        QApplication.processEvents()
    
    def on_model_selected(self, model_info: dict):
        """Deprecated: selection via download list removed"""
        return
    
    def download_model(self, model_file: str):
        """Deprecated: download removed"""
        return
    
    def cancel_download(self, model_file: str):
        """Deprecated: download removed"""
        return
    
    def delete_model(self, model_file: str):
        """Deprecated: download removed"""
        return
    
    def on_model_status_changed(self, model_name: str, status: str):
        """Deprecated: download removed"""
        return
    
    def on_download_progress(self, model_name: str, progress: int):
        """Deprecated: download removed"""
        return
    
    def on_mode_changed(self, button, checked):
        """Handle mode selection change"""
        if checked:
            if button == self.api_radio:
                self.api_group.setEnabled(True)
                self.local_group.setEnabled(False)
                self.whisper_manager.set_mode("api")
            else:
                self.api_group.setEnabled(False)
                self.local_group.setEnabled(True)
                self.whisper_manager.set_mode("local")
    
    
    def load_settings(self):
        """Load Whisper settings from config"""
        mode = self.config_manager.get_setting("whisper/mode", "api")
        if mode == "api":
            self.api_radio.setChecked(True)
        else:
            self.local_radio.setChecked(True)
        
        api_key = self.config_manager.get_setting("whisper/api_key", "")
        self.api_key_edit.setText(api_key)
        
        # Load selected model for local mode
        selected_model = self.config_manager.get_setting("whisper/local_model", "base")
        # Sync dropdowns if present
        if hasattr(self, 'category_combo') and hasattr(self, 'model_combo'):
            # First determine which category contains the selected model
            found_category = "All"
            for category in self.whisper_manager.get_model_categories():
                if selected_model in self.whisper_manager.get_models_by_category(category):
                    if category != "All":  # Prefer specific category over "All"
                        found_category = category
                        break
            
            # Set the category dropdown
            self.category_combo.setCurrentText(found_category)
            # Update model dropdown for that category
            self.populate_model_combo(found_category)
            # Set the specific model by finding matching item data
            for i in range(self.model_combo.count()):
                if self.model_combo.itemData(i) == selected_model:
                    self.model_combo.setCurrentIndex(i)
                    break
    
    def save_settings(self):
        """Save Whisper settings to config"""
        mode = "api" if self.api_radio.isChecked() else "local"
        self.config_manager.set_setting("whisper/mode", mode)
        self.config_manager.set_setting("whisper/api_key", self.api_key_edit.text())
        
        # Save selected local model from dropdown
        if hasattr(self, 'model_combo') and self.model_combo.currentIndex() >= 0:
            model_name = self.model_combo.currentData()  # Get actual model name from item data
            if model_name:
                self.config_manager.set_setting("whisper/local_model", model_name)
                if mode == "local":
                    self.whisper_manager.set_local_model(model_name)
    
    
    # Removed fixed size constraints to allow better layout flexibility
    
    def closeEvent(self, event):
        """Handle dialog close event"""
        # Stop UI refresh timer
        if hasattr(self, 'ui_refresh_timer'):
            self.ui_refresh_timer.stop()
        event.accept()


class OutputTab(QWidget):
    """Output settings tab"""
    
    def __init__(self, config_manager: ConfigManager, parent=None):
        super().__init__(parent)
        self.config_manager = config_manager
        self.setup_ui()
        self.load_settings()
    
    def setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(20)  # Reduced spacing for compact scroll layout
        
        # Output behavior
        output_group = ModernGroupBox("Output Behavior")
        output_layout = QVBoxLayout(output_group)
        
        self.output_group = QButtonGroup()
        self.copy_only = ModernRadioButton("Copy to clipboard only")
        self.copy_only.setIcon(get_button_icon('clipboard', 16))
        self.copy_only.setIconSize(QSize(16, 16))
        self.paste_only = ModernRadioButton("Paste to active app only")
        self.paste_only.setIcon(get_button_icon('paste', 16))
        self.paste_only.setIconSize(QSize(16, 16))
        self.copy_and_paste = ModernRadioButton("Copy and paste")
        self.copy_and_paste.setIcon(get_button_icon('clipboard', 16))
        self.copy_and_paste.setIconSize(QSize(16, 16))
        self.display_only = ModernRadioButton("Display only (no copy/paste)")
        self.display_only.setIcon(get_button_icon('eye', 16))
        self.display_only.setIconSize(QSize(16, 16))
        
        self.output_group.addButton(self.copy_only, 0)
        self.output_group.addButton(self.paste_only, 1)
        self.output_group.addButton(self.copy_and_paste, 2)
        self.output_group.addButton(self.display_only, 3)
        
        output_layout.addWidget(self.copy_only)
        output_layout.addWidget(self.paste_only)
        output_layout.addWidget(self.copy_and_paste)
        output_layout.addWidget(self.display_only)
        
        layout.addWidget(output_group)
        
        # Additional options
        options_group = ModernGroupBox("Additional Options")
        options_layout = QVBoxLayout(options_group)
        
        self.silent_mode = ModernCheckBox("Silent mode (hide transcription text)")
        self.silent_mode.setIcon(get_button_icon('silent', 16))
        self.silent_mode.setIconSize(QSize(16, 16))
        
        self.auto_clear = ModernCheckBox("Auto-clear after copying/pasting")
        self.auto_clear.setIcon(get_button_icon('trash', 16))
        self.auto_clear.setIconSize(QSize(16, 16))
        
        # Auto-clear delay setting
        auto_clear_layout = QHBoxLayout()
        auto_clear_layout.addWidget(self.auto_clear)
        
        # Add delay input next to auto-clear checkbox
        delay_label = QLabel("after")
        delay_label.setStyleSheet("color: #6c757d; font-size: 12px; margin-left: 10px;")
        
        self.auto_clear_delay = QLineEdit()
        self.auto_clear_delay.setFixedWidth(40)
        self.auto_clear_delay.setText("5")
        self.auto_clear_delay.setValidator(QIntValidator(1, 999))  # Only allow integers 1-999
        self.auto_clear_delay.setStyleSheet("""
            QLineEdit {
                border: 1px solid #dee2e6;
                border-radius: 4px;
                padding: 4px;
                font-size: 12px;
                background-color: white;
                color: black;
            }
            QLineEdit:hover {
                border-color: #adb5bd;
            }
            QLineEdit:focus {
                border-color: #4A90E2;
            }
        """)
        
        seconds_label = QLabel("seconds")
        seconds_label.setStyleSheet("color: #6c757d; font-size: 12px;")
        
        auto_clear_layout.addWidget(delay_label)
        auto_clear_layout.addWidget(self.auto_clear_delay)
        auto_clear_layout.addWidget(seconds_label)
        auto_clear_layout.addStretch()
        
        # Connect auto-clear checkbox to enable/disable delay input
        self.auto_clear.toggled.connect(self._update_auto_clear_delay_state)
        
        options_layout.addWidget(self.silent_mode)
        options_layout.addLayout(auto_clear_layout)
        
        # Help text for auto-clear
        auto_clear_help = QLabel("Note: 'Paste to active app only' always clears clipboard immediately regardless of this setting")
        auto_clear_help.setStyleSheet("""
            color: #856404;
            background-color: #fff3cd;
            border: 1px solid #ffeaa7;
            border-radius: 4px;
            padding: 8px;
            font-size: 11px;
        """)
        auto_clear_help.setWordWrap(True)
        options_layout.addWidget(auto_clear_help)
        
        layout.addWidget(options_group)
        
        # Add minimal space for scroll layout
        layout.addStretch()
    
    def _update_auto_clear_delay_state(self, checked: bool):
        """Enable/disable auto-clear delay input based on checkbox state"""
        self.auto_clear_delay.setEnabled(checked)
    
    def load_settings(self):
        """Load output settings from config"""
        output_mode = self.config_manager.get_setting("output/mode", 0)
        buttons = [self.copy_only, self.paste_only, self.copy_and_paste, self.display_only]
        if 0 <= output_mode < len(buttons):
            buttons[output_mode].setChecked(True)
        
        self.silent_mode.setChecked(self.config_manager.get_setting("output/silent_mode", False))
        auto_clear_enabled = self.config_manager.get_setting("output/auto_clear", False)
        self.auto_clear.setChecked(auto_clear_enabled)
        
        # Load auto-clear delay
        auto_clear_delay = self.config_manager.get_setting("output/auto_clear_delay", 5)
        self.auto_clear_delay.setText(str(auto_clear_delay))
        
        # Update delay input state based on checkbox
        self._update_auto_clear_delay_state(auto_clear_enabled)
    
    def save_settings(self):
        """Save output settings to config"""
        checked_button = self.output_group.checkedId()
        self.config_manager.set_setting("output/mode", checked_button)
        self.config_manager.set_setting("output/silent_mode", self.silent_mode.isChecked())
        self.config_manager.set_setting("output/auto_clear", self.auto_clear.isChecked())
        
        # Save auto-clear delay (validate input)
        try:
            delay_value = int(self.auto_clear_delay.text())
            # Ensure delay is between 1 and 999 seconds
            delay_value = max(1, min(999, delay_value))
            self.config_manager.set_setting("output/auto_clear_delay", delay_value)
        except ValueError:
            # If invalid input, use default of 5 seconds
            self.config_manager.set_setting("output/auto_clear_delay", 5)


class SettingsDialog(QDialog):
    """Beautiful settings dialog with tabbed interface"""
    
    # Signal emitted when settings are saved
    settings_saved = Signal()
    
    # Theme color definitions
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
    
    def __init__(self, parent=None, config_manager=None):
        super().__init__(parent)
        # Use shared config manager from parent if provided, otherwise create new one
        self.config_manager = config_manager if config_manager is not None else ConfigManager()
        self.setup_ui()
        self.setModal(True)
        # Apply initial theme
        initial_theme = self.config_manager.get_setting("ui/theme", "white")
        self.apply_theme(initial_theme)
        
    def setup_ui(self):
        self.setWindowTitle("QuillScribe Settings")
        self.setFixedSize(600, 500)  # Fixed compact size - much shorter vertically
        self.setWindowFlags(Qt.WindowType.Dialog | Qt.WindowType.WindowCloseButtonHint)
        
        layout = QVBoxLayout(self)
        layout.setSpacing(15)  # Reduced spacing for compact layout
        layout.setContentsMargins(15, 15, 15, 15)  # Reduced margins for compact layout
        
        # Title - smaller for compact layout
        title = QLabel("Settings")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        title.setStyleSheet("""
            QLabel {
                color: #2c3e50;
                font-size: 18px;
                font-weight: 400;
                margin-bottom: 5px;
            }
        """)
        layout.addWidget(title)
        
        # Tab widget - compact size
        self.tabs = QTabWidget()
        self.tabs.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        self.tabs.setMaximumHeight(350)  # Reduced height for compact layout
        # Tab styling will be set by apply_tab_theme method
        self.apply_tab_theme("white")
        
        # Create tabs - pass audio manager to audio tab for shared state
        audio_manager = getattr(self.parent(), 'audio_manager', None) if self.parent() else None
        self.audio_tab = AudioTab(self.config_manager, audio_manager)
        self.whisper_tab = WhisperTab(self.config_manager)
        self.output_tab = OutputTab(self.config_manager)
        self.ui_tab = UITab(self.config_manager)
        
        # Wrap each tab in a scroll area for compact layout
        self.audio_scroll = self._create_scroll_area(self.audio_tab)
        self.whisper_scroll = self._create_scroll_area(self.whisper_tab)
        self.output_scroll = self._create_scroll_area(self.output_tab)
        self.ui_scroll = self._create_scroll_area(self.ui_tab)
        
        self.tabs.addTab(self.audio_scroll, "Audio")
        self.tabs.setTabIcon(self.tabs.indexOf(self.audio_scroll), get_icon('audio', 16))
        self.tabs.addTab(self.whisper_scroll, "Whisper")
        self.tabs.setTabIcon(self.tabs.indexOf(self.whisper_scroll), get_icon('brain', 16))
        self.tabs.addTab(self.output_scroll, "Output")
        self.tabs.setTabIcon(self.tabs.indexOf(self.output_scroll), get_icon('clipboard', 16))
        self.tabs.addTab(self.ui_scroll, "UI Settings")
        self.tabs.setTabIcon(self.tabs.indexOf(self.ui_scroll), get_icon('settings', 16))
        
        layout.addWidget(self.tabs)
        
        # Buttons with compact spacing
        button_layout = QHBoxLayout()
        button_layout.setSpacing(10)  # Reduced button spacing for compact layout
        button_layout.addStretch()
        
        self.cancel_button = QPushButton("Cancel")
        self.cancel_button.setIcon(get_white_button_icon('cancel', 16))
        self.cancel_button.setIconSize(QSize(16, 16))
        self.cancel_button.setStyleSheet("""
            QPushButton {
                background: #6c757d;
                color: white;
                border: none;
                border-radius: 6px;
                padding: 10px 20px 10px 36px;
                font-size: 14px;
                min-width: 80px;
                text-align: left;
            }
            QPushButton:hover {
                background: #5a6268;
            }
        """)
        
        self.save_button = QPushButton("Save Settings")
        self.save_button.setIcon(get_white_button_icon('save', 16))
        self.save_button.setIconSize(QSize(16, 16))
        self.save_button.setStyleSheet("""
            QPushButton {
                background: #4A90E2;
                color: white;
                border: none;
                border-radius: 6px;
                padding: 10px 20px;
                font-size: 14px;
                min-width: 80px;
                text-align: center;
            }
            QPushButton:hover {
                background: #357ABD;
            }
        """)
        
        button_layout.addWidget(self.cancel_button)
        button_layout.addWidget(self.save_button)
        
        layout.addLayout(button_layout)
        
        # Connect buttons
        self.cancel_button.clicked.connect(self.reject)
        self.save_button.clicked.connect(self.save_and_close)
    
    def _create_scroll_area(self, widget):
        """Create a scroll area with custom themed scrollbars for the given widget"""
        scroll_area = QScrollArea()
        scroll_area.setWidget(widget)
        scroll_area.setWidgetResizable(True)
        scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        
        # Apply custom scrollbar styling (will be updated by theme)
        self._apply_scrollbar_theme(scroll_area, "white")
        
        return scroll_area
    
    def _apply_scrollbar_theme(self, scroll_area, theme_name):
        """Apply themed styling to scrollbar"""
        colors = self._get_theme_colors(theme_name)
        is_dark = self._is_dark_color(colors["primary"])
        
        if is_dark:
            # Dark theme scrollbar
            scrollbar_bg = "#3c3c3c"
            scrollbar_handle = "#666666"
            scrollbar_handle_hover = "#777777"
            scrollbar_handle_pressed = "#555555"
        else:
            # Light theme scrollbar
            scrollbar_bg = "#f8f9fa"
            scrollbar_handle = "#ced4da"
            scrollbar_handle_hover = "#adb5bd"
            scrollbar_handle_pressed = "#6c757d"
        
        scroll_area.setStyleSheet(f"""
            /* Ensure the scroll area and its viewport use the theme background */
            QScrollArea {{
                border: none;
                background-color: {colors["primary"]};
            }}
            QScrollArea > QWidget#qt_scrollarea_viewport {{
                background-color: {colors["primary"]};
            }}
            QWidget#qt_scrollarea_viewport {{
                background-color: {colors["primary"]};
            }}
            QScrollBar:vertical {{
                background: {scrollbar_bg};
                width: 12px;
                border: none;
                border-radius: 6px;
                margin: 0px;
            }}
            QScrollBar::handle:vertical {{
                background: {scrollbar_handle};
                min-height: 20px;
                border-radius: 6px;
                margin: 2px;
            }}
            QScrollBar::handle:vertical:hover {{
                background: {scrollbar_handle_hover};
            }}
            QScrollBar::handle:vertical:pressed {{
                background: {scrollbar_handle_pressed};
            }}
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
                border: none;
                background: none;
                height: 0px;
            }}
            QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{
                background: none;
            }}
        """)
    
    def save_and_close(self):
        """Save all settings and close dialog"""
        try:
            self.audio_tab.save_settings()
            self.whisper_tab.save_settings()
            self.output_tab.save_settings()
            self.ui_tab.save_settings()
            self.config_manager.save_settings()
            
            # Emit signal to notify main window that settings were saved
            self.settings_saved.emit()
            
            self.accept()
        except Exception as e:
            # Could show an error dialog here
            print(f"Error saving settings: {e}")
    
    def apply_theme(self, theme_name):
        """Apply the selected theme to the dialog background and all group boxes"""
        colors = self._get_theme_colors(theme_name)
        
        # Apply to dialog background
        self.setStyleSheet(f"""
            QDialog {{
                background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                    stop:0 {colors["primary"]}, stop:1 {colors["secondary"]});
                color: {colors["text_primary"]};
            }}
        """)
        
        # Apply to all group boxes in all tabs
        self._apply_theme_to_group_boxes(colors)

        # Apply theme to all modern widgets
        is_dark = self._is_dark_color(colors["primary"])
        # ComboBox needs accent/border injection
        for combo in self.findChildren(ModernComboBox):
            combo.apply_theme(is_dark, colors["accent"], colors["border"])
        # Other widgets keep existing signature
        for widget in self.findChildren(ModernLineEdit):
            widget.apply_theme(is_dark)
        for widget in self.findChildren(ModernKeySequenceEdit):
            widget.apply_theme(is_dark)
        for widget in self.findChildren(ModernRadioButton):
            widget.apply_theme(is_dark)
        for widget in self.findChildren(ModernCheckBox):
            widget.apply_theme(is_dark)
        
        # Apply theme to tabs
        self.apply_tab_theme(theme_name)
        
        # Apply theme to scrollbars
        if hasattr(self, 'audio_scroll'):
            self._apply_scrollbar_theme(self.audio_scroll, theme_name)
        if hasattr(self, 'whisper_scroll'):
            self._apply_scrollbar_theme(self.whisper_scroll, theme_name)
        if hasattr(self, 'output_scroll'):
            self._apply_scrollbar_theme(self.output_scroll, theme_name)
        if hasattr(self, 'ui_scroll'):
            self._apply_scrollbar_theme(self.ui_scroll, theme_name)

        # Apply theme to icons
        self._apply_icon_theme(is_dark)

        # Explicitly set background for tab content widgets to match theme
        # This ensures areas not covered by group boxes don't appear dark
        try:
            if hasattr(self, 'audio_tab'):
                self.audio_tab.setStyleSheet(f"background-color: {colors['primary']};")
            if hasattr(self, 'whisper_tab'):
                self.whisper_tab.setStyleSheet(f"background-color: {colors['primary']};")
            if hasattr(self, 'output_tab'):
                self.output_tab.setStyleSheet(f"background-color: {colors['primary']};")
            if hasattr(self, 'ui_tab'):
                self.ui_tab.setStyleSheet(f"background-color: {colors['primary']};")
        except Exception:
            pass
    
    def _apply_theme_to_group_boxes(self, colors):
        """Apply theme to all ModernGroupBox instances"""
        # Find all ModernGroupBox widgets in the dialog
        for widget in self.findChildren(ModernGroupBox):
            widget.apply_theme(colors["primary"], colors["secondary"])
    
    def _darken_color(self, hex_color, factor=0.15):
        """Darken a hex color by the given factor (0.0 to 1.0)"""
        # Remove # if present
        hex_color = hex_color.lstrip('#')
        
        # Convert to RGB
        r = int(hex_color[0:2], 16)
        g = int(hex_color[2:4], 16)
        b = int(hex_color[4:6], 16)
        
        # Darken by reducing values
        r = int(r * (1 - factor))
        g = int(g * (1 - factor))
        b = int(b * (1 - factor))
        
        # Convert back to hex
        return f"#{r:02x}{g:02x}{b:02x}"
    
    def _lighten_color(self, hex_color, factor=0.15):
        """Lighten a hex color by the given factor (0.0 to 1.0)"""
        # Remove # if present
        hex_color = hex_color.lstrip('#')
        
        # Convert to RGB
        r = int(hex_color[0:2], 16)
        g = int(hex_color[2:4], 16)
        b = int(hex_color[4:6], 16)
        
        # Lighten by increasing values towards 255
        r = int(r + (255 - r) * factor)
        g = int(g + (255 - g) * factor)
        b = int(b + (255 - b) * factor)
        
        # Convert back to hex
        return f"#{r:02x}{g:02x}{b:02x}"
    
    def _apply_icon_theme(self, is_dark: bool):
        """Apply appropriate icon colors based on dark/light theme"""
        # Update tab icons
        if hasattr(self, 'tabs'):
            # Audio tab
            audio_index = None
            for i in range(self.tabs.count()):
                if self.tabs.tabText(i) == "Audio":
                    audio_index = i
                    break
            if audio_index is not None:
                if is_dark:
                    self.tabs.setTabIcon(audio_index, get_icon('audio', 16, QColor(255, 255, 255)))
                else:
                    self.tabs.setTabIcon(audio_index, get_icon('audio', 16))
            
            # Whisper tab
            whisper_index = None
            for i in range(self.tabs.count()):
                if self.tabs.tabText(i) == "Whisper":
                    whisper_index = i
                    break
            if whisper_index is not None:
                if is_dark:
                    self.tabs.setTabIcon(whisper_index, get_icon('brain', 16, QColor(255, 255, 255)))
                else:
                    self.tabs.setTabIcon(whisper_index, get_icon('brain', 16))
            
            # Output tab
            output_index = None
            for i in range(self.tabs.count()):
                if self.tabs.tabText(i) == "Output":
                    output_index = i
                    break
            if output_index is not None:
                if is_dark:
                    self.tabs.setTabIcon(output_index, get_icon('clipboard', 16, QColor(255, 255, 255)))
                else:
                    self.tabs.setTabIcon(output_index, get_icon('clipboard', 16))
            
            # UI Settings tab
            ui_index = None
            for i in range(self.tabs.count()):
                if self.tabs.tabText(i) == "UI Settings":
                    ui_index = i
                    break
            if ui_index is not None:
                if is_dark:
                    self.tabs.setTabIcon(ui_index, get_icon('settings', 16, QColor(255, 255, 255)))
                else:
                    self.tabs.setTabIcon(ui_index, get_icon('settings', 16))
        
        # Update button icons (these are already using get_white_button_icon for dark backgrounds)
        # The cancel and save buttons already use white icons on their dark backgrounds, so no change needed
        
        # Update icons in tab content - these need to be updated based on theme
        try:
            # Audio tab icons
            if hasattr(self, 'audio_tab'):
                # Find and update refresh button
                for button in self.audio_tab.findChildren(QPushButton):
                    if hasattr(button, 'objectName') and 'refresh' in str(button.objectName()).lower():
                        if is_dark:
                            button.setIcon(get_white_button_icon('refresh', 16))
                        else:
                            button.setIcon(get_button_icon('refresh', 16))
                    elif 'detect' in button.text().lower():
                        if is_dark:
                            button.setIcon(get_white_button_icon('refresh', 14))
                        else:
                            button.setIcon(get_button_icon('refresh', 14))
                
                # Find and update checkbox icons
                for checkbox in self.audio_tab.findChildren(ModernCheckBox):
                    if 'auto' in checkbox.text().lower():
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('sound', 16))
                        else:
                            checkbox.setIcon(get_button_icon('sound', 16))
            
            # Whisper tab icons
            if hasattr(self, 'whisper_tab'):
                # Find and update test buttons
                for button in self.whisper_tab.findChildren(QPushButton):
                    if 'test' in button.text().lower():
                        if is_dark:
                            button.setIcon(get_white_button_icon('test', 16))
                        else:
                            button.setIcon(get_button_icon('test', 16))
                
                # Find and update radio button icons
                for radio in self.whisper_tab.findChildren(ModernRadioButton):
                    if 'api' in radio.text().lower():
                        if is_dark:
                            radio.setIcon(get_white_button_icon('api', 16))
                        else:
                            radio.setIcon(get_button_icon('api', 16))
                    elif 'local' in radio.text().lower():
                        if is_dark:
                            radio.setIcon(get_white_button_icon('local', 16))
                        else:
                            radio.setIcon(get_button_icon('local', 16))
            
            # Output tab icons
            if hasattr(self, 'output_tab'):
                # Find and update radio button icons
                for radio in self.output_tab.findChildren(ModernRadioButton):
                    text = radio.text().lower()
                    if 'copy' in text and 'paste' not in text:
                        if is_dark:
                            radio.setIcon(get_white_button_icon('clipboard', 16))
                        else:
                            radio.setIcon(get_button_icon('clipboard', 16))
                    elif 'paste' in text and 'copy' not in text:
                        if is_dark:
                            radio.setIcon(get_white_button_icon('paste', 16))
                        else:
                            radio.setIcon(get_button_icon('paste', 16))
                    elif 'copy' in text and 'paste' in text:
                        if is_dark:
                            radio.setIcon(get_white_button_icon('clipboard', 16))
                        else:
                            radio.setIcon(get_button_icon('clipboard', 16))
                    elif 'display' in text:
                        if is_dark:
                            radio.setIcon(get_white_button_icon('eye', 16))
                        else:
                            radio.setIcon(get_button_icon('eye', 16))
                
                # Find and update checkbox icons
                for checkbox in self.output_tab.findChildren(ModernCheckBox):
                    text = checkbox.text().lower()
                    if 'silent' in text:
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('silent', 16))
                        else:
                            checkbox.setIcon(get_button_icon('silent', 16))
                    elif 'auto clear' in text or 'clear' in text:
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('trash', 16))
                        else:
                            checkbox.setIcon(get_button_icon('trash', 16))
            
            # UI tab icons
            if hasattr(self, 'ui_tab'):
                # Find and update checkbox icons
                for checkbox in self.ui_tab.findChildren(ModernCheckBox):
                    text = checkbox.text().lower()
                    if 'compact' in text:
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('compact', 16))
                        else:
                            checkbox.setIcon(get_button_icon('compact', 16))
                    elif 'title' in text or 'titlebar' in text:
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('window', 16))
                        else:
                            checkbox.setIcon(get_button_icon('window', 16))
                    elif 'waveform' in text:
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('zap', 16))
                        else:
                            checkbox.setIcon(get_button_icon('zap', 16))
                    elif 'minimize' in text:
                        if is_dark:
                            checkbox.setIcon(get_white_button_icon('window', 16))
                        else:
                            checkbox.setIcon(get_button_icon('window', 16))
                
                # Update theme icon in the UI tab
                if hasattr(self.ui_tab, 'findChildren'):
                    for label in self.ui_tab.findChildren(QLabel):
                        # Look for the theme icon label
                        if hasattr(label, 'pixmap') and label.pixmap() is not None:
                            # Check if this is likely the theme icon (has a pixmap and is near theme-related text)
                            parent = label.parent()
                            if parent and any('theme' in str(child.text()).lower() for child in parent.findChildren(QLabel) if hasattr(child, 'text')):
                                if is_dark:
                                    label.setPixmap(get_icon('settings', 16, QColor(255, 255, 255)).pixmap(16, 16))
                                else:
                                    label.setPixmap(get_icon('settings', 16).pixmap(16, 16))
                                break
        except Exception as e:
            print(f"Error updating icon theme: {e}")
    
    def _is_dark_color(self, hex_color):
        """Determine if a color is dark based on its luminance"""
        # Remove # if present
        hex_color = hex_color.lstrip('#')
        
        # Convert to RGB
        r = int(hex_color[0:2], 16)
        g = int(hex_color[2:4], 16)
        b = int(hex_color[4:6], 16)
        
        # Calculate luminance using standard formula
        luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255
        return luminance < 0.5
    
    def _get_theme_colors(self, theme_name):
        """Get comprehensive color scheme for a theme"""
        theme = self.THEMES.get(theme_name, self.THEMES["white"])
        primary_color = theme["primary"]
        secondary_color = theme["secondary"]
        is_dark = self._is_dark_color(primary_color)
        
        if is_dark:
            return {
                "primary": primary_color,
                "secondary": secondary_color,
                "text_primary": "#ffffff",
                "text_secondary": "#e0e0e0",
                "text_muted": "#b0b0b0",
                "border": "#555555",
                "border_light": "#666666",
                "accent": "#4A90E2",
                "accent_hover": "#5BA0F2"
            }
        else:
            return {
                "primary": primary_color,
                "secondary": secondary_color,
                "text_primary": "#2c3e50",
                "text_secondary": "#495057",
                "text_muted": "#6c757d",
                "border": "#dee2e6",
                "border_light": "#adb5bd",
                "accent": "#4A90E2",
                "accent_hover": "#357ABD"
            }
    
    def apply_tab_theme(self, theme_name):
        """Apply theme-aware styling to tabs"""
        colors = self._get_theme_colors(theme_name)
        is_dark = self._is_dark_color(colors["primary"])
        
        # Create appropriate accent colors for tabs
        if is_dark:
            active_bg = self._lighten_color(colors["primary"], 0.15)
            active_border = self._lighten_color(colors["primary"], 0.3)
            hover_bg = self._lighten_color(colors["primary"], 0.08)
        else:
            active_bg = self._darken_color(colors["primary"], 0.08)
            active_border = self._darken_color(colors["primary"], 0.2)
            hover_bg = self._darken_color(colors["primary"], 0.04)
        
        self.tabs.setStyleSheet(f"""
            QTabWidget::pane {{
                border: 2px solid {colors["border"]};
                border-radius: 8px;
                background-color: {colors["primary"]};
            }}
            QTabWidget::tab-bar {{
                
            }}
            QTabBar::tab {{
                background: {colors["secondary"]};
                color: {colors["text_secondary"]};
                border: 2px solid {colors["border"]};
                border-bottom: none;
                border-top-left-radius: 8px;
                border-top-right-radius: 8px;
                padding: 8px 16px;
                margin-right: 2px;
                font-size: 13px;
                font-weight: 500;
                outline: none;
            }}
            QTabBar::tab:selected {{
                background: {active_bg};
                color: {colors["text_primary"]};
                border-color: {active_border};
                border-bottom: 2px solid {active_bg};
                font-weight: 500;
                outline: none;
            }}
            QTabBar::tab:hover {{
                background: {hover_bg};
                color: {colors["text_primary"]};
                border-color: {colors["border_light"]};
                outline: none;
            }}
            QTabBar::tab:selected:hover {{
                background: {active_bg};
                color: {colors["text_primary"]};
                border-color: {active_border};
                outline: none;
            }}
        """)
    
    def closeEvent(self, event):
        """Handle dialog close event"""
        try:
            # Clean up audio tab resources
            if hasattr(self, 'audio_tab'):
                self.audio_tab.cleanup()
        except Exception as e:
            print(f"Error during SettingsDialog cleanup: {e}")
        event.accept()


class UITab(QWidget):
    """UI Settings: Compact mode and visualization options"""
    def __init__(self, config_manager: ConfigManager, parent=None):
        super().__init__(parent)
        self.config_manager = config_manager
        self.setup_ui()
        self.load_settings()

    def setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(20)

        intro = QLabel("Configure UI, including Super Compact mode and microphone visualization.")
        intro.setWordWrap(True)
        intro.setStyleSheet("color: #6c757d; font-size: 12px;")
        layout.addWidget(intro)

        box = ModernGroupBox("UI Settings")
        form = QVBoxLayout(box)
        self.compact_checkbox = ModernCheckBox("Enable Super Compact UI")
        self.compact_checkbox.setIcon(get_button_icon('compact', 16))
        self.compact_checkbox.setIconSize(QSize(16, 16))
        form.addWidget(self.compact_checkbox)

        tip = QLabel("In compact mode, the window is frameless with a right-aligned close button, and you can drag anywhere to move.")
        tip.setWordWrap(True)
        tip.setStyleSheet("color: #6c757d; font-size: 11px;")
        form.addWidget(tip)
        
        # Add custom title bar setting
        self.custom_titlebar_checkbox = ModernCheckBox("Enable custom title bar")
        self.custom_titlebar_checkbox.setIcon(get_button_icon('window', 16))
        self.custom_titlebar_checkbox.setIconSize(QSize(16, 16))
        form.addWidget(self.custom_titlebar_checkbox)

        titlebar_tip = QLabel("When enabled, uses a custom title bar instead of the system default. Disable for standard OS title bar.")
        titlebar_tip.setWordWrap(True)
        titlebar_tip.setStyleSheet("color: #6c757d; font-size: 11px;")
        form.addWidget(titlebar_tip)
        
        # Add theme selection to the same group box
        theme_form_layout = QFormLayout()
        theme_form_layout.setContentsMargins(0, 10, 0, 0)  # Add some spacing from compact mode
        theme_form_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        
        # Create theme dropdown widget with icon
        theme_widget = QWidget()
        theme_widget_layout = QHBoxLayout(theme_widget)
        theme_widget_layout.setContentsMargins(0, 0, 0, 0)
        theme_widget_layout.setSpacing(6)
        
        theme_icon = QLabel()
        theme_icon.setPixmap(get_icon('settings', 16).pixmap(16, 16))
        theme_widget_layout.addWidget(theme_icon)
        
        self.theme_label = QLabel("Background theme:")
        self.theme_label.setStyleSheet("QLabel { color: #495057; font-size: 13px; }")
        theme_widget_layout.addWidget(self.theme_label)
        theme_widget_layout.addStretch()
        
        self.theme_dropdown = ModernComboBox()
        self.theme_dropdown.addItem("Classic White", "white")
        self.theme_dropdown.addItem("Warm Gray", "warm_gray")
        self.theme_dropdown.addItem("Soft Beige", "soft_beige")
        self.theme_dropdown.addItem("Muted Blue-Gray", "blue_gray")
        self.theme_dropdown.addItem("Warm Taupe", "warm_taupe")
        self.theme_dropdown.addItem("Soft Sage", "soft_sage")
        # Dark themes
        self.theme_dropdown.addItem("Dark Charcoal", "dark_charcoal")
        self.theme_dropdown.addItem("Dark Blue", "dark_blue")
        self.theme_dropdown.addItem("Dark Purple", "dark_purple")
        self.theme_dropdown.addItem("Dark Forest", "dark_forest")
        self.theme_dropdown.addItem("Dark Burgundy", "dark_burgundy")
        
        theme_form_layout.addRow(theme_widget, self.theme_dropdown)
        
        theme_help = QLabel("Choose a background color theme for the application. Changes apply immediately.")
        theme_help.setWordWrap(True)
        theme_help.setStyleSheet("color: #6c757d; font-size: 11px; background-color: transparent;")
        theme_form_layout.addRow("", theme_help)
        
        form.addLayout(theme_form_layout)

        layout.addWidget(box)

        # Visualization options moved here
        viz_group = ModernGroupBox("Visualization")
        viz_layout = QFormLayout(viz_group)
        viz_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)

        self.show_waveform_checkbox = ModernCheckBox("Show circular waveform during recording")
        self.show_waveform_checkbox.setIcon(get_button_icon('zap', 16))
        self.show_waveform_checkbox.setIconSize(QSize(16, 16))
        self.show_waveform_checkbox.setChecked(True)
        viz_layout.addRow(self.show_waveform_checkbox)

        self.animation_strength_label = QLabel("Animation Amplification:")
        self.animation_strength_slider = QSlider(Qt.Orientation.Horizontal)
        self.animation_strength_slider.setRange(1, 10)
        self.animation_strength_slider.setValue(3)
        self.animation_strength_slider.setTickPosition(QSlider.TickPosition.TicksBelow)
        self.animation_strength_slider.setTickInterval(1)
        self.animation_strength_slider.setStyleSheet("""
            QSlider::groove:horizontal { border: 2px solid #dee2e6; height: 8px; background: #f8f9fa; border-radius: 4px; }
            QSlider::handle:horizontal { background: #4A90E2; border: 2px solid #357ABD; width: 16px; margin: -4px 0; border-radius: 8px; }
            QSlider::handle:horizontal:hover { background: #5BA0F2; }
        """)

        self.animation_strength_value_label = QLabel("3x")
        self.animation_strength_value_label.setStyleSheet("QLabel { color: #495057; font-size: 12px; font-weight: bold; min-width: 30px; }")
        self.animation_strength_slider.valueChanged.connect(self.update_animation_strength_label)

        strength_layout = QHBoxLayout()
        strength_layout.addWidget(self.animation_strength_slider)
        strength_layout.addWidget(self.animation_strength_value_label)
        viz_layout.addRow(self.animation_strength_label, strength_layout)

        strength_help = QLabel("Higher values make waveform more visible with quiet voice")
        strength_help.setStyleSheet("color: #6c757d; font-size: 11px; background-color: transparent;")
        strength_help.setWordWrap(True)
        viz_layout.addRow("", strength_help)

        layout.addWidget(viz_group)
        
        # Shortcuts & Window behavior
        shortcuts_group = ModernGroupBox("Shortcuts & Window")
        shortcuts_layout = QFormLayout(shortcuts_group)
        shortcuts_layout.setLabelAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        
        self.minimize_on_close_checkbox = ModernCheckBox("Minimize on close (instead of exiting)")
        self.minimize_on_close_checkbox.setIcon(get_button_icon('window', 16))
        self.minimize_on_close_checkbox.setIconSize(QSize(16, 16))
        self.minimize_on_close_checkbox.setChecked(True)
        shortcuts_layout.addRow(self.minimize_on_close_checkbox)

        # Create label with icon for shortcut
        shortcut_widget = QWidget()
        shortcut_layout = QHBoxLayout(shortcut_widget)
        shortcut_layout.setContentsMargins(0, 0, 0, 0)
        shortcut_layout.setSpacing(6)
        
        shortcut_icon = QLabel()
        shortcut_icon.setPixmap(get_icon('keyboard', 16).pixmap(16, 16))
        shortcut_layout.addWidget(shortcut_icon)
        
        self.shortcut_label = QLabel("Recording shortcut:")
        self.shortcut_label.setStyleSheet("QLabel { color: #495057; font-size: 13px; }")
        shortcut_layout.addWidget(self.shortcut_label)
        shortcut_layout.addStretch()
        
        self.shortcut_edit = ModernKeySequenceEdit()
        shortcuts_layout.addRow(shortcut_widget, self.shortcut_edit)

        shortcut_help = QLabel("Click in the field above and press your desired key combination to record a shortcut. This shortcut toggles recording. On Windows, 'Win' refers to the Windows key.")
        shortcut_help.setWordWrap(True)
        shortcut_help.setStyleSheet("color: #6c757d; font-size: 11px; background-color: transparent;")
        shortcuts_layout.addRow("", shortcut_help)

        layout.addWidget(shortcuts_group)
        layout.addStretch()

    def load_settings(self):
        enabled = bool(self.config_manager.get_setting("ui/compact_mode", False))
        self.compact_checkbox.setChecked(enabled)
        show_waveform = self.config_manager.get_setting("ui/show_waveform", True)
        self.show_waveform_checkbox.setChecked(bool(show_waveform))
        animation_strength = self.config_manager.get_setting("ui/animation_strength", 3)
        self.animation_strength_slider.setValue(int(animation_strength))
        self.update_animation_strength_label(int(animation_strength))
        # New settings
        minimize_on_close = bool(self.config_manager.get_setting("ui/minimize_on_close", True))
        self.minimize_on_close_checkbox.setChecked(minimize_on_close)
        shortcut = self.config_manager.get_setting("shortcuts/record_toggle", "Win+F")
        if isinstance(shortcut, str):
            # Convert Windows format to Qt format for display
            qt_shortcut = ModernKeySequenceEdit.windows_to_qt_shortcut(shortcut)
            self.shortcut_edit.setKeySequence(QKeySequence.fromString(qt_shortcut))
        
        # Load custom title bar setting
        custom_titlebar = bool(self.config_manager.get_setting("ui/custom_titlebar", True))
        self.custom_titlebar_checkbox.setChecked(custom_titlebar)
        
        # Load theme setting
        theme = self.config_manager.get_setting("ui/theme", "white")
        index = self.theme_dropdown.findData(theme)
        if index >= 0:
            self.theme_dropdown.setCurrentIndex(index)
        
        # Connect theme change signal
        self.theme_dropdown.currentIndexChanged.connect(self.on_theme_changed)

    def save_settings(self):
        self.config_manager.set_setting("ui/compact_mode", self.compact_checkbox.isChecked())
        self.config_manager.set_setting("ui/show_waveform", self.show_waveform_checkbox.isChecked())
        self.config_manager.set_setting("ui/animation_strength", self.animation_strength_slider.value())
        # Save new settings
        self.config_manager.set_setting("ui/minimize_on_close", self.minimize_on_close_checkbox.isChecked())
        qt_shortcut_sequence = self.shortcut_edit.keySequence().toString()
        # Convert Qt format to Windows format for storage
        windows_shortcut = ModernKeySequenceEdit.qt_to_windows_shortcut(qt_shortcut_sequence) or "Win+F"
        self.config_manager.set_setting("shortcuts/record_toggle", windows_shortcut)
        
        # Save custom title bar setting
        self.config_manager.set_setting("ui/custom_titlebar", self.custom_titlebar_checkbox.isChecked())
        
        # Save theme
        theme_data = self.theme_dropdown.currentData()
        if theme_data:
            self.config_manager.set_setting("ui/theme", theme_data)

    def update_animation_strength_label(self, value):
        self.animation_strength_value_label.setText(f"{value}x")
    
    def on_theme_changed(self):
        """Apply theme immediately when changed"""
        theme_data = self.theme_dropdown.currentData()
        if theme_data:
            self.config_manager.set_setting("ui/theme", theme_data)
            self.config_manager.save_settings()  # Save immediately
            
            # Apply theme to the parent dialog immediately
            dialog = self.parent()
            while dialog and not isinstance(dialog, SettingsDialog):
                dialog = dialog.parent()
            if dialog:
                dialog.apply_theme(theme_data)
                
                # Also apply to main window if it exists
                main_window = dialog.parent()
                if main_window and hasattr(main_window, 'apply_theme'):
                    main_window.apply_theme(theme_data)


class UISettingsDialog(QDialog):
    """A super-compact, frameless settings dialog for compact mode."""
    settings_saved = Signal()

    def __init__(self, parent=None, config_manager=None):
        super().__init__(parent)
        self.config_manager = config_manager if config_manager is not None else ConfigManager()
        self._drag_active = False
        self._drag_offset = None
        self.setup_ui()

    def setup_ui(self):
        self.setWindowTitle("UI Settings")
        self.setFixedSize(200, 200)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Dialog)

        layout = QVBoxLayout(self)
        layout.setSpacing(10)
        layout.setContentsMargins(8, 8, 8, 8)

        # Top bar with right-aligned ✕ button
        top = QHBoxLayout()
        title = QLabel("UI")
        title.setStyleSheet("QLabel { color: #2c3e50; font-size: 12px; font-weight: 600; }")
        top.addWidget(title)
        top.addStretch()
        self.close_btn = QPushButton(self)
        self.close_btn.setIcon(get_button_icon('close', 12))
        self.close_btn.setIconSize(QSize(12, 12))
        self.close_btn.setFixedSize(20, 20)
        self.close_btn.setStyleSheet(
            """
            QPushButton {
                background: rgba(0,0,0,0.08);
                color: #2c3e50;
                border: 1px solid #ced4da;
                border-radius: 10px;
                font-size: 12px;
                font-weight: bold;
                padding: 0px;
            }
            QPushButton:hover { background: rgba(0,0,0,0.2); }
            """
        )
        # Use a drawn icon for perfect centering of the close "X"
        try:
            cross_size = 10
            pix = QPixmap(cross_size, cross_size)
            pix.fill(Qt.GlobalColor.transparent)
            p = QPainter(pix)
            p.setRenderHint(QPainter.RenderHint.Antialiasing)
            pen = QPen(QColor(44, 62, 80), 2, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap)
            p.setPen(pen)
            p.drawLine(2, 2, cross_size - 2, cross_size - 2)
            p.drawLine(cross_size - 2, 2, 2, cross_size - 2)
            p.end()
            self.close_btn.setIcon(QIcon(pix))
            self.close_btn.setIconSize(pix.rect().size())
            self.close_btn.setText("")
        except Exception:
            pass
        self.close_btn.clicked.connect(self.reject)
        top.addWidget(self.close_btn)
        layout.addLayout(top)

        # Content
        info = QLabel("Super Compact UI")
        info.setStyleSheet("QLabel { color: #495057; font-size: 11px; }")
        layout.addWidget(info)

        self.compact_checkbox = QCheckBox("Enable")
        self.compact_checkbox.setStyleSheet("""
            QCheckBox {
                font-size: 12px;
                color: #495057;
                spacing: 8px;
                outline: none;
            }
            QCheckBox::indicator {
                width: 14px;
                height: 14px;
            }
            QCheckBox::indicator:unchecked {
                border: 2px solid #dee2e6;
                border-radius: 3px;
                background-color: white;
            }
            QCheckBox::indicator:unchecked:hover {
                border-color: #4A90E2;
            }
            QCheckBox::indicator:checked {
                border: 2px solid #4A90E2;
                border-radius: 3px;
                background-color: #4A90E2;
            }
        """)
        self.compact_checkbox.setChecked(bool(self.config_manager.get_setting("ui/compact_mode", False)))
        layout.addWidget(self.compact_checkbox)

        layout.addStretch()

        # Buttons
        buttons = QHBoxLayout()
        buttons.addStretch()
        save_btn = QPushButton("Save")
        save_btn.setIcon(get_white_button_icon('save', 12))
        save_btn.setIconSize(QSize(12, 12))
        save_btn.setStyleSheet(
            "QPushButton { background: #4A90E2; color: white; border: none; border-radius: 6px; padding: 6px 10px; font-size: 12px; }"
        )
        save_btn.clicked.connect(self.save_and_close)
        buttons.addWidget(save_btn)
        layout.addLayout(buttons)

        # Drag anywhere support
        self.installEventFilter(self)

        # Styling
        self.setStyleSheet(
            """
            QDialog {
                background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                    stop:0 #ffffff, stop:1 #f8f9fa);
            }
            """
        )

    def eventFilter(self, obj, event):
        if event.type() == QEvent.Type.MouseButtonPress and event.button() == Qt.MouseButton.LeftButton:
            self._drag_active = True
            # Qt6: globalPosition returns QPointF
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self._drag_offset = global_pos - self.frameGeometry().topLeft()
            return False
        elif event.type() == QEvent.Type.MouseMove and self._drag_active and event.buttons() & Qt.MouseButton.LeftButton:
            try:
                global_pos = event.globalPosition().toPoint()
            except Exception:
                global_pos = event.globalPos()
            self.move(global_pos - self._drag_offset)
            return True
        elif event.type() == QEvent.Type.MouseButtonRelease:
            self._drag_active = False
            return False
        return super().eventFilter(obj, event)

    def save_and_close(self):
        self.config_manager.set_setting("ui/compact_mode", self.compact_checkbox.isChecked())
        self.config_manager.save_settings()
        self.settings_saved.emit()
        self.accept() 
