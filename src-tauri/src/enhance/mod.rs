use async_trait::async_trait;

pub mod groq_llm;

use crate::error::Result;

/// Optional post-transcription text enhancement (reformatting, correction, etc.).
/// NoOp is the default — enabled only when config.enhance_enabled is true.
#[async_trait]
pub trait Enhancer: Send + Sync {
    async fn enhance(&self, text: &str, prompt: &str) -> Result<String>;
}

/// Default no-op enhancer: returns text unchanged.
pub struct NoOp;

#[async_trait]
impl Enhancer for NoOp {
    async fn enhance(&self, text: &str, _prompt: &str) -> Result<String> {
        Ok(text.to_string())
    }
}

pub fn build_enhancer(enabled: bool, _groq_model: &str) -> Box<dyn Enhancer> {
    if enabled {
        // TODO(v2): switch to GroqLlm when enhance feature is complete
        tracing::warn!("enhance mode requested but GroqLlm enhancer not yet implemented; using NoOp");
        Box::new(NoOp)
    } else {
        Box::new(NoOp)
    }
}
