use crate::error::Result;

#[cfg(feature = "vad")]
const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SIZE: usize = 512;
#[cfg(feature = "vad")]
const VAD_THRESHOLD: f32 = 0.5;

/// Trim leading and trailing silence from 16 kHz mono f32 samples.
/// When the `vad` feature is enabled, uses Silero VAD via ort.
/// Without the feature, applies a simple energy-based threshold.
pub fn trim_silence(samples: &[f32]) -> Result<Vec<f32>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(feature = "vad")]
    return trim_silero(samples);

    #[cfg(not(feature = "vad"))]
    return trim_energy(samples);
}

/// Silero VAD (requires `vad` feature + LLVM for ort bindgen).
#[cfg(feature = "vad")]
fn trim_silero(samples: &[f32]) -> Result<Vec<f32>> {
    use voice_activity_detector::VoiceActivityDetector;

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

    let first = chunks.iter().position(|(_, s)| *s);
    let last = chunks.iter().rposition(|(_, s)| *s);

    match (first, last) {
        (Some(start), Some(end)) => {
            let s = start * CHUNK_SIZE;
            let e = ((end + 1) * CHUNK_SIZE).min(samples.len());
            tracing::debug!(speech_chunks = end - start + 1, "Silero VAD trimmed silence");
            Ok(samples[s..e].to_vec())
        }
        _ => {
            tracing::warn!("VAD found no speech in audio");
            Ok(Vec::new())
        }
    }
}

/// Simple energy-based fallback when Silero VAD is not compiled in.
/// Keeps chunks where RMS energy exceeds a threshold.
#[cfg(not(feature = "vad"))]
fn trim_energy(samples: &[f32]) -> Result<Vec<f32>> {
    const ENERGY_THRESHOLD: f32 = 0.01;

    let chunks: Vec<(usize, bool)> = samples
        .chunks(CHUNK_SIZE)
        .enumerate()
        .map(|(i, chunk)| {
            let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
            (i, rms > ENERGY_THRESHOLD)
        })
        .collect();

    let first = chunks.iter().position(|(_, s)| *s);
    let last = chunks.iter().rposition(|(_, s)| *s);

    match (first, last) {
        (Some(start), Some(end)) => {
            let s = start * CHUNK_SIZE;
            let e = ((end + 1) * CHUNK_SIZE).min(samples.len());
            tracing::debug!(speech_chunks = end - start + 1, "energy VAD trimmed silence");
            Ok(samples[s..e].to_vec())
        }
        _ => {
            tracing::warn!("energy VAD found no speech");
            Ok(Vec::new())
        }
    }
}
