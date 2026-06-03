use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use crate::error::{AppError, Result};

/// Resample interleaved audio to 16 kHz mono f32 (required by Whisper / Silero VAD).
pub fn to_16khz_mono(samples: &[f32], source_rate: u32, channels: u16) -> Result<Vec<f32>> {
    let channels = channels as usize;
    const TARGET_RATE: u32 = 16_000;

    // Step 1: deinterleave to mono by averaging channels
    let mono: Vec<f32> = if channels == 1 {
        samples.to_vec()
    } else {
        samples
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    if source_rate == TARGET_RATE {
        return Ok(mono);
    }

    // Step 2: resample to 16 kHz
    let ratio = TARGET_RATE as f64 / source_rate as f64;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let chunk_size = 1024;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .map_err(|e| AppError::AudioStream(format!("resampler init: {e}")))?;

    // Pad input to multiple of chunk_size
    let padded_len = ((mono.len() + chunk_size - 1) / chunk_size) * chunk_size;
    let mut padded = mono;
    padded.resize(padded_len, 0.0);

    let mut output = Vec::with_capacity((padded_len as f64 * ratio) as usize + 64);
    for chunk in padded.chunks(chunk_size) {
        let resampled = resampler
            .process(&[chunk], None)
            .map_err(|e| AppError::AudioStream(format!("resample error: {e}")))?;
        output.extend_from_slice(&resampled[0]);
    }

    tracing::debug!(input_samples = padded_len, output_samples = output.len(), "resampled audio");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_passthrough_16khz_mono() {
        let samples: Vec<f32> = (0..1600).map(|i| (i as f32 / 1600.0).sin()).collect();
        let result = to_16khz_mono(&samples, 16_000, 1).unwrap();
        assert_eq!(result.len(), samples.len());
    }

    #[test]
    fn test_resample_stereo_to_mono() {
        // Stereo 16 kHz — should average channels and return same length
        let samples: Vec<f32> = (0..3200).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let result = to_16khz_mono(&samples, 16_000, 2).unwrap();
        // Mono: half the samples
        assert_eq!(result.len(), 1600);
        // Average of 1.0 and -1.0 = 0.0
        for s in &result {
            assert!(s.abs() < 1e-5, "expected 0.0, got {s}");
        }
    }

    #[test]
    fn test_resample_44100_to_16000() {
        // 1 second at 44.1 kHz → expect ~16000 samples at 16 kHz
        let samples: Vec<f32> = (0..44100).map(|i| (i as f32 * 440.0 / 44100.0).sin()).collect();
        let result = to_16khz_mono(&samples, 44_100, 1).unwrap();
        let expected: usize = 16000;
        let tolerance: usize = 100;
        assert!(
            result.len().abs_diff(expected) < tolerance,
            "expected ~{expected} samples, got {}",
            result.len()
        );
    }
}
