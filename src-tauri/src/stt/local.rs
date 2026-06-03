use async_trait::async_trait;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::Transcriber;
use crate::error::{AppError, Result};

pub struct LocalWhisper {
    ctx: Arc<WhisperContext>,
}

impl LocalWhisper {
    pub fn new(model_path: &str) -> Result<Self> {
        if !std::path::Path::new(model_path).exists() {
            return Err(AppError::ModelNotFound(model_path.to_string()));
        }
        tracing::info!(path = model_path, "loading whisper model");
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| AppError::ModelLoad(e.to_string()))?;
        Ok(Self { ctx: Arc::new(ctx) })
    }
}

#[async_trait]
impl Transcriber for LocalWhisper {
    async fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        let ctx = self.ctx.clone();
        let samples = samples.to_vec();
        let lang = if language == "auto" { "auto".to_string() } else { language.to_string() };

        // whisper inference is CPU-bound sync — must not block the tokio thread pool
        tokio::task::spawn_blocking(move || {
            let mut state = ctx.create_state().map_err(|e| AppError::Stt(e.to_string()))?;
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            if lang != "auto" {
                params.set_language(Some(&lang));
            }
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            state.full(params, &samples).map_err(|e| AppError::Stt(e.to_string()))?;

            let n_segments = state.full_n_segments().map_err(|e| AppError::Stt(e.to_string()))?;
            let text = (0..n_segments)
                .map(|i| state.full_get_segment_text(i).map_err(|e| AppError::Stt(e.to_string())))
                .collect::<Result<Vec<_>>>()?
                .join(" ")
                .trim()
                .to_string();

            tracing::info!(text_len = text.len(), "local whisper transcription complete");
            Ok(text)
        })
        .await
        .map_err(|e| AppError::Stt(format!("spawn_blocking join error: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    // Integration test requires a real model — skipped in CI unless model is present
    // Run manually: cargo test stt::local -- --nocapture
}
