//! spectrum.rs — live microphone → log-spaced frequency bands for the overlay
//! waveform, so the bars track the actual TONALITY of the voice (low → high)
//! instead of a canned animation.
//!
//! Stays ultra-light: the cpal audio callback only appends downmixed mono
//! samples to a small shared ring (cheap, lock-and-extend). The overlay thread
//! pulls a 1024-pt FFT at ~30 Hz (`compute_bands`) when it repaints — so the
//! heavy work runs on the UI timer, never on the realtime audio thread, and
//! nothing is computed at all while the overlay is hidden.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

const FFT_SIZE: usize = 1024;
const RING_CAP: usize = FFT_SIZE * 2;

static RING: Mutex<VecDeque<f32>> = Mutex::new(VecDeque::new());
static FFT: OnceLock<std::sync::Arc<dyn Fft<f32>>> = OnceLock::new();

fn fft() -> &'static std::sync::Arc<dyn Fft<f32>> {
    FFT.get_or_init(|| FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE))
}

/// Append interleaved capture samples (downmixed to mono) to the ring. Called
/// from the cpal audio callback — kept cheap (lock + extend, bounded).
pub fn push_samples(interleaved: &[f32], channels: u16) {
    let ch = channels.max(1) as usize;
    let Ok(mut ring) = RING.lock() else {
        return;
    };
    let mut i = 0;
    while i + ch <= interleaved.len() {
        let mut s = 0.0f32;
        for c in 0..ch {
            s += interleaved[i + c];
        }
        ring.push_back(s / ch as f32);
        i += ch;
    }
    let overflow = ring.len().saturating_sub(RING_CAP);
    for _ in 0..overflow {
        ring.pop_front();
    }
}

/// Reset between recordings so a new dictation starts from silence.
pub fn clear() {
    if let Ok(mut ring) = RING.lock() {
        ring.clear();
    }
}

/// Fill `out` with `out.len()` log-spaced band energies in 0..1 (low → high
/// frequency). Returns `false` if a full FFT window isn't available yet — the
/// caller should decay its bars instead of forcing them to zero.
pub fn compute_bands(out: &mut [f32]) -> bool {
    if out.is_empty() {
        return false;
    }
    let window: Vec<f32> = {
        let Ok(ring) = RING.lock() else {
            return false;
        };
        if ring.len() < FFT_SIZE {
            return false;
        }
        ring.iter().skip(ring.len() - FFT_SIZE).copied().collect()
    };

    // Hann window to tame spectral leakage, then forward FFT.
    let mut buf: Vec<Complex<f32>> = window
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_SIZE as f32 - 1.0)).cos();
            Complex { re: s * w, im: 0.0 }
        })
        .collect();
    fft().process(&mut buf);

    let half = FFT_SIZE / 2;
    let n = out.len();
    // Log-spaced bin edges across [1, half) so each bar covers a musical-ish
    // frequency span (low bars = bass, high bars = treble).
    let edge = |t: f32| -> usize {
        let (lo, hi) = (1.0f32, half as f32);
        (lo * (hi / lo).powf(t)).round().clamp(lo, hi) as usize
    };
    for (b, slot) in out.iter_mut().enumerate() {
        let k0 = edge(b as f32 / n as f32);
        let k1 = edge((b + 1) as f32 / n as f32).max(k0 + 1).min(half);
        let mut mag = 0.0f32;
        for c in &buf[k0..k1] {
            mag += c.norm();
        }
        mag /= (k1 - k0) as f32;
        // Log-compress magnitude to a perceptual 0..1 (constants tuned for mic
        // speech levels; adjust the +70 / 60 if bars sit too low or clip).
        let db = 20.0 * (mag + 1e-6).log10();
        *slot = ((db + 70.0) / 60.0).clamp(0.0, 1.0);
    }
    true
}
