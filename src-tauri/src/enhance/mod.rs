use async_trait::async_trait;

pub mod groq_llm;

use crate::error::Result;
use groq_llm::GroqLlm;

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

pub fn build_enhancer(enabled: bool, enhance_model: &str) -> Box<dyn Enhancer> {
    if enabled {
        match GroqLlm::new(enhance_model.to_string()) {
            Ok(llm) => Box::new(llm),
            Err(e) => {
                tracing::error!("failed to create GroqLlm enhancer: {e}; falling back to NoOp");
                Box::new(NoOp)
            }
        }
    } else {
        Box::new(NoOp)
    }
}
