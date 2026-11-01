//! Audio helpers: resampling and WAV encoding.

use super::WhisperManager;
use anyhow::Result;

use hound::{SampleFormat, WavSpec, WavWriter};
use std::io::Cursor;

impl WhisperManager {
    pub(crate) fn encode_wav(audio_data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec)?;
            for &sample in audio_data {
                let clamped = sample.clamp(-1.0, 1.0);
                let int_sample = (clamped * i16::MAX as f32) as i16;
                writer.write_sample(int_sample)?;
            }
            writer.finalize()?;
        }

        Ok(cursor.into_inner())
    }

    /// Simple linear resampling from `from_rate` to `to_rate`.
    ///
    /// Shared with the live-session provider, which needs 16 kHz audio for
    /// Deepgram. Keep this the single implementation — a second resampler would
    /// drift.
    pub(crate) fn resample(data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate || data.is_empty() {
            return data.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (data.len() as f64 / ratio).ceil() as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f64 * ratio;
            let idx_floor = src_idx.floor() as usize;
            let idx_ceil = (idx_floor + 1).min(data.len() - 1);
            let frac = (src_idx - idx_floor as f64) as f32;

            let sample = data[idx_floor] * (1.0 - frac) + data[idx_ceil] * frac;
            resampled.push(sample);
        }

        resampled
    }
}
