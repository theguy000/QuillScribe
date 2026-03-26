use anyhow::{Context, Result};
use chrono::Local;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

// ── Data structures ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllTimeStats {
    pub total_sessions: u64,
    pub total_recordings: u64,
    pub total_duration_secs: f64,
    pub total_transcription_time_secs: f64,
    pub total_characters: u64,
    pub total_words: u64,
    pub successful_transcriptions: u64,
    pub failed_transcriptions: u64,
    pub api_transcriptions: u64,
    pub local_transcriptions: u64,
    pub avg_transcription_time_secs: f64,
    pub fastest_transcription_secs: f64,
    pub slowest_transcription_secs: f64,
    pub avg_audio_duration_secs: f64,
    pub daily_usage: HashMap<String, u64>,
}

impl Default for AllTimeStats {
    fn default() -> Self {
        AllTimeStats {
            total_sessions: 0,
            total_recordings: 0,
            total_duration_secs: 0.0,
            total_transcription_time_secs: 0.0,
            total_characters: 0,
            total_words: 0,
            successful_transcriptions: 0,
            failed_transcriptions: 0,
            api_transcriptions: 0,
            local_transcriptions: 0,
            avg_transcription_time_secs: 0.0,
            fastest_transcription_secs: f64::MAX,
            slowest_transcription_secs: 0.0,
            avg_audio_duration_secs: 0.0,
            daily_usage: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub recordings: u64,
    pub duration_secs: f64,
    pub transcription_time_secs: f64,
    pub characters: u64,
    pub words: u64,
    pub successful: u64,
    pub failed: u64,
    pub start_time: Option<String>,
}

impl Default for SessionStats {
    fn default() -> Self {
        SessionStats {
            recordings: 0,
            duration_secs: 0.0,
            transcription_time_secs: 0.0,
            characters: 0,
            words: 0,
            successful: 0,
            failed: 0,
            start_time: None,
        }
    }
}

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
    stats_path: PathBuf,
    history_path: PathBuf,
    all_time: Mutex<AllTimeStats>,
    session: Mutex<SessionStats>,
    history: Mutex<Vec<HistoryEntry>>,
    session_active: Mutex<bool>,
}

const MAX_HISTORY_ENTRIES: usize = 1000;

impl StatisticsManager {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("quillscribe");

        let stats_path = config_dir.join("statistics.json");
        let history_path = config_dir.join("history.json");

        let all_time = Self::load_stats_from_file(&stats_path);
        let history = Self::load_history_from_file(&history_path);

        info!(
            "Statistics manager initialized. {} all-time recordings, {} history entries",
            all_time.total_recordings,
            history.len()
        );

        StatisticsManager {
            stats_path,
            history_path,
            all_time: Mutex::new(all_time),
            session: Mutex::new(SessionStats::default()),
            history: Mutex::new(history),
            session_active: Mutex::new(false),
        }
    }

    fn load_stats_from_file(path: &PathBuf) -> AllTimeStats {
        if !path.exists() {
            return AllTimeStats::default();
        }
        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(stats) => stats,
                Err(e) => {
                    warn!(
                        "Failed to parse statistics file, backing up and resetting: {}",
                        e
                    );
                    let backup = path.with_extension("json.bak");
                    let _ = fs::copy(path, &backup);
                    AllTimeStats::default()
                }
            },
            Err(e) => {
                warn!("Failed to read statistics file: {}", e);
                AllTimeStats::default()
            }
        }
    }

    fn load_history_from_file(path: &PathBuf) -> Vec<HistoryEntry> {
        if !path.exists() {
            return Vec::new();
        }
        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
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
            },
            Err(e) => {
                warn!("Failed to read history file: {}", e);
                Vec::new()
            }
        }
    }

    pub fn save_statistics(&self) {
        if let Ok(stats) = self.all_time.lock() {
            if let Some(parent) = self.stats_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match serde_json::to_string_pretty(&*stats) {
                Ok(json) => {
                    if let Err(e) = fs::write(&self.stats_path, json) {
                        error!("Failed to save statistics: {}", e);
                    }
                }
                Err(e) => error!("Failed to serialize statistics: {}", e),
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

    pub fn record_session_start(&self) {
        if let Ok(mut session) = self.session.lock() {
            *session = SessionStats::default();
            session.start_time = Some(Local::now().to_rfc3339());
        }
        if let Ok(mut active) = self.session_active.lock() {
            *active = true;
        }
        if let Ok(mut stats) = self.all_time.lock() {
            stats.total_sessions += 1;
        }
        self.save_statistics();
        debug!("Session started");
    }

    pub fn record_recording_start(&self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        if let Ok(mut stats) = self.all_time.lock() {
            stats.total_recordings += 1;
            *stats.daily_usage.entry(today).or_insert(0) += 1;
        }
        if let Ok(mut session) = self.session.lock() {
            session.recordings += 1;
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
        let word_count = text.split_whitespace().count() as u64;
        let char_count = text.len() as u64;

        // Update all-time stats
        if let Ok(mut stats) = self.all_time.lock() {
            stats.total_duration_secs += duration_secs;
            stats.total_transcription_time_secs += transcription_time_secs;

            if success {
                stats.successful_transcriptions += 1;
                stats.total_characters += char_count;
                stats.total_words += word_count;

                match mode {
                    "api" => stats.api_transcriptions += 1,
                    "local" => stats.local_transcriptions += 1,
                    _ => {}
                }

                if transcription_time_secs < stats.fastest_transcription_secs {
                    stats.fastest_transcription_secs = transcription_time_secs;
                }
                if transcription_time_secs > stats.slowest_transcription_secs {
                    stats.slowest_transcription_secs = transcription_time_secs;
                }

                let total_successful = stats.successful_transcriptions as f64;
                stats.avg_transcription_time_secs =
                    stats.total_transcription_time_secs / total_successful;
                stats.avg_audio_duration_secs = stats.total_duration_secs / total_successful;
            } else {
                stats.failed_transcriptions += 1;
            }
        }

        // Update session stats
        if let Ok(mut session) = self.session.lock() {
            session.duration_secs += duration_secs;
            session.transcription_time_secs += transcription_time_secs;
            if success {
                session.successful += 1;
                session.characters += char_count;
                session.words += word_count;
            } else {
                session.failed += 1;
            }
        }

        // Add to history
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

        if let Ok(mut history) = self.history.lock() {
            history.push(entry);
            // Cap at MAX_HISTORY_ENTRIES
            if history.len() > MAX_HISTORY_ENTRIES {
                let excess = history.len() - MAX_HISTORY_ENTRIES;
                history.drain(0..excess);
            }
        }

        self.save_statistics();
        self.save_history();
        debug!(
            "Recorded transcription result: success={}, mode={}, duration={:.1}s",
            success, mode, duration_secs
        );
    }

    pub fn record_session_end(&self) {
        if let Ok(mut active) = self.session_active.lock() {
            if !*active {
                return; // Already ended — idempotent
            }
            *active = false;
        }
        self.save_statistics();
        self.save_history();
        debug!("Session ended");
    }

    pub fn get_statistics(&self) -> AllTimeStats {
        self.all_time.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn get_session_statistics(&self) -> SessionStats {
        self.session.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn get_recent_history(&self, days: i64) -> Vec<HistoryEntry> {
        let cutoff = Local::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();

        self.history
            .lock()
            .map(|h| {
                h.iter()
                    .filter(|entry| entry.timestamp >= cutoff_str)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_accuracy_rate(&self) -> f64 {
        let stats = self.get_statistics();
        let total = stats.successful_transcriptions + stats.failed_transcriptions;
        if total == 0 {
            return 0.0;
        }
        (stats.successful_transcriptions as f64 / total as f64) * 100.0
    }

    pub fn get_daily_usage_trend(&self, days: i64) -> HashMap<String, u64> {
        let stats = self.get_statistics();
        let mut trend = HashMap::new();
        let today = Local::now().date_naive();

        for i in 0..days {
            let date = today - chrono::Duration::days(i);
            let key = date.format("%Y-%m-%d").to_string();
            let count = stats.daily_usage.get(&key).copied().unwrap_or(0);
            trend.insert(key, count);
        }

        trend
    }

    pub fn reset_statistics(&self) {
        if let Ok(mut stats) = self.all_time.lock() {
            *stats = AllTimeStats::default();
        }
        if let Ok(mut history) = self.history.lock() {
            history.clear();
        }
        self.save_statistics();
        self.save_history();
        info!("Statistics reset");
    }

    pub fn export_statistics(&self, file_path: &str) -> Result<()> {
        let export = serde_json::json!({
            "all_time": self.get_statistics(),
            "session": self.get_session_statistics(),
            "history": self.history.lock().map(|h| h.clone()).unwrap_or_default(),
            "exported_at": Local::now().to_rfc3339(),
        });

        let json =
            serde_json::to_string_pretty(&export).context("Failed to serialize export data")?;
        fs::write(file_path, json).context("Failed to write export file")?;

        info!("Statistics exported to {}", file_path);
        Ok(())
    }
}
