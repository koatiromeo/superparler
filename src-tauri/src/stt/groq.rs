use async_trait::async_trait;
use reqwest::multipart;

use super::Transcriber;
use crate::error::{AppError, Result};

const GROQ_TRANSCRIPTION_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

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

    fn get_api_key() -> Result<String> {
        let entry = keyring::Entry::new("superparler", "groq_api_key")
            .map_err(|e| AppError::Keyring(e.to_string()))?;
        entry.get_password().map_err(|_| AppError::Keyring("Groq API key not set — use engine:set_groq_key".to_string()))
    }

    /// Encode f32 samples to WAV bytes in memory using hound.
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
            writer.write_sample(s).map_err(|e| AppError::Stt(format!("WAV write: {e}")))?;
        }
        writer.finalize().map_err(|e| AppError::Stt(format!("WAV finalize: {e}")))?;
        Ok(buf.into_inner())
    }
}

#[async_trait]
impl Transcriber for GroqWhisper {
    async fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        let api_key = Self::get_api_key()?;
        let wav_bytes = Self::encode_wav(samples)?;

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AppError::Network(e.to_string()))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        tracing::info!(model = %self.model, "sending audio to Groq API");

        let response = self
            .client
            .post(GROQ_TRANSCRIPTION_URL)
            .bearer_auth(&api_key)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Stt(format!("Groq API error {status}: {body}")));
        }

        let json: serde_json::Value = response.json().await?;
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

    // Test WAV encoding produces valid bytes
    #[test]
    fn test_encode_wav_produces_valid_bytes() {
        let samples: Vec<f32> = (0..1600).map(|i| (i as f32 / 1600.0 * 2.0 - 1.0)).collect();
        let result = GroqWhisper::encode_wav(&samples).unwrap();
        // WAV files start with "RIFF"
        assert_eq!(&result[0..4], b"RIFF");
        assert!(result.len() > 44, "WAV must have header + data");
    }

    // Integration test with mock server — requires mockito feature
    // TODO: add #[tokio::test] with mockito for full HTTP mock
}
