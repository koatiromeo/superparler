use voice_activity_detector::VoiceActivityDetector;

use crate::error::{AppError, Result};

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SIZE: usize = 512;
const VAD_THRESHOLD: f32 = 0.5;

/// Trim leading and trailing silence from 16 kHz mono f32 samples using Silero VAD.
/// Returns the trimmed samples (may be empty if all silence).
pub fn trim_silence(samples: &[f32]) -> Result<Vec<f32>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(SAMPLE_RATE)
        .chunk_size(CHUNK_SIZE)
        .build()
        .map_err(|e| AppError::Stt(format!("VAD init: {e}")))?;

    let chunks: Vec<(usize, bool)> = samples
        .chunks(CHUNK_SIZE)
        .enumerate()
        .map(|(i, chunk)| {
            let prob = vad.predict(chunk.to_vec());
            (i, prob > VAD_THRESHOLD)
        })
        .collect();

    // Find first and last speech chunk
    let first_speech = chunks.iter().position(|(_, is_speech)| *is_speech);
    let last_speech = chunks.iter().rposition(|(_, is_speech)| *is_speech);

    match (first_speech, last_speech) {
        (Some(start), Some(end)) => {
            let sample_start = start * CHUNK_SIZE;
            let sample_end = ((end + 1) * CHUNK_SIZE).min(samples.len());
            tracing::debug!(
                total_chunks = chunks.len(),
                speech_chunks = end - start + 1,
                "VAD trimmed silence"
            );
            Ok(samples[sample_start..sample_end].to_vec())
        }
        _ => {
            tracing::warn!("VAD found no speech in audio");
            Ok(Vec::new())
        }
    }
}
