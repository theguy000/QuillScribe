#!/usr/bin/env bash
#
# QuillScribe — One-click Linux installer
# Downloads the latest AppImage from GitHub Releases and installs it locally.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/theguy000/QuillScribe/main/install.sh | bash
#   — or —
#   ./install.sh
#
set -euo pipefail

REPO="theguy000/QuillScribe"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
APP_NAME="QuillScribe"

# ── Helpers ──────────────────────────────────────────────────────────────────

info()  { printf "\033[1;34m[info]\033[0m  %s\n" "$1"; }
ok()    { printf "\033[1;32m[ok]\033[0m    %s\n" "$1"; }
warn()  { printf "\033[1;33m[warn]\033[0m  %s\n" "$1" >&2; }
err()   { printf "\033[1;31m[error]\033[0m %s\n" "$1" >&2; exit 1; }

command_exists() { command -v "$1" &>/dev/null; }

# ── Preflight checks ────────────────────────────────────────────────────────

if [[ "$(uname -s)" != "Linux" ]]; then
  err "This installer is for Linux only. Download the Windows installer from GitHub Releases."
fi

if ! command_exists curl && ! command_exists wget; then
  err "Neither curl nor wget found. Install one and re-run."
fi

# ── Resolve latest release ─────────────────────────────────────────────────

info "Fetching latest release from GitHub…"

API_URL="https://api.github.com/repos/${REPO}/releases/latest"

if command_exists curl; then
  RELEASE_JSON=$(curl -fsSL "$API_URL") || err "Failed to reach GitHub API. Check your connection."
else
  RELEASE_JSON=$(wget -qO- "$API_URL")  || err "Failed to reach GitHub API. Check your connection."
fi

# Extract the AppImage asset download URL
APPIMAGE_URL=$(echo "$RELEASE_JSON" | grep -oP '"browser_download_url":\s*"\K[^"]*\.AppImage"' | head -1 | tr -d '"')

if [[ -z "${APPIMAGE_URL:-}" ]]; then
  err "No AppImage found in the latest release. Pre-built Linux packages may not be available yet."
fi

TAG_NAME=$(echo "$RELEASE_JSON" | grep -oP '"tag_name":\s*"\K[^"]+' | head -1)
FILENAME=$(basename "$APPIMAGE_URL")

info "Latest version: ${TAG_NAME}"

# ── Download ───────────────────────────────────────────────────────────────

DEST="${INSTALL_DIR}/${APP_NAME}.AppImage"

info "Downloading ${FILENAME}…"

mkdir -p "$INSTALL_DIR"

if command_exists curl; then
  curl -fSL --progress-bar -o "$DEST" "$APPIMAGE_URL" || err "Download failed."
else
  wget -q --show-progress -O "$DEST" "$APPIMAGE_URL"   || err "Download failed."
fi

chmod +x "$DEST"

# ── Optional: create symlink without .AppImage extension ──────────────────

SYMLINK="${INSTALL_DIR}/quillscribe"
if [[ ! -L "$SYMLINK" ]] && [[ ! -e "$SYMLINK" ]]; then
  ln -s "$DEST" "$SYMLINK"
  ok "Symlink created: ${SYMLINK} → ${DEST}"
fi

# ── Ensure install dir is on PATH ──────────────────────────────────────────

SHELL_RC=""
if [[ -f "$HOME/.bashrc" ]]; then SHELL_RC="$HOME/.bashrc";
elif [[ -f "$HOME/.zshrc" ]]; then SHELL_RC="$HOME/.zshrc"; fi

PATH_LINE="export PATH=\"\${PATH:+\$PATH:}\$HOME/.local/bin\""

if [[ "$INSTALL_DIR" == "$HOME/.local/bin" ]]; then
  if [[ -n "$SHELL_RC" ]] && ! grep -q '\.local/bin' "$SHELL_RC" 2>/dev/null; then
    echo "" >> "$SHELL_RC"
    echo "# Added by QuillScribe installer" >> "$SHELL_RC"
    echo "$PATH_LINE" >> "$SHELL_RC"
    ok "Added \$HOME/.local/bin to PATH in ${SHELL_RC}"
    warn "Run \`source ${SHELL_RC}\` or open a new terminal for PATH to take effect."
  fi
fi

# ── Done ────────────────────────────────────────────────────────────────────

echo ""
ok "QuillScribe ${TAG_NAME} installed!"
echo ""
echo "  Run it with:"
echo "    quillscribe"
echo "  — or —"
echo "    ${DEST}"
echo ""
echo "To uninstall, simply delete:"
echo "    ${DEST}"
echo "    ${SYMLINK}"
echo ""
