#!/usr/bin/env python3
"""
Generate theme-aware taskbar icons and default bundle icons for QuillScribe.

Produces:
  src-tauri/icons/taskbar/<theme>.png  — 256x256 PNGs for runtime set_icon()
  src-tauri/icons/icon.ico             — multi-size ICO for PE resource embedding
  src-tauri/icons/32x32.png            — default bundle icon
  src-tauri/icons/128x128.png          — default bundle icon
  src-tauri/icons/128x128@2x.png       — default bundle icon (256x256)

Colors are extracted from the same palette as src/app.css.

Uses only Pillow (no native Cairo dependency).
"""

import io
import math
import os
import struct
from PIL import Image, ImageDraw


# ── Theme color palette (matches src/app.css exactly) ─────────────────────
# Format: theme_name -> (bg_primary, accent)
THEMES = {
    "white":          ("#f6f8fb", "#2563eb"),
    "warm_gray":      ("#f5f5f4", "#6366f1"),
    "soft_beige":     ("#faf8f5", "#d97706"),
    "blue_gray":      ("#f0f4f8", "#3b82f6"),
    "warm_taupe":     ("#f7f5f3", "#9333ea"),
    "soft_sage":      ("#f3f7f5", "#059669"),
    "dark_charcoal":  ("#18181b", "#a78bfa"),
    "dark_blue":      ("#111827", "#60a5fa"),
    "dark_purple":    ("#1a1025", "#c084fc"),
    "dark_forest":    ("#0f1a14", "#4ade80"),
    "dark_burgundy":  ("#1c0f14", "#fb7185"),
    "obsidian":       ("#0a0a0a", "#94a3b8"),
}

DEFAULT_THEME = "white"

# ── Feather quill geometry ────────────────────────────────────────────────
# The favicon.svg viewBox is 0 0 24 24. The quill consists of:
#   - A filled/stroked shape: M20.24 12.24a6 6 0 0 0-8.49-8.49L5 10.5V19h8.5z
#   - Line 1: (16,8) -> (2,22)
#   - Line 2: (17.5,15) -> (9,15)
#
# We approximate the arc "a6 6 0 0 0-8.49-8.49" with intermediate points.
# The arc goes from (20.24, 12.24) to (20.24-8.49, 12.24-8.49) = (11.75, 3.75)
# It's a 6-radius arc sweeping counter-clockwise.
#
# Arc center: The arc is part of a circle of radius 6.
# From (20.24,12.24) to (11.75,3.75), with large-arc=0, sweep=0.
# Center can be computed: midpoint is (16.0, 8.0), distance = 8.49*sqrt(2)/2 ≈ 6.0
# The center is at (16.0, 8.0) (coincidence — it's roughly on the line between points).
# More precisely, for a radius-6 arc: d = sqrt((20.24-11.75)^2 + (12.24-3.75)^2) = sqrt(72.1 + 72.1) = 12.01
# half = 6.005, h = sqrt(36 - 36.06) — this is basically a semicircle.

def _arc_points(cx, cy, r, start_angle, end_angle, n=16):
    """Generate points along a circular arc."""
    points = []
    for i in range(n + 1):
        t = start_angle + (end_angle - start_angle) * i / n
        points.append((cx + r * math.cos(t), cy + r * math.sin(t)))
    return points


def _quill_outline_24():
    """
    Return the outline of the quill shape in the 24x24 coordinate space.
    The SVG path: M20.24 12.24 a6 6 0 0 0 -8.49 -8.49 L5 10.5 V19 h8.5 z

    Breakdown:
      - Start at (20.24, 12.24)
      - Arc (relative): rx=6 ry=6, x-rotation=0, large-arc=0, sweep=0, dx=-8.49 dy=-8.49
        End: (11.75, 3.75)
      - Line to (5, 10.5)
      - Vertical line to y=19 -> (5, 19)
      - Horizontal line dx=+8.5 -> (13.5, 19)
      - Close path back to (20.24, 12.24)
    """
    # Compute the arc. For SVG arc a6,6 0 0,0 -8.49,-8.49:
    # Start: (20.24, 12.24), End: (11.75, 3.75), r=6
    # Using the SVG arc to center formula:
    x1, y1 = 20.24, 12.24
    x2, y2 = 11.75, 3.75
    r = 6.0

    # Midpoint
    mx, my = (x1 + x2) / 2, (y1 + y2) / 2  # (15.995, 7.995)
    dx, dy = (x2 - x1) / 2, (y2 - y1) / 2  # (-4.245, -4.245)
    d = math.sqrt(dx * dx + dy * dy)  # ~6.003

    # For large-arc=0, sweep=0:
    # discriminant
    disc = max(0, (r * r) / (d * d) - 1)
    sq = math.sqrt(disc)

    # sweep=0 means we pick the center that gives CW arc when going from p1 to p2
    # For sweep=0, large_arc=0:
    # sign = +1 if large_arc != sweep else -1
    sign = -1  # large_arc(0) == sweep(0) -> True -> sign = -1

    cx = mx + sign * sq * dy  # center x
    cy = my - sign * sq * dx  # center y

    # Angles
    a1 = math.atan2(y1 - cy, x1 - cx)
    a2 = math.atan2(y2 - cy, x2 - cx)

    # For sweep=0 (counter-clockwise in SVG = clockwise in math since y is down)
    # We need to go from a1 to a2 in the negative direction
    if a2 > a1:
        a2 -= 2 * math.pi

    arc_pts = _arc_points(cx, cy, r, a1, a2, n=20)

    # Full outline
    points = []
    points.extend(arc_pts)      # Arc from (20.24,12.24) to (11.75,3.75)
    points.append((5, 10.5))    # L5 10.5
    points.append((5, 19))      # V19
    points.append((13.5, 19))   # h8.5
    # z closes back to start (20.24, 12.24) — handled by polygon fill

    return points


# Pre-compute the quill outline in 24x24 space
_QUILL_OUTLINE_24 = _quill_outline_24()
_LINE1_24 = [(16, 8), (2, 22)]
_LINE2_24 = [(17.5, 15), (9, 15)]


def _hex_to_rgb(hex_color: str) -> tuple[int, int, int]:
    """Convert '#rrggbb' to (r, g, b)."""
    h = hex_color.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


def _draw_rounded_rect(draw, xy, radius, fill):
    """Draw a filled rounded rectangle."""
    x0, y0, x1, y1 = xy
    r = radius
    # Four corner circles
    draw.ellipse([x0, y0, x0 + 2 * r, y0 + 2 * r], fill=fill)
    draw.ellipse([x1 - 2 * r, y0, x1, y0 + 2 * r], fill=fill)
    draw.ellipse([x0, y1 - 2 * r, x0 + 2 * r, y1], fill=fill)
    draw.ellipse([x1 - 2 * r, y1 - 2 * r, x1, y1], fill=fill)
    # Two rectangles to fill the center
    draw.rectangle([x0 + r, y0, x1 - r, y1], fill=fill)
    draw.rectangle([x0, y0 + r, x1, y1 - r], fill=fill)


def _transform_points(points, scale, offset_x, offset_y):
    """Scale and translate a list of (x,y) points."""
    return [(x * scale + offset_x, y * scale + offset_y) for x, y in points]


def render_icon(bg_hex: str, accent_hex: str, size: int) -> Image.Image:
    """
    Render a QuillScribe icon at the given size using pure Pillow.

    Renders at 4x supersampling then downscales for anti-aliasing.
    """
    ss = 4  # supersampling factor
    ss_size = size * ss

    bg_rgb = _hex_to_rgb(bg_hex)
    accent_rgb = _hex_to_rgb(accent_hex)

    img = Image.new("RGBA", (ss_size, ss_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # ── Rounded rectangle background ──────────────────────────────────────
    # Corner radius = 48/256 of the icon size (matching SVG rx=48 on 256 viewBox)
    corner_r = int(ss_size * 48 / 256)
    _draw_rounded_rect(draw, (0, 0, ss_size - 1, ss_size - 1), corner_r, fill=bg_rgb + (255,))

    # ── Feather quill ─────────────────────────────────────────────────────
    # The quill sits in a 24x24 viewBox, placed at (48,48) in 256x256 space
    # and scaled by 256*160/256 = 160 pixels for the 24-unit space -> scale = 160/24 = 6.667
    # offset = 48 in 256 space
    quill_area = ss_size * 160 / 256   # usable area for the 24x24 quill
    quill_scale = quill_area / 24
    quill_offset = ss_size * 48 / 256

    # Stroke width: SVG stroke-width=2 in 24-unit space
    stroke_w = max(1, int(2 * quill_scale + 0.5))

    # Draw filled quill shape outline
    outline_pts = _transform_points(_QUILL_OUTLINE_24, quill_scale, quill_offset, quill_offset)
    draw.polygon(outline_pts, outline=accent_rgb + (255,), fill=None)
    # Thicken the outline by drawing it as a series of lines
    for i in range(len(outline_pts)):
        p1 = outline_pts[i]
        p2 = outline_pts[(i + 1) % len(outline_pts)]
        draw.line([p1, p2], fill=accent_rgb + (255,), width=stroke_w)

    # Draw the two lines
    line1_pts = _transform_points(_LINE1_24, quill_scale, quill_offset, quill_offset)
    line2_pts = _transform_points(_LINE2_24, quill_scale, quill_offset, quill_offset)
    draw.line(line1_pts, fill=accent_rgb + (255,), width=stroke_w)
    draw.line(line2_pts, fill=accent_rgb + (255,), width=stroke_w)

    # Add round caps to line endpoints
    cap_r = stroke_w // 2
    for pts in [line1_pts, line2_pts, [outline_pts[0], outline_pts[-1]]]:
        for px, py in pts:
            draw.ellipse(
                [px - cap_r, py - cap_r, px + cap_r, py + cap_r],
                fill=accent_rgb + (255,),
            )

    # ── Downsample for anti-aliasing ──────────────────────────────────────
    img = img.resize((size, size), Image.LANCZOS)
    return img


def build_ico(images: list[tuple[Image.Image, int]]) -> bytes:
    """
    Build an ICO file from a list of (PIL Image, size) tuples.
    Uses PNG encoding for each entry (supported by Windows Vista+).
    """
    count = len(images)
    header = struct.pack("<HHH", 0, 1, count)

    entries_data = []
    image_data_blocks = []
    offset = 6 + count * 16

    for img, size in images:
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        png_data = buf.getvalue()

        w = size if size < 256 else 0
        h = size if size < 256 else 0
        entry = struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(png_data), offset)
        entries_data.append(entry)
        image_data_blocks.append(png_data)
        offset += len(png_data)

    return header + b"".join(entries_data) + b"".join(image_data_blocks)


def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    icons_dir = os.path.join(base_dir, "src-tauri", "icons")
    taskbar_dir = os.path.join(icons_dir, "taskbar")
    os.makedirs(taskbar_dir, exist_ok=True)

    print("Generating QuillScribe taskbar icons...\n")

    # ── Generate theme-specific taskbar icons (256x256) ───────────────────
    for theme_name, (bg, accent) in THEMES.items():
        img = render_icon(bg, accent, 256)
        out_path = os.path.join(taskbar_dir, f"{theme_name}.png")
        img.save(out_path, "PNG")
        print(f"  taskbar/{theme_name}.png  (256x256)")

    # ── Generate default bundle icons from the default theme ──────────────
    bg, accent = THEMES[DEFAULT_THEME]

    bundle_sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
    }
    for filename, size in bundle_sizes.items():
        img = render_icon(bg, accent, size)
        img.save(os.path.join(icons_dir, filename), "PNG")
        print(f"  {filename}  ({size}x{size})")

    # ── Generate icon.ico (multi-size) ────────────────────────────────────
    ico_sizes = [16, 24, 32, 48, 64, 256]
    ico_images = []
    for s in ico_sizes:
        img = render_icon(bg, accent, s)
        ico_images.append((img, s))

    ico_bytes = build_ico(ico_images)
    ico_path = os.path.join(icons_dir, "icon.ico")
    with open(ico_path, "wb") as f:
        f.write(ico_bytes)
    print(f"  icon.ico  ({len(ico_sizes)} sizes: {ico_sizes})")

    print(f"\nDone. Generated {len(THEMES)} taskbar icons + bundle icons.")


if __name__ == "__main__":
    main()
