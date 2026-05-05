use anyhow::{Context, Result};
use arboard::Clipboard;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::thread::sleep;
use std::time::Duration;

/// Output modes control how transcription results are delivered.
///
/// - `CopyToClipboard` (0): Copies the text to the system clipboard only.
/// - `TypeToApp` (1): Types the text into the active window via simulated paste,
///   then restores the original clipboard contents (clipboard is not polluted).
/// - `CopyAndType` (2): Copies text to clipboard AND types it into the active window.
/// - `DisplayOnly` (3): Shows the text in the app UI only; no clipboard or typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum OutputMode {
    CopyToClipboard = 0,
    TypeToApp = 1,
    CopyAndType = 2,
    DisplayOnly = 3,
}

impl From<u8> for OutputMode {
    fn from(value: u8) -> Self {
        match value {
            0 => OutputMode::CopyToClipboard,
            1 => OutputMode::TypeToApp,
            2 => OutputMode::CopyAndType,
            3 => OutputMode::DisplayOnly,
            _ => {
                warn!(
                    "Unknown output mode value: {}, defaulting to CopyToClipboard",
                    value
                );
                OutputMode::CopyToClipboard
            }
        }
    }
}

// ── Linux: paste tool detection (cached once per process) ────────────────────

/// Which tool will be used to simulate paste on Linux.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinuxPasteTool {
    /// xdotool — works on X11, zero setup required.
    Xdotool,
    /// ydotool — works on X11 and Wayland (kernel-level input).
    Ydotool,
    /// No working tool was found.
    None,
}

/// Status of paste tool availability on Linux.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteToolStatus {
    /// Which tool was detected and will be used.
    pub detected_tool: LinuxPasteTool,
    /// Whether xdotool is available.
    pub xdotool_found: bool,
    /// Whether ydotool binary is installed.
    pub ydotool_found: bool,
    /// Whether ydotoold daemon is running (only checked if ydotool binary exists).
    pub ydotool_daemon_running: bool,
    /// Human-readable setup instructions when no tool is available,
    /// or informational note about which tool is active.
    pub setup_hint: String,
}

/// Cached paste tool status — computed once, reused for the lifetime of the process.
#[cfg(target_os = "linux")]
static PASTE_TOOL_STATUS: std::sync::OnceLock<PasteToolStatus> = std::sync::OnceLock::new();

/// Detect which paste tool is available on Linux. Result is cached after the first call.
///
/// Detection order:
/// 1. **xdotool** — preferred because it needs zero setup on X11 desktops (no daemon,
///    no special permissions). If available and working, it is used.
/// 2. **ydotool** — fallback that works on both X11 and Wayland, but requires the
///    `ydotoold` daemon and `/dev/uinput` access.
/// 3. **None** — neither tool is usable; user is shown setup instructions.
#[cfg(target_os = "linux")]
pub fn check_paste_tool() -> &'static PasteToolStatus {
    PASTE_TOOL_STATUS.get_or_init(|| {
        use std::process::Command;

        // 1. Check xdotool.
        let xdotool_found = Command::new("xdotool")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        // 2. Check ydotool binary.
        let ydotool_found = Command::new("ydotool")
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok();

        // 3. If ydotool binary exists, check if daemon is reachable.
        let ydotool_daemon_running = if ydotool_found {
            Command::new("ydotool")
                .args(["key", "0:0"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            false
        };

        // Determine which tool to use: prefer xdotool, fall back to ydotool.
        let detected_tool = if xdotool_found {
            LinuxPasteTool::Xdotool
        } else if ydotool_found && ydotool_daemon_running {
            LinuxPasteTool::Ydotool
        } else {
            LinuxPasteTool::None
        };

        let setup_hint = match detected_tool {
            LinuxPasteTool::Xdotool => {
                "Using xdotool (X11). No additional setup required.".to_string()
            }
            LinuxPasteTool::Ydotool => {
                "Using ydotool (Wayland/X11). No additional setup required.".to_string()
            }
            LinuxPasteTool::None => {
                let mut hint =
                    String::from("No paste tool is available. Install one of the following:\n\n");

                hint.push_str(
                    "Option 1 — xdotool (recommended for X11):\n\
                     Arch Linux:  sudo pacman -S xdotool\n\
                     Fedora:      sudo dnf install xdotool\n\
                     Ubuntu:      sudo apt install xdotool\n\n",
                );

                hint.push_str(
                    "Option 2 — ydotool (works on X11 and Wayland):\n\
                     Arch Linux:  sudo pacman -S ydotool\n\
                     Fedora:      sudo dnf install ydotool\n\
                     Ubuntu:      sudo apt install ydotool\n\
                     Then enable the daemon:\n\
                     sudo systemctl enable --now ydotool\n\
                     You may also need:  sudo usermod -aG input $USER",
                );

                // Add specific hint if ydotool is installed but daemon isn't running.
                if ydotool_found && !ydotool_daemon_running {
                    hint = "ydotool is installed but the ydotoold daemon is not running.\n\n\
                            Start and enable the daemon:\n\
                            sudo systemctl enable --now ydotool\n\n\
                            Your user may also need access to /dev/uinput.\n\
                            Add yourself to the input group:  sudo usermod -aG input $USER\n\
                            Then log out and back in.\n\n\
                            Alternatively, install xdotool (simpler, X11 only):\n\
                            Arch Linux:  sudo pacman -S xdotool\n\
                            Fedora:      sudo dnf install xdotool\n\
                            Ubuntu:      sudo apt install xdotool"
                        .to_string();
                }

                hint
            }
        };

        info!(
            "Linux paste tool check: xdotool={}, ydotool={} (daemon={}), using={:?}",
            xdotool_found, ydotool_found, ydotool_daemon_running, detected_tool
        );

        PasteToolStatus {
            detected_tool,
            xdotool_found,
            ydotool_found,
            ydotool_daemon_running,
            setup_hint,
        }
    })
}

/// On non-Linux platforms, return a dummy status (not applicable).
#[cfg(not(target_os = "linux"))]
pub fn check_paste_tool() -> PasteToolStatus {
    PasteToolStatus {
        detected_tool: LinuxPasteTool::None,
        xdotool_found: false,
        ydotool_found: false,
        ydotool_daemon_running: false,
        setup_hint: "Paste tool detection is only applicable on Linux.".to_string(),
    }
}

#[cfg(target_os = "linux")]
const XDOTOOL_PASTE_ARGS: [&str; 3] = ["key", "--clearmodifiers", "ctrl+v"];

#[cfg(target_os = "linux")]
const XDOTOOL_RELEASE_ARGS: [&str; 4] = ["keyup", "Control_L", "Control_R", "v"];

// 29 = KEY_LEFTCTRL, 47 = KEY_V; :1 = key down, :0 = key up.
#[cfg(target_os = "linux")]
const YDOTOOL_PASTE_ARGS: [&str; 5] = ["key", "29:1", "47:1", "47:0", "29:0"];

// Release V plus both Ctrl keys in case a previous synthetic paste was interrupted.
#[cfg(target_os = "linux")]
const YDOTOOL_RELEASE_ARGS: [&str; 4] = ["key", "47:0", "29:0", "97:0"];

#[cfg(all(test, target_os = "linux"))]
fn xdotool_paste_args() -> [&'static str; 3] {
    XDOTOOL_PASTE_ARGS
}

#[cfg(all(test, target_os = "linux"))]
fn xdotool_release_args() -> [&'static str; 4] {
    XDOTOOL_RELEASE_ARGS
}

#[cfg(all(test, target_os = "linux"))]
fn ydotool_paste_args() -> [&'static str; 5] {
    YDOTOOL_PASTE_ARGS
}

#[cfg(all(test, target_os = "linux"))]
fn ydotool_release_args() -> [&'static str; 4] {
    YDOTOOL_RELEASE_ARGS
}

#[cfg(target_os = "linux")]
fn run_paste_command(
    command: &str,
    args: impl IntoIterator<Item = &'static str>,
) -> Result<std::process::ExitStatus> {
    std::process::Command::new(command)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute {command}"))
}

#[cfg(target_os = "linux")]
fn release_paste_keys(command: &str, args: impl IntoIterator<Item = &'static str>) {
    let _ = std::process::Command::new(command).args(args).status();
}

// ── OutputManager ────────────────────────────────────────────────────────────

pub struct OutputManager;

impl OutputManager {
    pub fn new() -> Self {
        OutputManager
    }

    pub fn process_transcription(&self, text: &str, mode: OutputMode) -> Result<String> {
        if text.is_empty() {
            return Ok("No text to process".to_string());
        }

        info!("Processing transcription with mode: {:?}", mode);
        debug!("Transcription text length: {}", text.len());

        let status = match mode {
            OutputMode::CopyToClipboard => {
                self.copy_to_clipboard(text)?;
                "Text copied to clipboard".to_string()
            }
            OutputMode::TypeToApp => {
                // Type text into the active window without polluting the clipboard.
                // Save current clipboard → copy text → paste → restore clipboard.
                let original_clipboard = self.get_clipboard_content().ok();
                self.copy_to_clipboard(text)?;
                self.simulate_paste()?;
                // Wait for the paste to complete before restoring the clipboard.
                sleep(Duration::from_millis(150));
                // Restore the original clipboard contents.
                match original_clipboard {
                    Some(ref original) => {
                        if let Err(e) = self.copy_to_clipboard(original) {
                            warn!("Failed to restore original clipboard contents: {}", e);
                        }
                    }
                    None => {
                        if let Err(e) = self.clear_clipboard() {
                            warn!("Failed to clear clipboard after paste: {}", e);
                        }
                    }
                }
                "Text typed to active window".to_string()
            }
            OutputMode::CopyAndType => {
                // Copy text to clipboard AND type it into the active window.
                // Clipboard retains the transcription text.
                self.copy_to_clipboard(text)?;
                self.simulate_paste()?;
                "Text copied to clipboard and typed to active window".to_string()
            }
            OutputMode::DisplayOnly => "Text displayed in app".to_string(),
        };

        Ok(status)
    }

    pub fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        Self::copy_text_to_clipboard(text)
    }

    pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
        let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        clipboard
            .set_text(text.to_string())
            .context("Failed to copy text to clipboard")?;
        debug!("Copied {} characters to clipboard", text.len());
        // On Linux (X11), the clipboard is owned by the process that set it.
        // Give clipboard managers time to read the contents before dropping.
        #[cfg(target_os = "linux")]
        sleep(Duration::from_millis(100));
        Ok(())
    }

    /// Simulate a Ctrl+V paste into the currently active application.
    /// - Windows: uses the Win32 SendInput API.
    /// - Linux: uses xdotool (X11) or ydotool (Wayland/X11), whichever was detected.
    pub fn simulate_paste(&self) -> Result<()> {
        #[cfg(windows)]
        {
            self.windows_paste()?;
        }

        #[cfg(target_os = "linux")]
        {
            self.linux_paste()?;
        }

        #[cfg(not(any(windows, target_os = "linux")))]
        {
            warn!("Paste simulation is not yet supported on this platform");
            anyhow::bail!("Paste simulation is not supported on this platform");
        }

        Ok(())
    }

    #[cfg(windows)]
    fn windows_paste(&self) -> Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
            KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
        };

        debug!("Simulating Ctrl+V paste via SendInput (Windows)");

        let release_keys = || {
            let release_inputs = [
                // V up
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_V,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // Ctrl up
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_CONTROL,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];

            unsafe { SendInput(&release_inputs, std::mem::size_of::<INPUT>() as i32) }
        };

        // Clear any stale synthetic state from a previous interrupted paste.
        let _ = release_keys();

        let inputs = [
            // Ctrl down
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // V down
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];

        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 {
            let _ = release_keys();
            anyhow::bail!(
                "SendInput failed: sent {} of {} key-down events",
                sent,
                inputs.len()
            );
        }

        sleep(Duration::from_millis(20));

        const RELEASE_INPUT_COUNT: u32 = 2;
        let sent = release_keys();
        if sent != RELEASE_INPUT_COUNT {
            anyhow::bail!(
                "SendInput failed: sent {} of {} key-up events",
                sent,
                RELEASE_INPUT_COUNT
            );
        }

        debug!("Ctrl+V paste simulated successfully (Windows)");
        Ok(())
    }

    /// Simulate Ctrl+V on Linux using the cached detected tool (xdotool or ydotool).
    #[cfg(target_os = "linux")]
    fn linux_paste(&self) -> Result<()> {
        let status = check_paste_tool();

        let (command, paste_args, release_args, tool_label) = match &status.detected_tool {
            LinuxPasteTool::Xdotool => {
                debug!("Simulating Ctrl+V paste via xdotool (Linux/X11)");
                (
                    "xdotool",
                    XDOTOOL_PASTE_ARGS.as_slice(),
                    XDOTOOL_RELEASE_ARGS.as_slice(),
                    "xdotool",
                )
            }
            LinuxPasteTool::Ydotool => {
                debug!("Simulating Ctrl+V paste via ydotool (Linux)");
                (
                    "ydotool",
                    YDOTOOL_PASTE_ARGS.as_slice(),
                    YDOTOOL_RELEASE_ARGS.as_slice(),
                    "ydotool",
                )
            }
            LinuxPasteTool::None => {
                anyhow::bail!(
                    "No paste tool available on this system.\n{}",
                    status.setup_hint
                );
            }
        };

        release_paste_keys(command, release_args.iter().copied());
        let result = run_paste_command(command, paste_args.iter().copied())?;
        release_paste_keys(command, release_args.iter().copied());

        if !result.success() {
            anyhow::bail!(
                "{} exited with non-zero status: {:?}",
                tool_label,
                result.code()
            );
        }

        debug!("Ctrl+V paste simulated successfully via {}", tool_label);

        Ok(())
    }

    pub fn get_clipboard_content(&self) -> Result<String> {
        let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        let text = clipboard
            .get_text()
            .context("Failed to get clipboard content")?;
        Ok(text)
    }

    pub fn clear_clipboard(&self) -> Result<()> {
        let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        clipboard.clear().context("Failed to clear clipboard")?;
        debug!("Clipboard cleared");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn test_clipboard(&self) -> Result<bool> {
        let test_text = "quillscribe_clipboard_test";

        self.copy_to_clipboard(test_text)?;
        let content = self.get_clipboard_content()?;

        let success = content == test_text;
        if success {
            debug!("Clipboard test passed");
        } else {
            warn!(
                "Clipboard test failed: expected '{}', got '{}'",
                test_text, content
            );
        }

        self.clear_clipboard()?;
        Ok(success)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn xdotool_paste_uses_clear_modifiers() {
        assert_eq!(xdotool_paste_args(), ["key", "--clearmodifiers", "ctrl+v"]);
    }

    #[test]
    fn xdotool_release_releases_ctrl_and_v() {
        let args = xdotool_release_args();
        assert!(args.contains(&"Control_L"));
        assert!(args.contains(&"Control_R"));
        assert!(args.contains(&"v"));
    }

    #[test]
    fn ydotool_paste_releases_keys_after_pressing_them() {
        assert_eq!(
            ydotool_paste_args(),
            ["key", "29:1", "47:1", "47:0", "29:0"]
        );
    }

    #[test]
    fn ydotool_release_releases_v_and_ctrl_keys() {
        assert_eq!(ydotool_release_args(), ["key", "47:0", "29:0", "97:0"]);
    }
}
