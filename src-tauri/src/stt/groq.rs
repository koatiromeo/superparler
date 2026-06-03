use async_trait::async_trait;
use reqwest::multipart;

use super::Transcriber;
use crate::error::{AppError, Result};

const GROQ_TRANSCRIPTION_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
/// Keyring service name — must match `commands::engine::set_groq_key`.
const KEYRING_SERVICE: &str = "superparler";
const KEYRING_ACCOUNT: &str = "groq";

pub struct GroqWhisper {
    model: String,
    client: reqwest::Client,
}

impl GroqWhisper {
    pub fn new(model: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;
        Ok(Self { model, client })
    }

    /// Read the Groq API key.
    /// Priority: OS keyring ("superparler"/"groq") → env var GROQ_API_KEY.
    fn get_api_key() -> Result<String> {
        // 1. Try OS keyring
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
            if let Ok(key) = entry.get_password() {
                if !key.trim().is_empty() {
                    return Ok(key);
                }
            }
        }

        // 2. Fall back to environment variable
        std::env::var("GROQ_API_KEY").map_err(|_| {
            AppError::Keyring(
                "Groq API key not found. Set it via:\n\
                 • app Settings → Moteur → Clé API (stored in OS keyring), or\n\
                 • environment variable GROQ_API_KEY"
                    .to_string(),
            )
        })
    }

    /// Encode 16 kHz mono f32 samples to in-memory WAV bytes (hound).
    fn encode_wav(samples: &[f32]) -> Result<Vec<u8>> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut buf, spec)
            .map_err(|e| AppError::Stt(format!("WAV encode: {e}")))?;
        for &s in samples {
            writer
                .write_sample(s)
                .map_err(|e| AppError::Stt(format!("WAV write sample: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| AppError::Stt(format!("WAV finalize: {e}")))?;
        Ok(buf.into_inner())
    }
}

#[async_trait]
impl Transcriber for GroqWhisper {
    async fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let api_key = Self::get_api_key()?;
        let wav_bytes = Self::encode_wav(samples)?;
        let wav_size_kb = wav_bytes.len() / 1024;

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AppError::Network(format!("MIME type: {e}")))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        tracing::info!(
            model = %self.model,
            wav_kb = wav_size_kb,
            language,
            "sending audio to Groq API"
        );

        let response = self
            .client
            .post(GROQ_TRANSCRIPTION_URL)
            .bearer_auth(&api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Groq request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Stt(format!("Groq API {status}: {body}")));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::Stt(format!("Groq JSON parse: {e}")))?;

        let text = json["text"]
            .as_str()
            .ok_or_else(|| AppError::Stt("Groq response missing 'text' field".to_string()))?
            .trim()
            .to_string();

        tracing::info!(text_len = text.len(), "Groq transcription complete");
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_wav_header() {
        let n = 1600usize;
        let samples: Vec<f32> = (0..n).map(|i| i as f32 / n as f32 * 2.0 - 1.0).collect();
        let bytes = GroqWhisper::encode_wav(&samples).unwrap();

        // RIFF/WAVE magic bytes
        assert_eq!(&bytes[0..4], b"RIFF", "WAV must start with RIFF");
        assert_eq!(&bytes[8..12], b"WAVE", "RIFF type must be WAVE");

        // Total file size must accommodate at least the PCM data (n × 4 bytes)
        assert!(
            bytes.len() >= 44 + n * 4,
            "WAV too small: {} bytes for {} samples",
            bytes.len(),
            n
        );

        // Round-trip: read back with hound and verify sample count + values
        let cursor = std::io::Cursor::new(&bytes);
        let mut reader = hound::WavReader::new(cursor).expect("hound read-back");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);

        let read_back: Vec<f32> = reader
            .samples::<f32>()
            .map(|s| s.expect("read sample"))
            .collect();
        assert_eq!(read_back.len(), n, "sample count must match after round-trip");
        // First and last samples should be preserved within f32 precision
        assert!((read_back[0] - samples[0]).abs() < 1e-6);
        assert!((read_back[n - 1] - samples[n - 1]).abs() < 1e-6);
    }

    #[test]
    fn test_encode_wav_empty_samples() {
        let bytes = GroqWhisper::encode_wav(&[]).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
    }

    #[test]
    fn test_get_api_key_falls_back_to_env() {
        // SAFETY: single-threaded test; no other threads read GROQ_API_KEY concurrently.
        // Rust 2024 requires explicit unsafe for env mutation due to thread-safety concerns.
        unsafe { std::env::set_var("GROQ_API_KEY", "test_key_env_fallback") };
        let key = GroqWhisper::get_api_key();
        unsafe { std::env::remove_var("GROQ_API_KEY") };
        // Either keyring had a real key (priority) or env fallback was used — both OK.
        assert!(key.is_ok(), "get_api_key must succeed when GROQ_API_KEY is set");
    }

    /// Groq API connectivity test — sends a synthetic WAV (sine + noise at speech frequencies).
    /// Does NOT require a microphone or prior recording.
    /// Run: cargo test groq::tests::test_groq_api_connectivity -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "network — requires GROQ_API_KEY env var or keyring"]
    async fn test_groq_api_connectivity() {
        // Generate 2 seconds of speech-like audio (mixture of harmonics + noise)
        let samples: Vec<f32> = (0..32_000)
            .map(|i| {
                let t = i as f32 / 16_000.0;
                // Voiced speech approximation: F0=150 Hz + harmonics
                let voiced = 0.4 * (2.0 * std::f32::consts::PI * 150.0 * t).sin()
                    + 0.25 * (2.0 * std::f32::consts::PI * 300.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 450.0 * t).sin()
                    + 0.1 * (2.0 * std::f32::consts::PI * 600.0 * t).sin();
                // Amplitude envelope: fade in/out
                let env = (std::f32::consts::PI * i as f32 / 32_000.0).sin();
                voiced * env * 0.7
            })
            .collect();

        let groq = GroqWhisper::new("whisper-large-v3-turbo".to_string()).unwrap();
        let result = groq.transcribe(&samples, "fr").await;

        match result {
            Ok(text) => {
                println!("Groq API OK. Synthetic signal transcription: «{text}»");
                // Synthetic signal may produce empty or garbage — that's OK.
                // The important thing is the API call succeeded (no HTTP error).
            }
            Err(e) => panic!("Groq API call failed: {e}"),
        }
    }

    /// Real Groq transcription test — reads /tmp/superparler_test.wav (or %TEMP% on Windows).
    /// Run with: cargo test groq::tests::test_groq_transcribe_wav -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "network + requires record_test WAV file (run `cargo run --bin record_test` first)"]
    async fn test_groq_transcribe_wav() {
        let wav_path = std::env::temp_dir().join("superparler_test.wav");
        assert!(
            wav_path.exists(),
            "WAV not found at {}\nRun: cargo run --bin record_test --manifest-path src-tauri/Cargo.toml",
            wav_path.display()
        );

        // Read the WAV written by record_test (16kHz, mono, f32)
        let mut reader = hound::WavReader::open(&wav_path)
            .unwrap_or_else(|e| panic!("Cannot open WAV: {e}"));
        let spec = reader.spec();
        println!(
            "WAV: {}Hz, {} ch, {:?}, {} samples ({:.1}s)",
            spec.sample_rate,
            spec.channels,
            spec.sample_format,
            reader.len(),
            reader.len() as f32 / spec.sample_rate as f32
        );

        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .map(|s| s.expect("read f32 sample"))
                .collect(),
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.expect("read i16 sample") as f32 / i16::MAX as f32)
                .collect(),
        };

        assert!(!samples.is_empty(), "WAV is empty");

        let groq = GroqWhisper::new("whisper-large-v3-turbo".to_string())
            .expect("GroqWhisper::new failed");

        let text = groq
            .transcribe(&samples, "fr")
            .await
            .expect("transcription failed — check GROQ_API_KEY env var or keyring");

        println!("\n=== Transcription Groq ===");
        println!("«{text}»");
        println!("=========================\n");

        assert!(!text.is_empty(), "Groq returned empty transcription");
    }
}
