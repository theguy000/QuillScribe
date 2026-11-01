use anyhow::{Context, Result};
use arboard::Clipboard;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};

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

// Keep the X11 clipboard owner alive for the lifetime of the process. Without
// this, clipboard contents can disappear as soon as the temporary Clipboard is
// dropped on systems without a clipboard manager that eagerly persists data.
#[cfg(target_os = "linux")]
static CLIPBOARD: OnceLock<Mutex<Option<Clipboard>>> = OnceLock::new();

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

#[cfg(target_os = "linux")]
const XDOTOOL_TYPE_ARGS: [&str; 5] = ["type", "--clearmodifiers", "--delay", "0", "--"];

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
fn xdotool_type_args() -> [&'static str; 5] {
    XDOTOOL_TYPE_ARGS
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

#[cfg(target_os = "linux")]
fn run_xdotool_type_command(text: &str) -> Result<std::process::ExitStatus> {
    std::process::Command::new("xdotool")
        .args(XDOTOOL_TYPE_ARGS)
        .arg(text)
        .status()
        .context("Failed to execute xdotool type")
}

#[cfg(target_os = "linux")]
fn with_clipboard<T>(operation: impl FnOnce(&mut Clipboard) -> Result<T>) -> Result<T> {
    let lock = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut clipboard = lock.lock().expect("clipboard mutex poisoned");

    if clipboard.is_none() {
        *clipboard = Some(Clipboard::new().context("Failed to initialize clipboard")?);
    }

    operation(clipboard.as_mut().expect("clipboard initialized"))
}

#[cfg(not(target_os = "linux"))]
fn with_clipboard<T>(operation: impl FnOnce(&mut Clipboard) -> Result<T>) -> Result<T> {
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
    operation(&mut clipboard)
}

// ── Clipboard-preserving delivery ────────────────────────────────────────────

/// How long a pasted text is given to land before the clipboard goes back.
const PASTE_SETTLE_DELAY: Duration = Duration::from_millis(150);

/// How a segment reaches the caret once the clipboard holds it.
#[derive(Debug, Clone, Copy)]
enum Insert {
    /// Ctrl+V, the insertion every live-typed segment goes through, so the
    /// target application cannot rewrite the characters on the way in.
    Paste,
    /// The platform's typing path, where one exists. Only the stop-time output
    /// uses it, so Type to app still reaches an application that refuses a paste.
    Keystrokes,
}

/// Everything one delivery needs from the machine around it: the clipboard, the
/// synthetic input, and the wait that lets the target read the clipboard before
/// it is handed back.
///
/// The real host talks to the operating system. The tests use a fake, so the
/// sequence can be checked without a window and without a clipboard.
trait ClipboardHost {
    fn read_clipboard(&mut self) -> Result<String>;
    fn write_clipboard(&mut self, text: &str) -> Result<()>;
    fn clear_clipboard(&mut self) -> Result<()>;
    fn insert(&mut self, text: &str, how: Insert) -> Result<()>;
    fn settle(&mut self);
}

/// Put `text` at the caret, then give the user their clipboard back.
///
/// The clipboard is saved and restored around this one delivery, never held for
/// a whole utterance. A snapshot taken once and restored at the end would throw
/// away whatever the user copied while dictating.
///
/// The write and the synthetic input stay adjacent on purpose. On X11 the
/// clipboard belongs to a process rather than to the server, and the target
/// reads it while the synthetic input runs, so ownership has to be asserted
/// again for every delivery rather than once at the start.
///
/// Anything the user copies in the few hundred milliseconds between the paste
/// and the restore is replaced by the restore. That is the cost of not holding
/// the clipboard for the length of the dictation.
fn insert_preserving_clipboard(
    host: &mut impl ClipboardHost,
    text: &str,
    how: Insert,
) -> Result<()> {
    let original = host.read_clipboard().ok();
    let inserted = host
        .write_clipboard(text)
        .and_then(|()| host.insert(text, how));

    if inserted.is_ok() {
        host.settle();
    }

    match original {
        Some(original) => {
            if let Err(err) = host.write_clipboard(&original) {
                warn!("Failed to restore the original clipboard contents: {}", err);
            }
        }
        None => {
            if let Err(err) = host.clear_clipboard() {
                warn!("Failed to clear the clipboard after pasting: {}", err);
            }
        }
    }

    inserted
}

/// The stop-time rule: type the words, and if they never arrive, leave them on
/// the clipboard.
///
/// The clipboard keeps the text on a failed delivery because it is then the only
/// copy the user has left, which is what the stop-time output has always done.
/// Live typing does not do this. A fragment the target refused mid-utterance is
/// the stop path's business, and leaving it on the clipboard would put our words
/// over whatever the user copies next.
fn type_preserving_stop_delivery(host: &mut impl ClipboardHost, text: &str) -> Result<()> {
    let result = insert_preserving_clipboard(host, text, Insert::Keystrokes);
    if result.is_err() {
        if let Err(err) = host.write_clipboard(text) {
            warn!(
                "Failed to leave the undelivered text on the clipboard: {}",
                err
            );
        }
    }
    result
}

/// The real host: the system clipboard plus the platform's synthetic input.
struct SystemHost<'a> {
    output: &'a OutputManager,
}

impl ClipboardHost for SystemHost<'_> {
    fn read_clipboard(&mut self) -> Result<String> {
        self.output.get_clipboard_content()
    }

    fn write_clipboard(&mut self, text: &str) -> Result<()> {
        self.output.copy_to_clipboard(text)
    }

    fn clear_clipboard(&mut self) -> Result<()> {
        self.output.clear_clipboard()
    }

    fn insert(&mut self, text: &str, how: Insert) -> Result<()> {
        match how {
            Insert::Paste => self.output.simulate_paste(),
            Insert::Keystrokes => self.output.type_text_to_active_window(text),
        }
    }

    fn settle(&mut self) {
        sleep(PASTE_SETTLE_DELAY);
    }
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
                self.type_preserving_clipboard(text)?;
                "Text typed to active window".to_string()
            }
            OutputMode::CopyAndType => {
                // Copy text to clipboard AND type it into the active window.
                // Clipboard retains the transcription text.
                self.copy_to_clipboard(text)?;
                self.type_text_to_active_window(text)?;
                // X11 clipboard ownership can be lost while synthetic input is running;
                // re-copy after typing so Copy & Type always leaves this text available.
                self.copy_to_clipboard(text)?;
                "Text copied to clipboard and typed to active window".to_string()
            }
            OutputMode::DisplayOnly => "Text displayed in app".to_string(),
        };

        Ok(status)
    }

    pub fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        Self::copy_text_to_clipboard(text)
    }

    /// Paste `text` into the window that holds focus, then put the user's
    /// clipboard back.
    ///
    /// This is the live typing transport, called once per Final segment. The
    /// clipboard is borrowed for the length of one paste and written again
    /// immediately before every Ctrl+V, because the target application's
    /// autocorrect and completion never get to rewrite a pasted character.
    pub fn paste_preserving_clipboard(&self, text: &str) -> Result<()> {
        let started = Instant::now();
        let result =
            insert_preserving_clipboard(&mut SystemHost { output: self }, text, Insert::Paste);
        // Per-segment cost, which is what accumulates over an utterance.
        debug!(
            "Pasted {} characters in {:?}",
            text.chars().count(),
            started.elapsed()
        );
        result
    }

    /// Type `text` into the window that holds focus, then put the user's
    /// clipboard back.
    ///
    /// This is the stop-time path. On Linux with xdotool it types the words, so
    /// Type to app still reaches an application that refuses a paste, and
    /// everywhere else typing means a paste. Live typing uses
    /// [`Self::paste_preserving_clipboard`] instead.
    pub fn type_preserving_clipboard(&self, text: &str) -> Result<()> {
        type_preserving_stop_delivery(&mut SystemHost { output: self }, text)
    }

    pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
        with_clipboard(|clipboard| {
            clipboard
                .set_text(text.to_string())
                .context("Failed to copy text to clipboard")
        })
        .context("Failed to copy text to clipboard")?;
        debug!("Copied {} characters to clipboard", text.len());
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

    /// Type text into the active window.
    ///
    /// On Linux with xdotool, this sends actual text-entry key events instead of
    /// Ctrl+V so Copy & Type still works when paste is blocked by the target app.
    fn type_text_to_active_window(&self, text: &str) -> Result<()> {
        #[cfg(not(target_os = "linux"))]
        let _ = text;

        #[cfg(target_os = "linux")]
        {
            self.linux_type_text(text)?;
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.simulate_paste()?;
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

    #[cfg(target_os = "linux")]
    fn linux_type_text(&self, text: &str) -> Result<()> {
        let status = check_paste_tool();

        match &status.detected_tool {
            LinuxPasteTool::Xdotool => {
                debug!("Typing text via xdotool type (Linux/X11)");
                release_paste_keys("xdotool", XDOTOOL_RELEASE_ARGS.iter().copied());
                let result = run_xdotool_type_command(text)?;
                release_paste_keys("xdotool", XDOTOOL_RELEASE_ARGS.iter().copied());

                if !result.success() {
                    anyhow::bail!(
                        "xdotool type exited with non-zero status: {:?}",
                        result.code()
                    );
                }

                Ok(())
            }
            LinuxPasteTool::Ydotool => self.linux_paste(),
            LinuxPasteTool::None => {
                anyhow::bail!(
                    "No paste tool available on this system.\n{}",
                    status.setup_hint
                );
            }
        }
    }

    pub fn get_clipboard_content(&self) -> Result<String> {
        with_clipboard(|clipboard| {
            clipboard
                .get_text()
                .context("Failed to get clipboard content")
        })
    }

    pub fn clear_clipboard(&self) -> Result<()> {
        with_clipboard(|clipboard| clipboard.clear().context("Failed to clear clipboard"))?;
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
    fn xdotool_type_uses_text_entry_with_clear_modifiers() {
        assert_eq!(
            xdotool_type_args(),
            ["type", "--clearmodifiers", "--delay", "0", "--"]
        );
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

    #[test]
    fn clipboard_content_survives_after_copy_handle_scope() {
        let output = OutputManager::new();
        let text = "quillscribe_clipboard_retention_test";

        OutputManager::copy_text_to_clipboard(text).expect("copy text to clipboard");
        sleep(Duration::from_millis(250));

        assert_eq!(
            output.get_clipboard_content().expect("read clipboard"),
            text
        );
    }
}

/// The save, paste, wait and restore sequence, checked without a window and
/// without a real clipboard.
#[cfg(test)]
mod clipboard_sequence_tests {
    use super::*;

    /// One thing the sequence asked the machine to do, in the order it asked.
    #[derive(Debug, PartialEq, Eq)]
    enum Op {
        Read,
        Write(String),
        Clear,
        Paste,
        Type(String),
        Settle,
    }

    /// Stands in for the system clipboard and the synthetic input. A paste
    /// records what the clipboard held at that moment, which is what the target
    /// application would have read.
    #[derive(Default)]
    struct FakeHost {
        clipboard: Option<String>,
        ops: Vec<Op>,
        delivered: Vec<String>,
        fail_inserts: usize,
    }

    impl FakeHost {
        fn holding(text: &str) -> Self {
            Self {
                clipboard: Some(text.to_string()),
                ..Self::default()
            }
        }

        fn failing_once() -> Self {
            Self {
                fail_inserts: 1,
                ..Self::holding(USER_CLIPBOARD)
            }
        }
    }

    impl ClipboardHost for FakeHost {
        fn read_clipboard(&mut self) -> Result<String> {
            self.ops.push(Op::Read);
            self.clipboard
                .clone()
                .ok_or_else(|| anyhow::anyhow!("the clipboard holds no text"))
        }

        fn write_clipboard(&mut self, text: &str) -> Result<()> {
            self.ops.push(Op::Write(text.to_string()));
            self.clipboard = Some(text.to_string());
            Ok(())
        }

        fn clear_clipboard(&mut self) -> Result<()> {
            self.ops.push(Op::Clear);
            self.clipboard = None;
            Ok(())
        }

        fn insert(&mut self, text: &str, how: Insert) -> Result<()> {
            let at_the_caret = match how {
                Insert::Paste => {
                    self.ops.push(Op::Paste);
                    self.clipboard.clone().unwrap_or_default()
                }
                Insert::Keystrokes => {
                    self.ops.push(Op::Type(text.to_string()));
                    text.to_string()
                }
            };
            if self.fail_inserts > 0 {
                self.fail_inserts -= 1;
                anyhow::bail!("the target refused the paste");
            }
            self.delivered.push(at_the_caret);
            Ok(())
        }

        fn settle(&mut self) {
            self.ops.push(Op::Settle);
        }
    }

    const USER_CLIPBOARD: &str = "the user's own text";

    fn clipboard_write(text: &str) -> Op {
        Op::Write(text.to_string())
    }

    #[test]
    fn every_segment_saves_and_restores_the_clipboard() {
        let mut host = FakeHost::holding(USER_CLIPBOARD);

        insert_preserving_clipboard(&mut host, "Hello there.", Insert::Paste).unwrap();
        insert_preserving_clipboard(&mut host, "How are you?", Insert::Paste).unwrap();

        assert_eq!(host.delivered, ["Hello there.", "How are you?"]);
        assert_eq!(
            host.ops,
            [
                Op::Read,
                clipboard_write("Hello there."),
                Op::Paste,
                Op::Settle,
                clipboard_write(USER_CLIPBOARD),
                Op::Read,
                clipboard_write("How are you?"),
                Op::Paste,
                Op::Settle,
                clipboard_write(USER_CLIPBOARD),
            ]
        );
        assert_eq!(host.clipboard.as_deref(), Some(USER_CLIPBOARD));
    }

    #[test]
    fn each_paste_reads_the_clipboard_written_for_it() {
        let mut host = FakeHost::holding(USER_CLIPBOARD);

        insert_preserving_clipboard(&mut host, "one", Insert::Paste).unwrap();
        insert_preserving_clipboard(&mut host, "two", Insert::Paste).unwrap();

        assert_eq!(
            host.delivered,
            ["one", "two"],
            "a later segment never pastes an earlier segment's text"
        );
    }

    #[test]
    fn what_the_user_copies_between_segments_is_what_comes_back() {
        let mut host = FakeHost::holding(USER_CLIPBOARD);

        insert_preserving_clipboard(&mut host, "one", Insert::Paste).unwrap();
        host.clipboard = Some("copied while dictating".to_string());
        insert_preserving_clipboard(&mut host, "two", Insert::Paste).unwrap();

        assert_eq!(
            host.clipboard.as_deref(),
            Some("copied while dictating"),
            "a snapshot taken once per utterance would have restored the older text"
        );
    }

    #[test]
    fn a_failed_paste_still_gives_the_clipboard_back() {
        let mut host = FakeHost::failing_once();

        let result = insert_preserving_clipboard(&mut host, "Hello there.", Insert::Paste);

        assert!(result.is_err(), "the caller retries a failed paste");
        assert_eq!(host.clipboard.as_deref(), Some(USER_CLIPBOARD));
        assert!(
            !host.ops.contains(&Op::Settle),
            "nothing landed, so nothing is waited for"
        );
    }

    #[test]
    fn a_clipboard_with_no_text_on_it_is_left_empty() {
        let mut host = FakeHost::default();

        insert_preserving_clipboard(&mut host, "Hello there.", Insert::Paste).unwrap();

        assert_eq!(host.ops.last(), Some(&Op::Clear));
        assert_eq!(host.clipboard, None);
    }

    #[test]
    fn the_stop_time_path_types_instead_of_pasting() {
        let mut host = FakeHost::holding(USER_CLIPBOARD);

        type_preserving_stop_delivery(&mut host, "Hello there.").unwrap();

        assert_eq!(
            host.ops,
            [
                Op::Read,
                clipboard_write("Hello there."),
                Op::Type("Hello there.".to_string()),
                Op::Settle,
                clipboard_write(USER_CLIPBOARD),
            ]
        );
    }

    #[test]
    fn a_failed_stop_time_delivery_leaves_the_words_on_the_clipboard() {
        let mut host = FakeHost::failing_once();

        let result = type_preserving_stop_delivery(&mut host, "Hello there.");

        assert!(result.is_err());
        assert_eq!(
            host.clipboard.as_deref(),
            Some("Hello there."),
            "the transcript never reached the window, so the clipboard is the copy left to the user"
        );
    }
}
