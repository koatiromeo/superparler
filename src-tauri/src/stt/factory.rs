use super::{Transcriber, groq::GroqWhisper};
use crate::{config::AppConfig, error::Result};

pub fn build_transcriber(config: &AppConfig) -> Result<Box<dyn Transcriber>> {
    tracing::info!(model = %config.groq_model, "building Groq transcriber");
    Ok(Box::new(GroqWhisper::new(config.groq_model.clone())?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn test_factory_groq_builds_ok() {
        let result = build_transcriber(&AppConfig::default());
        assert!(
            result.is_ok(),
            "Groq factory should succeed; key checked at transcribe time"
        );
    }
}
