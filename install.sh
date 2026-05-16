#!/usr/bin/env bash
#
# QuillScribe — One-click Linux installer
# Downloads the latest Linux release from GitHub Releases and installs it locally.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/theguy000/QuillScribe/main/install.sh | bash
#   — or —
#   ./install.sh
#
# Optional:
#   QUILLSCRIBE_INSTALL_FORMAT=auto|appimage|tarball ./install.sh
#
set -euo pipefail

REPO="theguy000/QuillScribe"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
APP_NAME="QuillScribe"
INSTALL_FORMAT="${QUILLSCRIBE_INSTALL_FORMAT:-auto}"

# ── Helpers ──────────────────────────────────────────────────────────────────

info()  { printf "\033[1;34m[info]\033[0m  %s\n" "$1"; }
ok()    { printf "\033[1;32m[ok]\033[0m    %s\n" "$1"; }
warn()  { printf "\033[1;33m[warn]\033[0m  %s\n" "$1" >&2; }
err()   { printf "\033[1;31m[error]\033[0m %s\n" "$1" >&2; exit 1; }

command_exists() { command -v "$1" &>/dev/null; }

TEMP_PATHS=()
cleanup() {
  local path
  for path in "${TEMP_PATHS[@]}"; do
    [[ -n "$path" ]] && rm -rf "$path"
  done
}
trap cleanup EXIT

download_file() {
  local url="$1"
  local dest="$2"

  if command_exists curl; then
    curl -fSL --progress-bar -o "$dest" "$url"
  else
    wget -q --show-progress -O "$dest" "$url"
  fi
}

require_asset() {
  local url="$1"
  local name="$2"

  if [[ -z "${url:-}" ]]; then
    err "No ${name} found in the latest release. Pre-built Linux packages may not be available yet."
  fi
}

# ── Preflight checks ────────────────────────────────────────────────────────

if [[ "$(uname -s)" != "Linux" ]]; then
  err "This installer is for Linux only. Download the Windows installer from GitHub Releases."
fi

if ! command_exists curl && ! command_exists wget; then
  err "Neither curl nor wget found. Install one and re-run."
fi

INSTALL_FORMAT="${INSTALL_FORMAT,,}"
case "$INSTALL_FORMAT" in
  auto|appimage|tarball) ;;
  *) err "Invalid QUILLSCRIBE_INSTALL_FORMAT='${QUILLSCRIBE_INSTALL_FORMAT}'. Use auto, appimage, or tarball." ;;
esac

if [[ "$INSTALL_FORMAT" == "tarball" ]] && ! command_exists tar; then
  err "tar is required for QUILLSCRIBE_INSTALL_FORMAT=tarball. Install tar and re-run."
fi

# ── Resolve latest release ─────────────────────────────────────────────────

info "Fetching latest release from GitHub…"

API_URL="https://api.github.com/repos/${REPO}/releases/latest"

if command_exists curl; then
  RELEASE_JSON=$(curl -fsSL "$API_URL") || err "Failed to reach GitHub API. Check your connection."
else
  RELEASE_JSON=$(wget -qO- "$API_URL")  || err "Failed to reach GitHub API. Check your connection."
fi

APPIMAGE_URL=$(printf '%s\n' "$RELEASE_JSON" | grep -oPm1 '"browser_download_url":\s*"\K[^"]*\.AppImage"' | tr -d '"' || true)
TARBALL_URL=$(printf '%s\n' "$RELEASE_JSON" | grep -oPm1 '"browser_download_url":\s*"\K[^"]*quillscribe-x86_64-unknown-linux-gnu\.tar\.gz"' | tr -d '"' || true)
TAG_NAME=$(printf '%s\n' "$RELEASE_JSON" | grep -oPm1 '"tag_name":\s*"\K[^"]+' || true)

case "$INSTALL_FORMAT" in
  appimage) require_asset "$APPIMAGE_URL" "AppImage" ;;
  tarball) require_asset "$TARBALL_URL" "Linux tarball" ;;
  auto)
    if [[ -z "${APPIMAGE_URL:-}" && -z "${TARBALL_URL:-}" ]]; then
      err "No AppImage or Linux tarball found in the latest release. Pre-built Linux packages may not be available yet."
    fi
    ;;
esac

info "Latest version: ${TAG_NAME}"

# ── Install helpers ─────────────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"

APPIMAGE_DEST="${INSTALL_DIR}/${APP_NAME}.AppImage"
LAUNCHER_PATH="${INSTALL_DIR}/quillscribe"
SELECTED_EXEC=""
SELECTED_FORMAT=""

ensure_launcher_replaceable() {
  if [[ -d "$LAUNCHER_PATH" && ! -L "$LAUNCHER_PATH" ]]; then
    err "Cannot replace ${LAUNCHER_PATH}; it is a directory. Remove it and re-run the installer."
  fi
}

install_appimage() {
  local filename tmp_dest output status

  require_asset "$APPIMAGE_URL" "AppImage"
  filename=$(basename "$APPIMAGE_URL")
  info "Downloading ${filename}…"

  tmp_dest=$(mktemp "${INSTALL_DIR}/.${APP_NAME}.AppImage.XXXXXX") || err "Could not create temporary download file."
  TEMP_PATHS+=("$tmp_dest")

  if ! download_file "$APPIMAGE_URL" "$tmp_dest"; then
    warn "Could not download ${filename}."
    warn "If QuillScribe is running, quit it from the app/tray menu and run the installer again."
    warn "To force quit it, run: pkill -f QuillScribe.AppImage"
    return 1
  fi

  chmod +x "$tmp_dest"

  info "Validating AppImage runtime…"
  set +e
  output=$("$tmp_dest" --appimage-version 2>&1)
  status=$?
  set -e

  if [[ "$status" -ne 0 ]] || grep -q 'elf_machine_rela_relative' <<< "$output"; then
    warn "AppImage runtime validation failed."
    printf '%s\n' "$output" >&2
    return 1
  fi

  ensure_launcher_replaceable
  mv -f "$tmp_dest" "$APPIMAGE_DEST" || err "Could not install ${APPIMAGE_DEST}. Close QuillScribe and try again."
  ln -sfn "$APPIMAGE_DEST" "$LAUNCHER_PATH"

  SELECTED_EXEC="$LAUNCHER_PATH"
  SELECTED_FORMAT="AppImage"
  ok "AppImage installed: ${APPIMAGE_DEST}"
  ok "Symlink created: ${LAUNCHER_PATH} → ${APPIMAGE_DEST}"
}

install_tarball() {
  local filename tmp_archive tmp_dir tmp_binary extracted_binary

  require_asset "$TARBALL_URL" "Linux tarball"
  if ! command_exists tar; then
    err "tar is required to install the Linux tarball fallback. Install tar and re-run."
  fi

  filename=$(basename "$TARBALL_URL")
  info "Downloading ${filename}…"

  tmp_archive=$(mktemp "${INSTALL_DIR}/.quillscribe.tar.XXXXXX") || err "Could not create temporary download file."
  tmp_dir=$(mktemp -d "${INSTALL_DIR}/.quillscribe.extract.XXXXXX") || err "Could not create temporary extraction directory."
  tmp_binary=$(mktemp "${INSTALL_DIR}/.quillscribe.bin.XXXXXX") || err "Could not create temporary install file."
  TEMP_PATHS+=("$tmp_archive" "$tmp_dir" "$tmp_binary")

  if ! download_file "$TARBALL_URL" "$tmp_archive"; then
    warn "Could not download ${filename}."
    warn "If QuillScribe is running, quit it from the app/tray menu and run the installer again."
    err "Download failed."
  fi

  tar -xzf "$tmp_archive" -C "$tmp_dir" || err "Could not extract ${filename}."
  extracted_binary="${tmp_dir}/quillscribe"
  if [[ ! -f "$extracted_binary" ]]; then
    err "Linux tarball did not contain the expected quillscribe executable."
  fi

  ensure_launcher_replaceable
  cp "$extracted_binary" "$tmp_binary"
  chmod +x "$tmp_binary"
  mv -fT "$tmp_binary" "$LAUNCHER_PATH" || err "Could not install ${LAUNCHER_PATH}. Close QuillScribe and try again."

  SELECTED_EXEC="$LAUNCHER_PATH"
  SELECTED_FORMAT="native Linux tarball"
  ok "Native executable installed: ${LAUNCHER_PATH}"
}

# ── Download and install ───────────────────────────────────────────────────

case "$INSTALL_FORMAT" in
  appimage)
    install_appimage || err "AppImage install failed. Use QUILLSCRIBE_INSTALL_FORMAT=tarball to install the native Linux tarball instead."
    ;;
  tarball)
    install_tarball
    ;;
  auto)
    if [[ -n "${APPIMAGE_URL:-}" ]] && install_appimage; then
      :
    else
      if [[ -z "${TARBALL_URL:-}" ]]; then
        err "AppImage install failed and no Linux tarball fallback is available."
      fi
      warn "Falling back to the native Linux tarball."
      install_tarball
    fi
    ;;
esac

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

# ── Desktop entry (application menu) ────────────────────────────────────────

DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/128x128/apps"

mkdir -p "$DESKTOP_DIR" "$ICON_DIR"

ICON_URL="https://raw.githubusercontent.com/${REPO}/main/icons/128x128.png"
ICON_PATH="${ICON_DIR}/quillscribe.png"

if [[ ! -f "$ICON_PATH" ]]; then
  info "Downloading desktop icon…"
  if command_exists curl; then
    curl -fsSL -o "$ICON_PATH" "$ICON_URL" 2>/dev/null || warn "Could not download icon; desktop entry will use a generic icon."
  else
    wget -q -O "$ICON_PATH" "$ICON_URL" 2>/dev/null || warn "Could not download icon; desktop entry will use a generic icon."
  fi
fi

DESKTOP_FILE="${DESKTOP_DIR}/quillscribe.desktop"

cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=QuillScribe
Comment=AI-powered voice dictation
Exec=${SELECTED_EXEC}
Icon=quillscribe
Terminal=false
Categories=Utility;Accessibility;
Keywords=dictation;voice;speech;transcription;AI;
EOF

chmod 644 "$DESKTOP_FILE"

if command_exists update-desktop-database; then
  update-desktop-database -q "$DESKTOP_DIR" 2>/dev/null || true
fi

ok "Desktop entry created: ${DESKTOP_FILE}"

# ── Done ────────────────────────────────────────────────────────────────────

echo ""
ok "QuillScribe ${TAG_NAME} installed via ${SELECTED_FORMAT}!"
echo ""
echo "  Run it with:"
echo "    quillscribe"
echo "  — or —"
echo "    ${SELECTED_EXEC}"
echo ""
echo "To uninstall this install, delete:"
if [[ "$SELECTED_FORMAT" == "AppImage" ]]; then
  echo "    ${APPIMAGE_DEST}"
fi
echo "    ${LAUNCHER_PATH}"
echo "    ${DESKTOP_FILE}"
echo "    ${ICON_PATH}"
if [[ "$SELECTED_FORMAT" != "AppImage" && -e "$APPIMAGE_DEST" ]]; then
  echo ""
  echo "If replacing a previous AppImage install, you may also remove:"
  echo "    ${APPIMAGE_DEST}"
fi
echo ""
