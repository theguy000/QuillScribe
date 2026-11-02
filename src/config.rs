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
    #[serde(default = "default_blocklist")]
    pub blocklist: Vec<String>,
}

fn default_blocklist() -> Vec<String> {
    vec![
        "DEV=".into(),
        "surround".into(),
        "front:".into(),
        "iec958:".into(),
        "dmix".into(),
        "hw:".into(),
    ]
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_id: None,
            sample_rate: 16_000,
            channels: 1,
            sounds_enabled: true,
            blocklist: default_blocklist(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub mode: String,
    #[serde(default = "default_api_provider")]
    pub api_provider: String,
    pub api_key: String,
    #[serde(default)]
    pub groq_api_key: String,
    #[serde(default)]
    pub gemini_api_key: String,
    pub api_model: String,
    pub local_model: String,
    pub local_model_path: String,
    pub api_language: String,
}

fn default_api_provider() -> String {
    "openai".to_string()
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            mode: "api".to_string(),
            api_provider: "openai".to_string(),
            api_key: String::new(),
            groq_api_key: String::new(),
            gemini_api_key: String::new(),
            api_model: "gpt-4o-transcribe".to_string(),
            local_model: "base".to_string(),
            local_model_path: String::new(),
            api_language: "en".to_string(),
        }
    }
}

/// Settings for the Deepgram live-session provider.
///
/// Deepgram is the only provider that streams while the user speaks, so its
/// knobs do not belong on `WhisperConfig`: `endpointing_ms` and
/// `utterance_end_ms` only exist for a live session, and the three formatter
/// flags are Deepgram's own vocabulary (see GLOSSARY.md — "Spoken punctuation"
/// is `dictation`, not the app's act of dictating).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "DeepgramConfigFields")]
pub struct DeepgramConfig {
    #[serde(default)]
    pub api_key: String,
    /// How Deepgram is used: `"recorded"` uploads the finished utterance to the
    /// prerecorded endpoint; `"live"` streams while the user speaks, which is
    /// what live typing needs to write text as it arrives.
    ///
    /// `live` is the default because live typing is on for a fresh install and
    /// cannot run without a Live session. See ADR 0003.
    #[serde(default = "default_deepgram_mode")]
    pub mode: String,
    /// Live typing: write each Final segment into the application the user is
    /// in. On by default, and the only setting behind the whole feature.
    ///
    /// It never disagrees with `mode`, and a hand-edited settings file cannot
    /// make them: a stored file that pairs recorded with it on loads with it
    /// off, and switching the mode to recorded turns it off.
    #[serde(default = "default_true")]
    pub live_typing: bool,
    #[serde(default = "default_deepgram_model")]
    pub model: String,
    #[serde(default = "default_deepgram_language")]
    pub language: String,
    /// Dates, times, currency, emails, URLs — and punctuation.
    #[serde(default = "default_true")]
    pub smart_format: bool,
    /// Required by `dictation`; harmless on its own.
    #[serde(default = "default_true")]
    pub punctuate: bool,
    /// Turns spoken "comma"/"new paragraph" into marks. English only.
    ///
    /// **Off by default, and it overrides `punctuate`/`smart_format` rather than
    /// adding to them.** Measured against the live service: with `dictation=true`
    /// the same clip comes back as `"the quick brown fox …"` — no capitals, no
    /// punctuation — while with it off the identical request returns
    /// `"The quick brown fox … dog."`. Think of it as the alternative to
    /// automatic punctuation, not an extra: you either let Deepgram punctuate or
    /// you dictate the punctuation yourself.
    #[serde(default)]
    pub dictation: bool,
    /// Drives the live word count; also required by `utterance_end_ms`.
    #[serde(default = "default_true")]
    pub interim_results: bool,
    /// Silence, in milliseconds, before Deepgram finalises a phrase.
    #[serde(default = "default_endpointing_ms")]
    pub endpointing_ms: u32,
    /// Silence, in milliseconds, before a turn boundary is reported.
    /// Deepgram accepts 1000..=5000.
    #[serde(default = "default_utterance_end_ms")]
    pub utterance_end_ms: u32,
}

fn default_deepgram_mode() -> String {
    "live".to_string()
}

/// The stored form of [`DeepgramConfig`], before the mode and the toggle are
/// reconciled.
#[derive(Deserialize)]
struct DeepgramConfigFields {
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_deepgram_mode")]
    mode: String,
    #[serde(default = "default_true")]
    live_typing: bool,
    #[serde(default = "default_deepgram_model")]
    model: String,
    #[serde(default = "default_deepgram_language")]
    language: String,
    #[serde(default = "default_true")]
    smart_format: bool,
    #[serde(default = "default_true")]
    punctuate: bool,
    #[serde(default)]
    dictation: bool,
    #[serde(default = "default_true")]
    interim_results: bool,
    #[serde(default = "default_endpointing_ms")]
    endpointing_ms: u32,
    #[serde(default = "default_utterance_end_ms")]
    utterance_end_ms: u32,
}

impl From<DeepgramConfigFields> for DeepgramConfig {
    fn from(fields: DeepgramConfigFields) -> Self {
        let mut config = Self {
            api_key: fields.api_key,
            mode: fields.mode,
            live_typing: fields.live_typing,
            model: fields.model,
            language: fields.language,
            smart_format: fields.smart_format,
            punctuate: fields.punctuate,
            dictation: fields.dictation,
            interim_results: fields.interim_results,
            endpointing_ms: fields.endpointing_ms,
            utterance_end_ms: fields.utterance_end_ms,
        };
        config.reconcile();
        config
    }
}

fn default_deepgram_model() -> String {
    "nova-3".to_string()
}

fn default_deepgram_language() -> String {
    "en".to_string()
}

fn default_true() -> bool {
    true
}

fn default_endpointing_ms() -> u32 {
    300
}

fn default_utterance_end_ms() -> u32 {
    1_000
}

impl DeepgramConfig {
    /// True when the user asked for the live-session path rather than the
    /// recorded upload.
    pub fn is_live(&self) -> bool {
        self.mode == "live"
    }

    /// The mode the user asked for.
    ///
    /// Live typing cannot run without a Live session, so asking for it moves
    /// the mode to live in the same step. One control instead of two, and the
    /// two settings can never be left disagreeing through the screen.
    pub fn set_live_typing(&mut self, enabled: bool) {
        self.live_typing = enabled;
        if enabled {
            self.mode = "live".to_string();
        }
    }

    /// The user switched the mode. Going back to recorded turns live typing
    /// off, because a recorded utterance has nothing to type from.
    pub fn set_mode(&mut self, mode: &str) {
        self.mode = mode.to_string();
        self.reconcile();
    }

    /// A recorded mode always means live typing off.
    fn reconcile(&mut self) {
        if !self.is_live() {
            self.live_typing = false;
        }
    }
}

impl Default for DeepgramConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            mode: default_deepgram_mode(),
            live_typing: true,
            model: default_deepgram_model(),
            language: default_deepgram_language(),
            smart_format: true,
            punctuate: true,
            dictation: false,
            interim_results: true,
            endpointing_ms: default_endpointing_ms(),
            utterance_end_ms: default_utterance_end_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// 0 = Copy to Clipboard, 1 = Type to Active Window, 2 = Copy & Type, 3 = Display Only
    pub mode: u8,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self { mode: 2 }
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
    #[serde(default)]
    pub deepgram: DeepgramConfig,
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
    #[allow(dead_code)]
    pub fn save_settings(&self) -> Result<()> {
        let guard = self.settings.lock().unwrap();
        Self::write_settings_to_disk(&guard)
    }

    // ── Validation ───────────────────────────────────────────────────────

    /// Returns `true` if `key` looks like a valid API key (OpenAI, Groq, or Gemini/Google).
    #[allow(dead_code)]
    pub fn validate_api_key(key: &str) -> bool {
        (key.starts_with("sk-") || key.starts_with("gsk_") || key.starts_with("AIza"))
            && key.len() > 20
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

    pub fn get_blocklist(&self) -> Vec<String> {
        self.settings.lock().unwrap().audio.blocklist.clone()
    }

    // ── Whisper getters ──────────────────────────────────────────────────

    pub fn get_whisper(&self) -> WhisperConfig {
        self.settings.lock().unwrap().whisper.clone()
    }

    pub fn get_whisper_mode(&self) -> String {
        self.settings.lock().unwrap().whisper.mode.clone()
    }

    // ── Deepgram getters ─────────────────────────────────────────────────

    pub fn get_deepgram(&self) -> DeepgramConfig {
        self.settings.lock().unwrap().deepgram.clone()
    }

    // ── Output getters ───────────────────────────────────────────────────

    pub fn get_output(&self) -> OutputConfig {
        self.settings.lock().unwrap().output.clone()
    }

    // ── UI getters / setters ─────────────────────────────────────────────

    #[allow(dead_code)]
    pub fn get_always_on_top(&self) -> bool {
        self.settings.lock().unwrap().ui.always_on_top
    }

    #[allow(dead_code)]
    pub fn get_custom_titlebar(&self) -> bool {
        self.settings.lock().unwrap().ui.custom_titlebar
    }

    #[allow(dead_code)]
    pub fn set_always_on_top(&self, on_top: bool) {
        self.settings.lock().unwrap().ui.always_on_top = on_top;
    }

    // ── Shortcuts getters ────────────────────────────────────────────────

    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn set_settings(&self, settings: Settings) {
        *self.settings.lock().unwrap() = settings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_typing::{live_typing_applies, live_typing_capability, LiveDelivery};

    /// Whether the screen shows a switch a user can turn on to make text
    /// appear: visible, operable, and drawn checked.
    fn shows_checked_switch(screen: crate::live_typing::LiveTypingScreen) -> bool {
        screen.visible && screen.enabled && screen.switch_checked
    }

    #[test]
    fn ui_config_has_correct_defaults() {
        let cfg = UiConfig::default();
        assert!(
            cfg.custom_titlebar,
            "custom_titlebar should default to true"
        );
        assert!(!cfg.always_on_top, "always_on_top should default to false");
        assert_eq!(cfg.theme, "white");
        assert_eq!(cfg.overlay_mode, "minimal");
    }

    #[test]
    fn ui_config_serde_roundtrip() {
        let original = UiConfig::default();
        let json = serde_json::to_string(&original).expect("serialize UiConfig");
        let parsed: UiConfig = serde_json::from_str(&json).expect("deserialize UiConfig");
        assert_eq!(parsed.custom_titlebar, original.custom_titlebar);
        assert_eq!(parsed.always_on_top, original.always_on_top);
        assert_eq!(parsed.theme, original.theme);
        assert_eq!(parsed.overlay_mode, original.overlay_mode);
    }

    #[test]
    fn settings_serde_preserves_ui_fields() {
        let mut settings = Settings::default();
        settings.ui.custom_titlebar = false;
        settings.ui.always_on_top = true;

        let json = serde_json::to_string_pretty(&settings).expect("serialize Settings");
        let parsed: Settings = serde_json::from_str(&json).expect("deserialize Settings");

        assert!(!parsed.ui.custom_titlebar);
        assert!(parsed.ui.always_on_top);
    }

    #[test]
    fn whisper_config_gemini_fields_serde_roundtrip() {
        let cfg = WhisperConfig {
            api_provider: "gemini".to_string(),
            gemini_api_key: "AIzaSyCustomKey".to_string(),
            ..WhisperConfig::default()
        };

        let json = serde_json::to_string(&cfg).expect("serialize WhisperConfig");
        let parsed: WhisperConfig = serde_json::from_str(&json).expect("deserialize WhisperConfig");

        assert_eq!(parsed.api_provider, "gemini");
        assert_eq!(parsed.gemini_api_key, "AIzaSyCustomKey");
    }

    #[test]
    fn deepgram_config_defaults_enable_the_normal_things() {
        let cfg = DeepgramConfig::default();
        assert_eq!(
            cfg.mode, "live",
            "live is the default now that text is typed while the user speaks"
        );
        assert!(cfg.is_live());
        assert!(cfg.live_typing, "live typing is on for a fresh install");
        assert_eq!(cfg.model, "nova-3");
        assert_eq!(cfg.language, "en");
        assert!(cfg.smart_format, "smart_format should default on");
        assert!(cfg.punctuate, "punctuate should default on");
        assert!(
            !cfg.dictation,
            "dictation must default off: it replaces automatic punctuation instead of adding to it"
        );
        assert!(cfg.interim_results, "interim results drive the word count");
        assert_eq!(cfg.endpointing_ms, 300);
        assert_eq!(cfg.utterance_end_ms, 1_000);
        assert!(cfg.api_key.is_empty());
    }

    #[test]
    fn settings_without_a_deepgram_block_still_load() {
        // A settings.json written before Deepgram existed must keep loading.
        let legacy = r#"{
            "audio": {"device_id": null, "sample_rate": 16000, "channels": 1,
                      "sounds_enabled": true, "blocklist": []},
            "whisper": {"mode": "api", "api_provider": "gemini", "api_key": "",
                        "api_model": "gemini-2.0-flash", "local_model": "base",
                        "local_model_path": "", "api_language": "en"},
            "output": {"mode": 2},
            "ui": {"theme": "white", "always_on_top": false,
                   "custom_titlebar": true, "overlay_mode": "minimal"},
            "shortcuts": {"record_toggle": "Meta+Shift+`"},
            "advanced": {"buffer_size": 1024, "noise_threshold": 0.01,
                         "silence_timeout": 2.0, "max_history_entries": 100}
        }"#;

        let parsed: Settings = serde_json::from_str(legacy).expect("legacy settings must load");
        assert_eq!(parsed.whisper.api_provider, "gemini");
        assert_eq!(parsed.deepgram, DeepgramConfig::default());
    }

    #[test]
    fn deepgram_config_serde_roundtrip() {
        let cfg = DeepgramConfig {
            api_key: "not-a-real-deepgram-key".to_string(),
            model: "nova-2".to_string(),
            language: "multi".to_string(),
            dictation: false,
            ..DeepgramConfig::default()
        };

        let json = serde_json::to_string(&cfg).expect("serialize DeepgramConfig");
        let parsed: DeepgramConfig =
            serde_json::from_str(&json).expect("deserialize DeepgramConfig");

        assert_eq!(parsed, cfg);
    }

    // ── Live typing: where it may run, and how the screen mirrors that ──────

    fn deepgram(provider: &str, mode: &str, output: u8) -> Settings {
        let mut settings = Settings::default();
        settings.whisper.mode = "api".to_string();
        settings.whisper.api_provider = provider.to_string();
        settings.deepgram.mode = mode.to_string();
        // Configured, so the cases below are about the setting under test.
        settings.deepgram.api_key = "not-a-real-deepgram-key".to_string();
        settings.output.mode = output;
        settings
    }

    #[test]
    fn a_fresh_install_types_as_it_speaks() {
        // A fresh install with Deepgram chosen, which is what the ticket's
        // "fresh install" means: the other providers have no Live session.
        let mut settings = Settings::default();
        settings.whisper.api_provider = "deepgram".to_string();
        settings.deepgram.api_key = "not-a-real-deepgram-key".to_string();

        assert_eq!(settings.deepgram.mode, "live");
        assert!(settings.deepgram.live_typing);
        let capability = live_typing_capability(&settings);
        assert!(capability.active);
        assert!(capability.control_enabled);
        assert_eq!(capability.reason, None);
    }

    #[test]
    fn no_key_disables_the_control_before_the_user_speaks() {
        // What a fresh install with Deepgram picked but not configured looks
        // like. The recording path refuses to start, so the switch must say so
        // rather than promise text that will never arrive.
        let mut settings = Settings::default();
        settings.whisper.api_provider = "deepgram".to_string();

        let capability = live_typing_capability(&settings);
        assert!(capability.control_available);
        assert!(!capability.control_enabled);
        assert!(!capability.active);
        assert!(capability
            .reason
            .expect("a missing key explains itself")
            .contains("API key"));

        let screen = capability.screen();
        assert!(!shows_checked_switch(screen));
        assert!(!live_typing_applies(&settings));
    }

    #[test]
    fn a_blank_key_counts_as_no_key() {
        let mut settings = deepgram("deepgram", "live", 2);
        settings.deepgram.api_key = "   ".to_string();

        assert!(!live_typing_capability(&settings).control_enabled);
    }

    #[test]
    fn a_pair_the_file_would_correct_still_reads_as_off_in_memory() {
        // No round trip: the toggle is set directly, leaving recorded mode
        // paired with it. The capability reconciles, so the screen cannot show
        // a checked switch while the recording path reads it as off.
        let mut settings = deepgram("deepgram", "recorded", 2);
        settings.deepgram.live_typing = true;

        let capability = live_typing_capability(&settings);
        assert!(!capability.active);
        assert!(!live_typing_applies(&settings));
        assert!(!shows_checked_switch(capability.screen()));
    }

    #[test]
    fn turning_live_typing_on_moves_the_mode_to_live() {
        let mut settings = deepgram("deepgram", "recorded", 2);
        settings.deepgram.live_typing = false;
        assert!(!live_typing_capability(&settings).active);

        settings.deepgram.set_live_typing(true);

        assert_eq!(settings.deepgram.mode, "live");
        assert!(live_typing_capability(&settings).active);
    }

    #[test]
    fn switching_the_mode_to_recorded_turns_live_typing_off() {
        // A settings file cannot carry the contradiction either.
        let json = serde_json::to_string(&Settings {
            deepgram: DeepgramConfig {
                mode: "recorded".to_string(),
                live_typing: true,
                ..DeepgramConfig::default()
            },
            ..Settings::default()
        })
        .unwrap();
        let parsed: Settings = serde_json::from_str(&json).unwrap();

        assert!(!parsed.deepgram.live_typing);
        assert!(!live_typing_capability(&parsed).active);
    }

    #[test]
    fn a_batch_provider_has_no_control_and_never_acts() {
        for provider in ["openai", "groq", "gemini"] {
            let capability = live_typing_capability(&deepgram(provider, "live", 2));
            assert!(!capability.active, "{provider} has no live session");
            assert!(!capability.control_available, "{provider} shows no switch");

            let mut local = deepgram(provider, "live", 2);
            local.whisper.mode = "local".to_string();
            let capability = live_typing_capability(&local);
            assert!(!capability.active);
            assert!(!capability.control_available);
        }
    }

    #[test]
    fn the_control_only_exists_inside_the_deepgram_group() {
        let capability = live_typing_capability(&deepgram("groq", "live", 2));

        assert!(!capability.control_available);
        assert!(!capability.active);
    }

    #[test]
    fn a_stored_recorded_mode_turns_a_recorded_utterance_it_off_too() {
        let mut settings = deepgram("deepgram", "live", 2);
        settings.deepgram.set_mode("recorded");

        assert!(!settings.deepgram.live_typing);
        assert!(!live_typing_capability(&settings).active);
    }

    #[test]
    fn display_only_disables_the_control_and_states_why() {
        let settings = deepgram("deepgram", "live", 3);
        let capability = live_typing_capability(&settings);

        assert!(!capability.active);
        assert!(capability.control_available);
        assert!(!capability.control_enabled);
        assert!(capability
            .reason
            .expect("display only explains itself")
            .contains("Display only"));
    }

    #[test]
    fn copying_without_typing_keeps_the_clipboard_current_instead() {
        let settings = deepgram("deepgram", "live", 0);
        let capability = live_typing_capability(&settings);

        assert!(capability.active);
        assert_eq!(capability.delivery, LiveDelivery::CopyToClipboard);
    }

    #[test]
    fn typing_modes_type_and_copy_and_type_still_copies_at_stop() {
        for output in [1, 2] {
            let capability = live_typing_capability(&deepgram("deepgram", "live", output));
            assert_eq!(capability.delivery, LiveDelivery::Type);
            assert!(capability.active);
        }
    }

    #[test]
    fn the_screen_and_the_behaviour_read_the_same_answer() {
        // Every combination a user can reach: provider, mode, output, toggle.
        for provider in ["openai", "groq", "gemini", "deepgram"] {
            for mode in ["recorded", "live"] {
                for output in [0u8, 1, 2, 3] {
                    for toggle in [false, true] {
                        let mut settings = deepgram(provider, mode, output);
                        settings.deepgram.live_typing = toggle;
                        settings = stored(settings);

                        let capability = live_typing_capability(&settings);
                        let screen = capability.screen();
                        let ran = live_typing_applies(&settings);
                        let case = format!("{provider}/{mode}/output {output}/toggle {toggle}");

                        // The stored state is never contradictory.
                        assert!(
                            !settings.deepgram.live_typing || settings.deepgram.is_live(),
                            "{case}: live typing cannot be on in recorded mode"
                        );
                        // No switch is drawn for a provider without a Live
                        // session, and one that is drawn is checked exactly when
                        // it acts, never for a stored toggle that cannot run.
                        assert_eq!(screen.visible, provider == "deepgram", "{case}: visible");
                        assert_eq!(
                            screen.switch_checked,
                            screen.visible && capability.active,
                            "{case}: checked"
                        );
                        if !capability.control_available {
                            assert!(!screen.switch_checked, "{case}: no switch to check");
                        }
                        // A switch that looks on is a switch that acts, and the
                        // reason a disabled one gives is what the user sees.
                        assert_eq!(
                            shows_checked_switch(screen),
                            capability.active,
                            "{case}: the screen and the capability must agree"
                        );
                        assert_eq!(
                            capability.active, ran,
                            "{case}: the derived answer is what runs"
                        );
                        assert_eq!(
                            capability.control_enabled,
                            capability.reason.is_none(),
                            "{case}: a control states why it is disabled"
                        );
                    }
                }
            }
        }
    }

    /// Settings load the way the app leaves them: the mode and the toggle
    /// cannot contradict each other.
    fn stored(settings: Settings) -> Settings {
        let json = serde_json::to_string(&settings).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn full_settings_default_serde_roundtrip() {
        let original = Settings::default();
        let json = serde_json::to_string_pretty(&original).expect("serialize Settings");
        let parsed: Settings = serde_json::from_str(&json).expect("deserialize Settings");

        assert_eq!(parsed.ui.custom_titlebar, original.ui.custom_titlebar);
        assert_eq!(parsed.ui.always_on_top, original.ui.always_on_top);
        assert_eq!(parsed.ui.theme, original.ui.theme);
        assert_eq!(parsed.ui.overlay_mode, original.ui.overlay_mode);
        assert_eq!(
            parsed.advanced.max_history_entries,
            original.advanced.max_history_entries
        );
    }
}
