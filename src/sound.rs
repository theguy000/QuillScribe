use log::{debug, error, info, warn};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::sync::Mutex;

/// Embedded WAV files for notification sounds.
/// We embed them at compile time so they're always available.
const START_WAV: &[u8] = include_bytes!("../sounds/start.wav");
const STOP_WAV: &[u8] = include_bytes!("../sounds/stop.wav");

/// Holds the lazily-initialized audio output stream.
struct StreamState {
    initialized: bool,
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
}

pub struct SoundManager {
    enabled: Mutex<bool>,
    // Lazily initialized on first sound playback to avoid blocking startup.
    stream: Mutex<StreamState>,
}

impl SoundManager {
    pub fn new() -> Self {
        SoundManager {
            enabled: Mutex::new(true),
            stream: Mutex::new(StreamState {
                initialized: false,
                _stream: None,
                handle: None,
            }),
        }
    }

    /// Ensures the audio output stream is initialized. Called lazily on first
    /// sound playback so that startup is not blocked by WASAPI initialization.
    fn ensure_stream(&self) -> bool {
        let mut state = match self.stream.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };

        if state.initialized {
            return state.handle.is_some();
        }

        state.initialized = true;

        match OutputStream::try_default() {
            Ok((s, h)) => {
                info!("Audio output stream initialized for notification sounds");
                state._stream = Some(s);
                state.handle = Some(h);
                true
            }
            Err(e) => {
                warn!(
                    "Failed to initialize audio output for sounds: {}. Sounds will be disabled.",
                    e
                );
                false
            }
        }
    }

    pub fn set_sounds_enabled(&self, enabled: bool) {
        if let Ok(mut e) = self.enabled.lock() {
            *e = enabled;
            debug!(
                "Notification sounds {}",
                if enabled { "enabled" } else { "disabled" }
            );
        }
    }

    pub fn is_sounds_enabled(&self) -> bool {
        self.enabled.lock().map(|e| *e).unwrap_or(false)
    }

    pub fn play_start_sound(&self) {
        self.play_sound(START_WAV, "start");
    }

    pub fn play_stop_sound(&self) {
        self.play_sound(STOP_WAV, "stop");
    }

    fn play_sound(&self, wav_data: &'static [u8], name: &str) {
        if !self.is_sounds_enabled() {
            debug!("Sounds disabled, skipping {} sound", name);
            return;
        }

        if !self.ensure_stream() {
            warn!(
                "No audio output stream available, cannot play {} sound",
                name
            );
            return;
        }

        let state = match self.stream.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        let handle = match &state.handle {
            Some(h) => h,
            None => return,
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(e) => {
                error!("Failed to create audio sink for {} sound: {}", name, e);
                return;
            }
        };

        let cursor = Cursor::new(wav_data);
        match Decoder::new(cursor) {
            Ok(source) => {
                sink.append(source);
                // Detach the sink so it plays to completion without blocking
                sink.detach();
                debug!("Playing {} notification sound", name);
            }
            Err(e) => {
                error!("Failed to decode {} WAV data: {}", name, e);
            }
        }
    }
}

// SoundManager contains OutputStream which is not Send.
// We need to ensure it's only accessed from the thread that created it,
// but for Tauri State we need Send+Sync. Since we wrap internals in Mutex
// and the OutputStream is never moved between threads, this is safe.
unsafe impl Send for SoundManager {}
unsafe impl Sync for SoundManager {}
