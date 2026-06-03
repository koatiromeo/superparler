use super::{groq::GroqWhisper, local, Transcriber};
use crate::{
    config::{AppConfig, Engine},
    error::Result,
};

/// Build the appropriate Transcriber based on the current config.
pub fn build_transcriber(config: &AppConfig) -> Result<Box<dyn Transcriber>> {
    match config.engine {
        Engine::Local => {
            tracing::info!(path = %config.local_model_path, "building local whisper transcriber");
            local::build_local(&config.local_model_path)
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
    fn test_factory_groq_builds_ok() {
        let config = AppConfig { engine: Engine::Groq, ..AppConfig::default() };
        let result = build_transcriber(&config);
        assert!(result.is_ok(), "Groq factory should succeed; key checked at transcribe time");
    }

    #[test]
    fn test_factory_local_missing_model_returns_error() {
        let config = AppConfig {
            engine: Engine::Local,
            local_model_path: "/nonexistent/model.gguf".to_string(),
            ..AppConfig::default()
        };
        let result = build_transcriber(&config);
        assert!(result.is_err(), "factory should fail for missing/unavailable local model");
    }
}
