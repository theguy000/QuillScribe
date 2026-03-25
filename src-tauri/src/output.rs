use anyhow::{Context, Result};
use arboard::Clipboard;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum OutputMode {
    CopyOnly = 0,
    PasteOnly = 1,
    CopyAndPaste = 2,
    DisplayOnly = 3,
}

impl From<u8> for OutputMode {
    fn from(value: u8) -> Self {
        match value {
            0 => OutputMode::CopyOnly,
            1 => OutputMode::PasteOnly,
            2 => OutputMode::CopyAndPaste,
            3 => OutputMode::DisplayOnly,
            _ => {
                warn!(
                    "Unknown output mode value: {}, defaulting to CopyOnly",
                    value
                );
                OutputMode::CopyOnly
            }
        }
    }
}

pub struct OutputManager;

impl OutputManager {
    pub fn new() -> Self {
        OutputManager
    }

    pub fn process_transcription(
        &self,
        text: &str,
        mode: OutputMode,
        silent_mode: bool,
    ) -> Result<String> {
        if text.is_empty() {
            return Ok("No text to process".to_string());
        }

        if !silent_mode {
            info!("Processing transcription with mode: {:?}", mode);
        }
        debug!("Transcription text length: {}", text.len());

        let status = match mode {
            OutputMode::CopyOnly => {
                self.copy_to_clipboard(text)?;
                "Text copied to clipboard".to_string()
            }
            OutputMode::PasteOnly => {
                self.copy_to_clipboard(text)?;
                self.paste_to_active_app()?;
                "Text pasted to active application".to_string()
            }
            OutputMode::CopyAndPaste => {
                self.copy_to_clipboard(text)?;
                self.paste_to_active_app()?;
                "Text copied and pasted".to_string()
            }
            OutputMode::DisplayOnly => "Text displayed (no clipboard operation)".to_string(),
        };

        Ok(status)
    }

    pub fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        clipboard
            .set_text(text.to_string())
            .context("Failed to copy text to clipboard")?;
        debug!("Copied {} characters to clipboard", text.len());
        Ok(())
    }

    pub fn paste_to_active_app(&self) -> Result<()> {
        #[cfg(windows)]
        {
            self.windows_paste()?;
        }

        #[cfg(not(windows))]
        {
            warn!("Auto-paste is only supported on Windows");
        }

        Ok(())
    }

    #[cfg(windows)]
    fn windows_paste(&self) -> Result<()> {
        use std::thread::sleep;
        use std::time::Duration;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
            KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
        };

        debug!("Simulating Ctrl+V paste via SendInput");

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
            anyhow::bail!(
                "SendInput failed: sent {} of {} key-down events",
                sent,
                inputs.len()
            );
        }

        sleep(Duration::from_millis(20));

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

        let sent = unsafe { SendInput(&release_inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != release_inputs.len() as u32 {
            anyhow::bail!(
                "SendInput failed: sent {} of {} key-up events",
                sent,
                release_inputs.len()
            );
        }

        debug!("Ctrl+V paste simulated successfully");
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
