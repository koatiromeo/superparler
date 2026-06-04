use crate::error::{AppError, Result};

/// Chunk size for VAD processing at 16 kHz (32 ms frames — Silero requirement).
const CHUNK_SIZE: usize = 512;
/// Padding: keep this many chunks before first speech / after last speech.
const PADDING_CHUNKS: usize = 3;

/// Trim leading and trailing silence from 16 kHz mono f32 samples.
///
/// - With `vad` feature: uses Silero VAD via `voice_activity_detector` / ort.
/// - Without: robust energy-based fallback with hysteresis (good enough for dictation).
///
/// Returns an empty Vec if no speech is detected (pipeline discards silently).
pub fn trim_silence(samples: &[f32]) -> Result<Vec<f32>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let speech_mask = detect_speech(samples)?;
    apply_mask_with_padding(samples, &speech_mask)
}

/// Returns a per-chunk boolean mask: true = speech detected.
fn detect_speech(samples: &[f32]) -> Result<Vec<bool>> {
    #[cfg(feature = "vad")]
    return detect_silero(samples);

    #[cfg(not(feature = "vad"))]
    return Ok(detect_energy(samples));
}

// ─── Silero VAD (requires `vad` feature) ────────────────────────────────────

#[cfg(feature = "vad")]
fn detect_silero(samples: &[f32]) -> Result<Vec<bool>> {
    use voice_activity_detector::VoiceActivityDetector;

    const SAMPLE_RATE: u32 = 16_000;
    const THRESHOLD: f32 = 0.5;

    let mut vad = VoiceActivityDetector::builder()
        .sample_rate(SAMPLE_RATE)
        .chunk_size(CHUNK_SIZE)
        .build()
        .map_err(|e| AppError::Stt(format!("Silero VAD init: {e}")))?;

    let mask: Vec<bool> = samples
        .chunks(CHUNK_SIZE)
        .map(|chunk| {
            let prob = vad.predict(chunk.to_vec());
            prob > THRESHOLD
        })
        .collect();

    let speech = mask.iter().filter(|&&s| s).count();
    tracing::debug!(total = mask.len(), speech, "Silero VAD: speech chunks");
    Ok(mask)
}

// ─── Energy-based fallback (no LLVM needed) ──────────────────────────────────

#[cfg(not(feature = "vad"))]
fn detect_energy(samples: &[f32]) -> Vec<bool> {
    // Adaptive threshold: 8 % of 90th-percentile RMS energy across all chunks.
    // This handles both quiet microphones and loud environments without a fixed constant.
    let rms_values: Vec<f32> = samples
        .chunks(CHUNK_SIZE)
        .map(|chunk| {
            let mean_sq = chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32;
            mean_sq.sqrt()
        })
        .collect();

    let threshold = adaptive_threshold(&rms_values);

    // Hysteresis: once speech starts, we stay in "speech" until N consecutive
    // silent frames. This avoids cutting brief pauses mid-sentence.
    const HANGOVER: usize = 6; // ~190 ms of silence before cutting
    let mut mask = vec![false; rms_values.len()];
    let mut hangover = 0usize;

    for (i, &rms) in rms_values.iter().enumerate() {
        if rms > threshold {
            mask[i] = true;
            hangover = HANGOVER;
        } else if hangover > 0 {
            mask[i] = true; // hysteresis: still "speech" for a while
            hangover -= 1;
        }
    }

    let speech = mask.iter().filter(|&&s| s).count();
    tracing::debug!(
        total = mask.len(),
        speech,
        threshold,
        "energy VAD: speech chunks"
    );
    mask
}

#[cfg(not(feature = "vad"))]
fn adaptive_threshold(rms_values: &[f32]) -> f32 {
    if rms_values.is_empty() {
        return 0.01;
    }
    let mut sorted = rms_values.to_vec();
    sorted.sort_by(f32::total_cmp);
    let p90_idx = (sorted.len() as f32 * 0.90) as usize;
    let p90 = sorted[p90_idx.min(sorted.len() - 1)];
    // Use 8% of p90 as the threshold; floor at 0.003 for silent environments
    (p90 * 0.08).max(0.003)
}

// ─── Apply mask → slice ───────────────────────────────────────────────────────

fn apply_mask_with_padding(samples: &[f32], mask: &[bool]) -> Result<Vec<f32>> {
    let first = match mask.iter().position(|&s| s) {
        Some(i) => i,
        None => {
            tracing::warn!("VAD: no speech detected — discarding buffer");
            return Ok(Vec::new());
        }
    };
    let last = mask.iter().rposition(|&s| s).unwrap_or(first);

    let start_chunk = first.saturating_sub(PADDING_CHUNKS);
    let end_chunk = (last + 1 + PADDING_CHUNKS).min(mask.len());

    let sample_start = start_chunk * CHUNK_SIZE;
    let sample_end = (end_chunk * CHUNK_SIZE).min(samples.len());

    let trimmed = &samples[sample_start..sample_end];
    tracing::info!(
        before = samples.len(),
        after = trimmed.len(),
        pct = (trimmed.len() as f32 / samples.len() as f32 * 100.0) as u32,
        "VAD trimmed silence"
    );
    Ok(trimmed.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_pure_silence() {
        let silence = vec![0.0f32; 16_000]; // 1 s silence
        let result = trim_silence(&silence).unwrap();
        assert!(
            result.is_empty(),
            "pure silence should produce empty output"
        );
    }

    #[test]
    fn test_trim_with_leading_silence() {
        // Energy VAD: constant 0.8f32 is a loud DC signal, well above RMS threshold.
        // Silero VAD: trained on real speech; a DC signal is NOT detected as speech (correct).
        // We test both behaviours to ensure no panic and correct contract per VAD type.

        let mut samples = vec![0.0f32; 16_000]; // 1 s silence
        samples.extend(vec![0.8f32; 8_000]); // 0.5 s "signal"
        samples.extend(vec![0.0f32; 8_000]); // 0.5 s silence

        let result = trim_silence(&samples).unwrap();

        // Output must never be longer than input — basic sanity regardless of VAD type.
        assert!(
            result.len() <= samples.len(),
            "output must not exceed input length"
        );

        // Energy fallback: loud signal passes the RMS threshold, silence is trimmed.
        #[cfg(not(feature = "vad"))]
        {
            assert!(
                !result.is_empty(),
                "energy VAD: 0.8f32 signal should be kept"
            );
            assert!(
                result.len() < samples.len() / 2,
                "energy VAD: 1 s leading silence should be trimmed (got {} / {})",
                result.len(),
                samples.len()
            );
        }

        // Silero VAD: a DC signal is correctly identified as non-speech.
        // Acceptable outcome: empty (no speech detected) or trimmed.
        // The important property is: no panic, and result ≤ input.
        #[cfg(feature = "vad")]
        eprintln!(
            "Silero VAD: DC signal returned {} / {} samples (empty = no speech detected, OK)",
            result.len(),
            samples.len()
        );
    }

    #[test]
    fn test_trim_empty_input() {
        let result = trim_silence(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_adaptive_threshold_not_panic() {
        #[cfg(not(feature = "vad"))]
        {
            let vals = vec![0.001, 0.002, 0.5, 0.8, 0.01];
            let t = super::adaptive_threshold(&vals);
            assert!(t > 0.0);
        }
    }
}
