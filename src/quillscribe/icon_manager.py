"""
Icon Manager for QuillScribe
Utility class for loading and managing SVG icons consistently throughout the application
"""

import os
import sys
from typing import Optional, Dict
from PySide6.QtGui import QIcon, QPixmap, QPainter, QColor
from PySide6.QtCore import Qt, QSize, QRect
from PySide6.QtSvg import QSvgRenderer


class IconManager:
    """Centralized icon management for QuillScribe application"""
    
    # Icon mappings for consistent usage throughout the app
    ICONS = {
        # Main UI icons
        'microphone': 'mic.svg',
        'sound': 'volume-2.svg',
        'audio': 'mic.svg',  # Use microphone icon for audio settings
        'settings': 'settings.svg',
        'close': 'x.svg',  # Simple X icon for close buttons
        
        # Output/clipboard icons
        'clipboard': 'clipboard.svg',
        'eye': 'eye.svg',
        'copy': 'clipboard.svg',
        'paste': 'clipboard.svg',  # Could also use a separate paste icon if available
        
        # Recording icons
        'record': 'mic.svg',
        'stop': 'square.svg',
        'play': 'play.svg',
        
        # System icons
        'silent': 'bell-off.svg',
        'refresh': 'refresh-cw.svg',
        'folder': 'folder.svg',
        'brain': 'brain.svg',
        'key': 'key.svg',
        'dashboard': 'layout-dashboard.svg',
        'trash': 'trash-2.svg',
        'zap': 'zap.svg',
        'category': 'category.svg',
        
        # App icons
        'app': 'app-icon.svg',
        
        # Action icons
        'save': 'settings.svg',  # Use settings icon for save actions
        'cancel': 'x.svg',  # Use X icon for cancel/close
        'test': 'play.svg',  # Use play icon for test actions
        'load': 'folder.svg',  # Use folder icon for load actions
        
        # UI mode icons
        'api': 'zap.svg',  # Lightning for fast API
        'local': 'brain.svg',  # Brain for local processing
        'compact': 'minimize-2.svg',  # Minimize for compact mode
        'keyboard': 'key.svg',  # Keyboard shortcut
        'window': 'window-minimize.svg'  # Window minimize option
    }
    
    def __init__(self):
        self._icon_cache: Dict[str, QIcon] = {}
        # Handle both development and frozen executable environments
        if getattr(sys, 'frozen', False):
            # Running as frozen executable - use PyInstaller's temporary directory
            base_path = sys._MEIPASS
            self._icons_dir = os.path.join(base_path, 'icons')
        else:
            # Running from source
            self._icons_dir = os.path.join(os.path.dirname(__file__), 'icons')
        
    def get_icon_path(self, icon_name: str) -> Optional[str]:
        """Get the full path to an icon file"""
        if icon_name not in self.ICONS:
            return None
            
        icon_file = self.ICONS[icon_name]
        icon_path = os.path.join(self._icons_dir, icon_file)
        
        if os.path.exists(icon_path):
            return icon_path
        return None
    
    def get_icon(self, icon_name: str, size: int = 24, color: Optional[QColor] = None, vertical_offset: int = 0) -> QIcon:
        """
        Get a QIcon for the specified icon name
        
        Args:
            icon_name: Name of the icon (key from ICONS dict)
            size: Desired icon size in pixels
            color: Optional color to colorize the icon
            vertical_offset: Optional vertical offset for alignment.
            
        Returns:
            QIcon object, or empty QIcon if icon not found
        """
        cache_key = f"{icon_name}_{size}_{color.name() if color else 'default'}_{vertical_offset}"
        
        # Return cached icon if available
        if cache_key in self._icon_cache:
            return self._icon_cache[cache_key]
        
        icon_path = self.get_icon_path(icon_name)
        if not icon_path:
            print(f"Warning: Icon '{icon_name}' not found")
            return QIcon()
        
        try:
            # Always create icon from pixmap to allow for offset
            icon = self._create_svg_icon(icon_path, size, color, vertical_offset)
            
            # Cache the icon
            self._icon_cache[cache_key] = icon
            return icon
            
        except Exception as e:
            print(f"Error loading icon '{icon_name}': {e}")
            return QIcon()
    
    def _create_svg_icon(self, icon_path: str, size: int, color: Optional[QColor], vertical_offset: int) -> QIcon:
        """Create a QIcon from an SVG, with optional color and vertical alignment."""
        try:
            # Read SVG content
            with open(icon_path, 'r', encoding='utf-8') as f:
                svg_content = f.read()
            
            # Replace stroke color if a color is provided
            if color:
                color_hex = color.name()
                if 'stroke="currentColor"' in svg_content:
                    svg_content = svg_content.replace('stroke="currentColor"', f'stroke="{color_hex}"')
                elif 'stroke="' in svg_content:
                    import re
                    svg_content = re.sub(r'stroke="[^"]*"', f'stroke="{color_hex}"', svg_content)
            
            # Create QPixmap from SVG content
            renderer = QSvgRenderer()
            renderer.load(svg_content.encode('utf-8'))
            
            # Create a pixmap that is slightly taller to accommodate the offset
            pixmap = QPixmap(size, size + vertical_offset)
            pixmap.fill(Qt.GlobalColor.transparent)
            
            painter = QPainter(pixmap)
            painter.setRenderHint(QPainter.RenderHint.Antialiasing)
            
            # Define the target rectangle for rendering, shifted down by the offset
            target_rect = QRect(0, vertical_offset, size, size)
            renderer.render(painter, target_rect)
            painter.end()
            
            return QIcon(pixmap)
            
        except Exception as e:
            print(f"Error creating svg icon for {icon_path}: {e}")
            # Fallback to original icon
            return QIcon(icon_path)
    
    def get_pixmap(self, icon_name: str, size: int = 24, color: Optional[QColor] = None) -> QPixmap:
        """Get a QPixmap for the specified icon"""
        icon = self.get_icon(icon_name, size, color)
        return icon.pixmap(size, size)
    
    def create_button_icon(self, icon_name: str, size: int = 16, color: Optional[QColor] = None, vertical_offset: int = 0) -> QIcon:
        """Create an icon optimized for buttons"""
        return self.get_icon(icon_name, size, color, vertical_offset=vertical_offset)
    
    def create_menu_icon(self, icon_name: str, color: Optional[QColor] = None) -> QIcon:
        """Create an icon optimized for menus (16px)"""
        return self.get_icon(icon_name, 16, color)
    
    def create_toolbar_icon(self, icon_name: str, color: Optional[QColor] = None) -> QIcon:
        """Create an icon optimized for toolbars (24px)"""
        return self.get_icon(icon_name, 24, color)
    
    def list_available_icons(self) -> list:
        """Get a list of all available icon names"""
        return list(self.ICONS.keys())
    
    def clear_cache(self):
        """Clear the icon cache to free memory"""
        self._icon_cache.clear()


# Global icon manager instance
icon_manager = IconManager()


def get_icon(icon_name: str, size: int = 24, color: Optional[QColor] = None) -> QIcon:
    """Convenience function to get an icon"""
    return icon_manager.get_icon(icon_name, size, color)


def get_button_icon(icon_name: str, size: int = 16, color: Optional[QColor] = None) -> QIcon:
    """Convenience function to get a button icon with appropriate color and alignment."""
    if color is None:
        color = QColor(73, 80, 87)  # Default to dark gray for visibility on light buttons
    # Apply a 2px vertical offset to improve alignment with text
    return icon_manager.create_button_icon(icon_name, size, color, vertical_offset=2)


def get_white_button_icon(icon_name: str, size: int = 16) -> QIcon:
    """Convenience function to get a white button icon for dark backgrounds with improved alignment."""
    # Apply a 2px vertical offset to improve alignment with text
    return icon_manager.create_button_icon(icon_name, size, QColor(255, 255, 255), vertical_offset=2)
