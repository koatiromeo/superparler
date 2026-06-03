use async_trait::async_trait;

pub mod factory;
pub mod groq;
pub mod local;

use crate::error::Result;

/// Core abstraction over all STT engines.
/// Input: 16 kHz mono f32 samples. Output: transcribed text.
#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, samples: &[f32], language: &str) -> Result<String>;
}
