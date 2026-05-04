use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub channels: u16,
    pub sample_rate: u32,
}

// ── Shared audio state (accessed from both main thread and audio callback) ──

struct AudioState {
    audio_level: f32,
    is_recording: bool,
    is_monitoring: bool,
    record_buffer: Vec<f32>,
    /// The actual sample rate of the active audio stream (set when stream starts).
    device_sample_rate: u32,
    /// Whether monitoring was active before the current recording started.
    /// Used to decide whether to stop the stream when recording ends.
    was_monitoring_before_recording: bool,
}

impl AudioState {
    fn new() -> Self {
        Self {
            audio_level: 0.0,
            is_recording: false,
            is_monitoring: false,
            record_buffer: Vec::new(),
            device_sample_rate: 16_000, // default, updated when stream starts
            was_monitoring_before_recording: false,
        }
    }
}

// ── Commands sent to the dedicated audio thread ─────────────────────────────

#[allow(dead_code)]
enum AudioCommand {
    GetDevices {
        blocklist: Vec<String>,
        reply: mpsc::Sender<Vec<AudioDevice>>,
    },
    StartMonitoring {
        device_id: Option<String>,
        reply: Option<mpsc::Sender<Result<()>>>,
    },
    StopMonitoring,
    StartRecording,
    StopRecording,
    TestMicrophone {
        device_id: Option<String>,
        reply: mpsc::Sender<Result<bool>>,
    },
    SetDevice {
        device_id: Option<String>,
        reply: mpsc::Sender<Result<()>>,
    },
    Shutdown,
}

// ── AudioManager ────────────────────────────────────────────────────────────
//
// This struct is Send + Sync because it only holds:
//   - Arc<Mutex<AudioState>> (Send + Sync)
//   - mpsc::Sender<AudioCommand> (Send, and we wrap in Mutex for Sync)
//   - Option<String> for the selected device
//
// The cpal Host and Stream live entirely inside the dedicated audio thread.

pub struct AudioManager {
    state: Arc<Mutex<AudioState>>,
    selected_device_id: Option<String>,
    command_tx: Mutex<mpsc::Sender<AudioCommand>>,
    _audio_thread: Option<thread::JoinHandle<()>>,
}

// mpsc::Sender is Send but not Sync; we wrap it in Mutex so the overall
// struct is Sync. The JoinHandle is Send. Everything else is trivially
// Send + Sync, so we can assert the bounds the compiler needs.
//
// Safety: All non-Send/Sync fields are behind Mutex or are never accessed
// concurrently.
unsafe impl Sync for AudioManager {}

impl AudioManager {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(AudioState::new()));
        let (tx, rx) = mpsc::channel::<AudioCommand>();

        let thread_state = Arc::clone(&state);
        let handle = thread::spawn(move || {
            audio_thread_fn(rx, thread_state);
        });

        Self {
            state,
            selected_device_id: None,
            command_tx: Mutex::new(tx),
            _audio_thread: Some(handle),
        }
    }

    pub fn get_available_devices(&self, blocklist: Vec<String>) -> Vec<AudioDevice> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let tx = self.command_tx.lock().unwrap();
        if tx
            .send(AudioCommand::GetDevices {
                blocklist,
                reply: reply_tx,
            })
            .is_err()
        {
            return Vec::new();
        }
        drop(tx);
        reply_rx.recv().unwrap_or_default()
    }

    /// Select a specific input device by id, or use the default if `None`.
    pub fn set_input_device(&mut self, device_id: Option<String>) -> Result<()> {
        if device_id.is_some() {
            // Validate that the device exists by asking the audio thread.
            let (reply_tx, reply_rx) = mpsc::channel();
            let tx = self.command_tx.lock().unwrap();
            tx.send(AudioCommand::SetDevice {
                device_id: device_id.clone(),
                reply: reply_tx,
            })
            .map_err(|_| anyhow!("Audio thread is not running"))?;
            drop(tx);
            reply_rx
                .recv()
                .map_err(|_| anyhow!("Audio thread did not respond"))??;
        }
        self.selected_device_id = device_id;
        Ok(())
    }

    /// Start audio monitoring. The audio level will be updated continuously
    /// and can be retrieved via `get_audio_level()`.
    /// If `wait_for_reply` is true, blocks until the audio thread confirms
    /// the stream started (or returns an error).
    pub fn start_monitoring(&mut self, wait_for_reply: bool) -> Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            if state.is_monitoring {
                return Ok(());
            }
            state.is_monitoring = true;
            state.audio_level = 0.0;
        }

        let reply = if wait_for_reply {
            let (reply_tx, reply_rx) = mpsc::channel();
            Some((reply_tx, reply_rx))
        } else {
            None
        };

        let tx = self.command_tx.lock().unwrap();
        tx.send(AudioCommand::StartMonitoring {
            device_id: self.selected_device_id.clone(),
            reply: reply.as_ref().map(|(tx, _)| tx.clone()),
        })
        .map_err(|_| anyhow!("Audio thread is not running"))?;
        drop(tx);

        if let Some((_, reply_rx)) = reply {
            reply_rx
                .recv()
                .map_err(|_| anyhow!("Audio thread did not respond"))??;
        }

        Ok(())
    }

    /// Stop audio monitoring.
    #[allow(dead_code)]
    pub fn stop_monitoring(&mut self) {
        {
            let mut state = self.state.lock().unwrap();
            state.is_monitoring = false;
            state.audio_level = 0.0;
        }

        let tx = self.command_tx.lock().unwrap();
        let _ = tx.send(AudioCommand::StopMonitoring);
    }

    /// Start recording audio data into an internal buffer.
    /// If monitoring is active, the stream is reused and recording is layered on top.
    pub fn start_recording(&mut self) -> Result<()> {
        let need_stream = {
            let mut state = self.state.lock().unwrap();
            if state.is_recording {
                return Ok(());
            }
            let was_monitoring = state.is_monitoring;
            state.was_monitoring_before_recording = was_monitoring;
            state.is_recording = true;
            state.is_monitoring = true;
            state.record_buffer.clear();
            !was_monitoring
        };

        let tx = self.command_tx.lock().unwrap();

        // Tell the audio thread to set the recording flag.
        tx.send(AudioCommand::StartRecording)
            .map_err(|_| anyhow!("Audio thread is not running"))?;

        // If there was no monitoring stream, start one.
        if need_stream {
            tx.send(AudioCommand::StartMonitoring {
                device_id: self.selected_device_id.clone(),
                reply: None,
            })
            .map_err(|_| anyhow!("Audio thread is not running"))?;
        }

        Ok(())
    }

    /// Stop recording and return the captured audio buffer (mono, f32 samples)
    /// together with the actual device sample rate.
    /// Returns `None` if no recording was in progress.
    pub fn stop_recording(&mut self) -> Result<Option<(Vec<f32>, u32)>> {
        let (buffer, device_sr, was_monitoring_before) = {
            let mut state = self.state.lock().unwrap();
            if !state.is_recording {
                return Ok(None);
            }
            state.is_recording = false;
            let was_mon = state.was_monitoring_before_recording;
            // If the user wasn't monitoring before recording, stop monitoring too.
            if !was_mon {
                state.is_monitoring = false;
            }
            let buf = std::mem::take(&mut state.record_buffer);
            let sr = state.device_sample_rate;
            (buf, sr, was_mon)
        };

        // Stop the stream if we weren't monitoring before recording started.
        if !was_monitoring_before {
            let tx = self.command_tx.lock().unwrap();
            let _ = tx.send(AudioCommand::StopRecording);
        }

        if buffer.is_empty() {
            Ok(None)
        } else {
            Ok(Some((buffer, device_sr)))
        }
    }

    /// Get the current normalized audio level (0.0 - 1.0).
    pub fn get_audio_level(&self) -> f32 {
        self.state.lock().unwrap().audio_level
    }

    /// Test whether a microphone is working by briefly opening a stream and
    /// checking if any non-silent audio is captured.
    #[allow(dead_code)]
    pub fn test_microphone(&self, device_id: Option<String>) -> Result<bool> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let tx = self.command_tx.lock().unwrap();
        tx.send(AudioCommand::TestMicrophone {
            device_id,
            reply: reply_tx,
        })
        .map_err(|_| anyhow!("Audio thread is not running"))?;
        drop(tx);
        reply_rx
            .recv()
            .map_err(|_| anyhow!("Audio thread did not respond"))?
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        let tx = self.command_tx.lock().unwrap();
        let _ = tx.send(AudioCommand::Shutdown);
        drop(tx);

        if let Some(handle) = self._audio_thread.take() {
            let _ = handle.join();
        }
    }
}

// ── Dedicated audio thread ──────────────────────────────────────────────────
//
// All cpal types (Host, Device, Stream) live exclusively on this thread,
// avoiding any Send/Sync issues.

#[cfg(target_os = "linux")]
fn suppress_stderr_during<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    unsafe {
        let dev_null = libc::open(
            b"/dev/null\0".as_ptr() as *const libc::c_char,
            libc::O_WRONLY,
        );
        if dev_null < 0 {
            return f();
        }
        let old_stderr = libc::dup(2);
        libc::dup2(dev_null, 2);
        libc::close(dev_null);
        let result = f();
        libc::dup2(old_stderr, 2);
        libc::close(old_stderr);
        result
    }
}

#[cfg(not(target_os = "linux"))]
fn suppress_stderr_during<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

#[allow(unused_assignments, unused_variables)]
fn audio_thread_fn(rx: mpsc::Receiver<AudioCommand>, state: Arc<Mutex<AudioState>>) {
    let host = suppress_stderr_during(cpal::default_host);
    // The active stream is kept alive here; dropping it stops audio capture.
    // The compiler warns about "unused assignments" because the reads are
    // implicit (keeping the stream alive / dropping it), so we suppress that.
    let mut active_stream: Option<cpal::Stream> = None;

    loop {
        let cmd = match rx.recv() {
            Ok(cmd) => cmd,
            Err(_) => break, // Channel closed, exit thread.
        };

        match cmd {
            AudioCommand::GetDevices { blocklist, reply } => {
                let devices = suppress_stderr_during(|| enumerate_devices(&host, &blocklist));
                let _ = reply.send(devices);
            }

            AudioCommand::SetDevice { device_id, reply } => {
                let result = if let Some(ref id) = device_id {
                    match suppress_stderr_during(|| find_device_by_id(&host, id)) {
                        Some(_) => Ok(()),
                        None => Err(anyhow!("Audio device not found: {}", id)),
                    }
                } else {
                    Ok(())
                };
                let _ = reply.send(result);
            }

            AudioCommand::StartMonitoring { device_id, reply } => {
                // Drop any existing stream first.
                active_stream = None;

                let result = (|| -> Result<()> {
                    let device = resolve_device(&host, device_id.as_deref())?;

                    // Store the actual device sample rate so the recorded audio
                    // is later encoded / resampled with the correct rate.
                    if let Ok(supported) = device.default_input_config() {
                        let mut s = state.lock().unwrap();
                        s.device_sample_rate = supported.sample_rate().0;
                    }

                    let stream = build_input_stream(&device, Arc::clone(&state))?;
                    stream.play().map_err(|e| anyhow!("Failed to start audio stream: {}", e))?;
                    active_stream = Some(stream);
                    Ok(())
                })();

                if let Err(e) = &result {
                    eprintln!("Failed to start monitoring: {}", e);
                    let mut s = state.lock().unwrap();
                    s.is_monitoring = false;
                }

                if let Some(reply_tx) = reply {
                    let _ = reply_tx.send(result);
                }
            }

            AudioCommand::StopMonitoring => {
                active_stream = None;
            }

            AudioCommand::StartRecording => {
                // Recording flag is already set in AudioState by the caller.
                // Nothing extra to do on the audio thread side.
            }

            AudioCommand::StopRecording => {
                // Stop the stream (the caller already cleared the recording flag).
                active_stream = None;
            }

            AudioCommand::TestMicrophone { device_id, reply } => {
                let result = run_microphone_test(&host, device_id.as_deref());
                let _ = reply.send(result);
            }

            AudioCommand::Shutdown => {
                active_stream = None;
                break;
            }
        }
    }
}

// ── Helper functions (all run on the audio thread) ──────────────────────────

fn enumerate_devices(host: &cpal::Host, blocklist: &[String]) -> Vec<AudioDevice> {
    let devices = match host.input_devices() {
        Ok(devs) => devs,
        Err(_) => return Vec::new(),
    };

    // First pass: collect devices with display names
    let mut candidates: Vec<AudioDevice> = devices
        .filter_map(|device| {
            let name = device.name().ok()?;

            if blocklist.iter().any(|pattern| name.contains(pattern)) {
                return None;
            }

            let mut display_name = name.clone();
            #[cfg(target_os = "linux")]
            {
                if name.starts_with("sysdefault:CARD=") {
                    display_name = name.replace("sysdefault:CARD=", "");
                } else if name == "pulse" {
                    display_name = "PulseAudio".to_string();
                } else if name == "pipewire" {
                    display_name = "PipeWire".to_string();
                } else if name == "default" {
                    display_name = "System Default".to_string();
                }
            }

            let config = device.default_input_config().ok()?;
            Some(AudioDevice {
                id: name,
                name: display_name,
                channels: config.channels(),
                sample_rate: config.sample_rate().0,
            })
        })
        .collect();

    // Disambiguate duplicate display names by appending the raw ID
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for dev in &candidates {
        *name_counts.entry(dev.name.clone()).or_default() += 1;
    }
    for dev in &mut candidates {
        if name_counts[&dev.name] > 1 {
            dev.name = format!("{} [{}]", dev.name, dev.id);
        }
    }

    candidates
}

fn find_device_by_id(host: &cpal::Host, id: &str) -> Option<cpal::Device> {
    host.input_devices()
        .ok()?
        .find(|d| d.name().map(|n| n == id).unwrap_or(false))
}

fn resolve_device(host: &cpal::Host, device_id: Option<&str>) -> Result<cpal::Device> {
    if let Some(id) = device_id {
        find_device_by_id(host, id).ok_or_else(|| anyhow!("Audio device not found: {}", id))
    } else {
        host.default_input_device()
            .ok_or_else(|| anyhow!("No default input device available"))
    }
}

fn build_input_stream(
    device: &cpal::Device,
    state: Arc<Mutex<AudioState>>,
) -> Result<cpal::Stream> {
    let supported_config = device
        .default_input_config()
        .map_err(|e| anyhow!("Failed to get default input config: {}", e))?;

    let sample_format = supported_config.sample_format();
    let channels = supported_config.channels();
    let config: StreamConfig = supported_config.into();

    let err_state = Arc::clone(&state);
    let err_fn = move |err: cpal::StreamError| {
        eprintln!("Audio stream error: {}", err);
        // Mark monitoring/recording as stopped so the next attempt starts a fresh stream.
        let mut s = err_state.lock().unwrap();
        s.is_monitoring = false;
        s.is_recording = false;
    };

    let stream = match sample_format {
        SampleFormat::F32 => {
            let state = Arc::clone(&state);
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    process_audio_data(data, channels, &state);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let state = Arc::clone(&state);
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let float_data: Vec<f32> =
                        data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    process_audio_data(&float_data, channels, &state);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let state = Arc::clone(&state);
            device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let float_data: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    process_audio_data(&float_data, channels, &state);
                },
                err_fn,
                None,
            )
        }
        _ => return Err(anyhow!("Unsupported sample format: {:?}", sample_format)),
    }
    .map_err(|e| anyhow!("Failed to build audio stream: {}", e))?;

    Ok(stream)
}

fn run_microphone_test(host: &cpal::Host, device_id: Option<&str>) -> Result<bool> {
    let device = resolve_device(host, device_id)?;
    let supported_config = device
        .default_input_config()
        .map_err(|e| anyhow!("Failed to get default input config: {}", e))?;

    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.into();

    let detected = Arc::new(Mutex::new(false));
    let detected_clone = Arc::clone(&detected);

    let err_fn = |err: cpal::StreamError| {
        eprintln!("Audio stream error during mic test: {}", err);
    };

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let has_signal = data.iter().any(|&s| s.abs() > 0.001);
                if has_signal {
                    *detected_clone.lock().unwrap() = true;
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => {
            let detected_clone2 = Arc::clone(&detected);
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let has_signal = data
                        .iter()
                        .any(|&s| (s as f32 / i16::MAX as f32).abs() > 0.001);
                    if has_signal {
                        *detected_clone2.lock().unwrap() = true;
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let detected_clone2 = Arc::clone(&detected);
            device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let has_signal = data.iter().any(|&s| {
                        let normalized = (s as f32 / u16::MAX as f32) * 2.0 - 1.0;
                        normalized.abs() > 0.001
                    });
                    if has_signal {
                        *detected_clone2.lock().unwrap() = true;
                    }
                },
                err_fn,
                None,
            )
        }
        _ => return Err(anyhow!("Unsupported sample format: {:?}", sample_format)),
    }
    .map_err(|e| anyhow!("Failed to build test stream: {}", e))?;

    stream
        .play()
        .map_err(|e| anyhow!("Failed to play test stream: {}", e))?;

    // Listen for ~500ms to detect audio.
    std::thread::sleep(std::time::Duration::from_millis(500));

    drop(stream);

    let result = *detected.lock().unwrap();
    Ok(result)
}

// ── Audio data processing ───────────────────────────────────────────────────

/// Process incoming audio data: compute level and optionally record.
///
/// The input `data` is expected to be interleaved multi-channel f32 samples.
/// We down-mix to mono, compute a smoothed audio level, and if recording is
/// active we append the mono samples to the record buffer.
fn process_audio_data(data: &[f32], channels: u16, state: &Arc<Mutex<AudioState>>) {
    if data.is_empty() {
        return;
    }

    let channels = channels as usize;

    // Down-mix to mono by averaging across channels.
    let mono_samples: Vec<f32> = data
        .chunks(channels)
        .map(|frame| {
            let sum: f32 = frame.iter().sum();
            sum / channels as f32
        })
        .collect();

    // Calculate mean absolute value for level.
    let mean_abs: f32 =
        mono_samples.iter().map(|s| s.abs()).sum::<f32>() / mono_samples.len() as f32;

    // Clamp to 0.0 - 1.0 range.
    let level = mean_abs.min(1.0);

    let mut state = state.lock().unwrap();

    // Smooth the level with exponential moving average (history weight 0.4).
    state.audio_level = state.audio_level * 0.4 + level * 0.6;

    // If recording, append mono samples to the buffer.
    if state.is_recording {
        state.record_buffer.extend_from_slice(&mono_samples);
    }
}
