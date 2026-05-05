use chrono::{DateTime, Local};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

// ── Data structures ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub duration_secs: f64,
    pub transcription_time_secs: f64,
    pub mode: String,
    pub success: bool,
    pub text_length: usize,
    pub confidence: f64,
    #[serde(default)]
    pub text: String,
}

// ── Statistics Manager ───────────────────────────────────────────────────────

pub struct StatisticsManager {
    history_path: PathBuf,
    history: Mutex<Vec<HistoryEntry>>,
    max_history_entries: Mutex<usize>,
}

const DEFAULT_MAX_HISTORY_ENTRIES: usize = 100;

impl StatisticsManager {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("quillscribe");

        let history_path = config_dir.join("history.json");
        let history = Self::load_history_from_file(&history_path);

        info!(
            "Statistics manager initialized. {} history entries",
            history.len()
        );

        StatisticsManager {
            history_path,
            history: Mutex::new(history),
            max_history_entries: Mutex::new(DEFAULT_MAX_HISTORY_ENTRIES),
        }
    }

    fn load_history_from_file(path: &PathBuf) -> Vec<HistoryEntry> {
        if !path.exists() {
            return Vec::new();
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read history file: {}", e);
                return Vec::new();
            }
        };
        match serde_json::from_str(&content) {
            Ok(history) => history,
            Err(e) => {
                warn!(
                    "Failed to parse history file, backing up and resetting: {}",
                    e
                );
                let backup = path.with_extension("json.bak");
                let _ = fs::copy(path, &backup);
                Vec::new()
            }
        }
    }

    pub fn save_history(&self) {
        if let Ok(history) = self.history.lock() {
            if let Some(parent) = self.history_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match serde_json::to_string_pretty(&*history) {
                Ok(json) => {
                    if let Err(e) = fs::write(&self.history_path, json) {
                        error!("Failed to save history: {}", e);
                    }
                }
                Err(e) => error!("Failed to serialize history: {}", e),
            }
        }
    }

    pub fn record_transcription_result(
        &self,
        success: bool,
        mode: &str,
        duration_secs: f64,
        transcription_time_secs: f64,
        text: &str,
        confidence: f64,
    ) {
        let entry = HistoryEntry {
            timestamp: Local::now().to_rfc3339(),
            duration_secs,
            transcription_time_secs,
            mode: mode.to_string(),
            success,
            text_length: text.len(),
            confidence,
            text: text.to_string(),
        };

        let max = self
            .max_history_entries
            .lock()
            .map(|m| *m)
            .unwrap_or(DEFAULT_MAX_HISTORY_ENTRIES);

        if let Ok(mut history) = self.history.lock() {
            history.push(entry);
            if history.len() > max {
                let excess = history.len() - max;
                history.drain(0..excess);
            }
        }

        self.save_history();
        debug!(
            "Recorded transcription result: success={}, mode={}, duration={:.1}s",
            success, mode, duration_secs
        );
    }

    pub fn get_recent_history(&self, days: i64) -> Vec<HistoryEntry> {
        let cutoff = Local::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();

        self.history
            .lock()
            .map(|h| {
                h.iter()
                    .filter(|entry| {
                        DateTime::parse_from_rfc3339(&entry.timestamp)
                            .map(|timestamp| timestamp.with_timezone(&Local) >= cutoff)
                            .unwrap_or_else(|_| entry.timestamp >= cutoff_str)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Updates the maximum number of history entries to keep.
    /// If the current history exceeds the new limit, the oldest entries are trimmed.
    pub fn set_max_history_entries(&self, max: usize) {
        let max = max.max(1);
        if let Ok(mut m) = self.max_history_entries.lock() {
            *m = max;
        }
        if let Ok(mut history) = self.history.lock() {
            if history.len() > max {
                let excess = history.len() - max;
                history.drain(0..excess);
            }
        }
        self.save_history();
        info!("Max history entries set to {}", max);
    }
}
