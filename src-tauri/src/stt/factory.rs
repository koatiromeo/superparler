use super::{groq::GroqWhisper, local::LocalWhisper, Transcriber};
use crate::{
    config::{AppConfig, Engine},
    error::Result,
};

/// Build the appropriate Transcriber based on the current config.
/// Called once per recording session (cheap for Groq, expensive for Local on first load).
pub fn build_transcriber(config: &AppConfig) -> Result<Box<dyn Transcriber>> {
    match config.engine {
        Engine::Local => {
            tracing::info!(path = %config.local_model_path, "building local whisper transcriber");
            Ok(Box::new(LocalWhisper::new(&config.local_model_path)?))
        }
        Engine::Groq => {
            tracing::info!(model = %config.groq_model, "building Groq transcriber");
            Ok(Box::new(GroqWhisper::new(config.groq_model.clone())?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, Engine};

    #[test]
    fn test_factory_groq_missing_key_returns_error_at_transcribe_time() {
        // Factory builds successfully even without key (key checked at transcribe time)
        let config = AppConfig { engine: Engine::Groq, ..AppConfig::default() };
        let result = build_transcriber(&config);
        assert!(result.is_ok(), "factory should succeed; key checked at transcribe time");
    }

    #[test]
    fn test_factory_local_missing_model_returns_error() {
        let config = AppConfig {
            engine: Engine::Local,
            local_model_path: "/nonexistent/model.gguf".to_string(),
            ..AppConfig::default()
        };
        let result = build_transcriber(&config);
        assert!(result.is_err(), "factory should fail for missing model file");
    }
}
