use super::{Transcriber, groq::GroqWhisper, local::LocalParakeet};
use crate::{
    config::{AppConfig, Engine},
    error::{AppError, Result},
};

/// Build the transcriber for the configured engine.
///
/// `Local` (default) uses the offline Parakeet ONNX model — it must already be
/// downloaded (startup handles that). If it isn't, we return a clear
/// `ModelNotFound` rather than blocking, so the pipeline can tell the user to
/// wait for the download to finish.
pub fn build_transcriber(config: &AppConfig) -> Result<Box<dyn Transcriber>> {
    match config.engine {
        Engine::Local => {
            if !crate::models::is_parakeet_v3_installed() {
                return Err(AppError::ModelNotFound(
                    "Modèle Parakeet pas encore installé (téléchargement en cours). \
                     Réessaie dans un instant."
                        .to_string(),
                ));
            }
            let dir = crate::models::parakeet_v3_dir()?;
            tracing::info!("building local Parakeet transcriber");
            Ok(Box::new(LocalParakeet::new(dir)?))
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
        let config = AppConfig {
            engine: Engine::Groq,
            ..AppConfig::default()
        };
        let result = build_transcriber(&config);
        assert!(
            result.is_ok(),
            "Groq factory should succeed; key checked at transcribe time"
        );
    }

    #[test]
    fn test_factory_local_no_panic_when_model_absent() {
        // Default engine is Local. In CI the model isn't downloaded, so we must
        // return a clean ModelNotFound error — never panic.
        let config = AppConfig::default();
        if let Err(e) = build_transcriber(&config) {
            assert!(
                matches!(e, AppError::ModelNotFound(_)),
                "expected ModelNotFound, got: {e}"
            );
        }
    }
}
