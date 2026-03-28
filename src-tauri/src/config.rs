use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

// ── Sub-configs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub device_id: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub sounds_enabled: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_id: None,
            sample_rate: 16_000,
            channels: 1,
            sounds_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub mode: String,
    pub api_key: String,
    pub api_model: String,
    pub local_model: String,
    pub local_model_path: String,
    pub api_language: String,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            mode: "api".to_string(),
            api_key: String::new(),
            api_model: "gpt-4o-transcribe".to_string(),
            local_model: "base".to_string(),
            local_model_path: String::new(),
            api_language: "en".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// 0 = Copy to Clipboard, 1 = Type to Active Window, 2 = Copy & Type, 3 = Display Only
    pub mode: u8,
    pub silent_mode: bool,
    pub auto_clear: bool,
    pub auto_clear_delay: u32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            mode: 2,
            silent_mode: false,
            auto_clear: false,
            auto_clear_delay: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub always_on_top: bool,
    pub custom_titlebar: bool,
    #[serde(default = "default_overlay_mode")]
    pub overlay_mode: String,
}

fn default_overlay_mode() -> String {
    "minimal".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "white".to_string(),
            always_on_top: false,
            custom_titlebar: true,
            overlay_mode: "minimal".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    pub record_toggle: String,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            record_toggle: "Meta+Shift+`".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    pub buffer_size: u32,
    pub noise_threshold: f64,
    pub silence_timeout: f64,
    #[serde(default = "default_max_history_entries")]
    pub max_history_entries: usize,
}

fn default_max_history_entries() -> usize {
    100
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1024,
            noise_threshold: 0.01,
            silence_timeout: 2.0,
            max_history_entries: default_max_history_entries(),
        }
    }
}

// ── Root settings ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub audio: AudioConfig,
    pub whisper: WhisperConfig,
    pub output: OutputConfig,
    pub ui: UiConfig,
    pub shortcuts: ShortcutsConfig,
    pub advanced: AdvancedConfig,
}

// ── ConfigManager ────────────────────────────────────────────────────────────

pub struct ConfigManager {
    settings: Mutex<Settings>,
}

impl ConfigManager {
    /// Creates a new `ConfigManager`.
    ///
    /// * Creates the config directory (`~/.config/quillscribe/`) if it does not
    ///   exist.
    /// * Loads existing settings from `settings.json`, or writes the defaults
    ///   when the file is missing / unreadable.
    pub fn new() -> Result<Self> {
        let config_dir = Self::config_dir()?;
        fs::create_dir_all(&config_dir).with_context(|| {
            format!(
                "Failed to create config directory: {}",
                config_dir.display()
            )
        })?;

        let settings = match Self::load_settings_from_disk() {
            Ok(s) => s,
            Err(_) => {
                let defaults = Settings::default();
                Self::write_settings_to_disk(&defaults)?;
                defaults
            }
        };

        Ok(Self {
            settings: Mutex::new(settings),
        })
    }

    // ── Persistence ──────────────────────────────────────────────────────

    /// Returns the path to the config directory (`~/.config/quillscribe/`).
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::config_dir().context("Could not determine config directory")?;
        Ok(home.join("quillscribe"))
    }

    fn settings_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("settings.json"))
    }

    fn load_settings_from_disk() -> Result<Settings> {
        let path = Self::settings_path()?;
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read settings file: {}", path.display()))?;
        let settings: Settings =
            serde_json::from_str(&data).with_context(|| "Failed to parse settings JSON")?;
        Ok(settings)
    }

    fn write_settings_to_disk(settings: &Settings) -> Result<()> {
        let path = Self::settings_path()?;
        let json = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings to JSON")?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write settings file: {}", path.display()))?;
        Ok(())
    }

    /// Persists the current in-memory settings to disk.
    pub fn save_settings(&self) -> Result<()> {
        let guard = self.settings.lock().unwrap();
        Self::write_settings_to_disk(&guard)
    }

    // ── Validation ───────────────────────────────────────────────────────

    /// Returns `true` if `key` looks like a valid OpenAI API key.
    pub fn validate_api_key(key: &str) -> bool {
        key.starts_with("sk-") && key.len() > 20
    }

    // ── Audio getters ────────────────────────────────────────────────────

    pub fn get_audio_device_id(&self) -> Option<String> {
        self.settings.lock().unwrap().audio.device_id.clone()
    }

    #[allow(dead_code)]
    pub fn get_sample_rate(&self) -> u32 {
        self.settings.lock().unwrap().audio.sample_rate
    }

    pub fn get_sounds_enabled(&self) -> bool {
        self.settings.lock().unwrap().audio.sounds_enabled
    }

    // ── Whisper getters ──────────────────────────────────────────────────

    pub fn get_whisper(&self) -> WhisperConfig {
        self.settings.lock().unwrap().whisper.clone()
    }

    pub fn get_whisper_mode(&self) -> String {
        self.settings.lock().unwrap().whisper.mode.clone()
    }

    // ── Output getters ───────────────────────────────────────────────────

    pub fn get_output(&self) -> OutputConfig {
        self.settings.lock().unwrap().output.clone()
    }

    // ── UI getters / setters ─────────────────────────────────────────────

    pub fn get_always_on_top(&self) -> bool {
        self.settings.lock().unwrap().ui.always_on_top
    }

    pub fn get_custom_titlebar(&self) -> bool {
        self.settings.lock().unwrap().ui.custom_titlebar
    }

    pub fn set_always_on_top(&self, on_top: bool) {
        self.settings.lock().unwrap().ui.always_on_top = on_top;
    }

    // ── Shortcuts getters ────────────────────────────────────────────────

    pub fn get_record_toggle(&self) -> String {
        self.settings
            .lock()
            .unwrap()
            .shortcuts
            .record_toggle
            .clone()
    }

    // ── Advanced getters ─────────────────────────────────────────────────

    pub fn get_max_history_entries(&self) -> usize {
        self.settings.lock().unwrap().advanced.max_history_entries
    }

    // ── Bulk access ──────────────────────────────────────────────────────

    /// Returns a snapshot of all current settings.
    pub fn get_settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    /// Replaces all settings with the provided value.
    pub fn set_settings(&self, settings: Settings) {
        *self.settings.lock().unwrap() = settings;
    }
}
