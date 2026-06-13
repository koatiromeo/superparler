// Integration tests for the STT pipeline
// Run with: cargo test --manifest-path src-tauri/Cargo.toml --test pipeline_test

use superparler_lib::{
    config::{AppConfig, Engine},
    error::AppError,
    models,
    stt::factory::build_transcriber,
};

/// Local engine with no downloaded model must return a clean `ModelNotFound`
/// (never panic). If the model happens to be installed on this machine, the
/// factory may legitimately succeed — assert against the actual install state.
#[tokio::test]
async fn test_factory_local_missing_model() {
    let config = AppConfig {
        engine: Engine::Local,
        ..AppConfig::default()
    };
    let installed = models::is_parakeet_v3_installed();
    match build_transcriber(&config) {
        Ok(_) => assert!(
            installed,
            "factory returned Ok but the model is not installed"
        ),
        Err(e) => assert!(
            matches!(e, AppError::ModelNotFound(_)),
            "expected ModelNotFound for a missing local model, got: {e}"
        ),
    }
}

/// Test that factory builds Groq transcriber without error (key checked at transcribe time)
#[tokio::test]
async fn test_factory_groq_builds_ok() {
    let config = AppConfig {
        engine: Engine::Groq,
        ..AppConfig::default()
    };
    let result = build_transcriber(&config);
    assert!(
        result.is_ok(),
        "Groq factory should succeed: {:?}",
        result.err()
    );
}

/// Test audio resample passthrough (16kHz mono → 16kHz mono)
#[test]
fn test_resample_identity() {
    use superparler_lib::audio::resample::to_16khz_mono;
    let samples: Vec<f32> = (0..1600).map(|i| (i as f32 / 1600.0).sin()).collect();
    let result = to_16khz_mono(&samples, 16_000, 1).unwrap();
    assert_eq!(result.len(), samples.len());
}

/// Test stereo → mono averaging
#[test]
fn test_resample_stereo_to_mono_length() {
    use superparler_lib::audio::resample::to_16khz_mono;
    let samples: Vec<f32> = vec![1.0f32; 3200]; // stereo 1600 frames
    let result = to_16khz_mono(&samples, 16_000, 2).unwrap();
    assert_eq!(result.len(), 1600, "Expected 1600 mono frames");
}
