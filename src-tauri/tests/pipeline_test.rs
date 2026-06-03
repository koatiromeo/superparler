// Integration tests for the STT pipeline
// Run with: cargo test --manifest-path src-tauri/Cargo.toml --test pipeline_test

use superparler_lib::{
    config::{AppConfig, Engine},
    stt::factory::build_transcriber,
};

/// Test that factory returns Err for local engine when model is missing
#[tokio::test]
async fn test_factory_local_missing_model() {
    let config = AppConfig {
        engine: Engine::Local,
        local_model_path: "/definitely/does/not/exist/model.gguf".to_string(),
        ..AppConfig::default()
    };
    let result = build_transcriber(&config);
    assert!(
        result.is_err(),
        "Expected error for missing local model, got Ok"
    );
}

/// Test that factory builds Groq transcriber without error (key checked at transcribe time)
#[tokio::test]
async fn test_factory_groq_builds_ok() {
    let config = AppConfig {
        engine: Engine::Groq,
        ..AppConfig::default()
    };
    let result = build_transcriber(&config);
    assert!(result.is_ok(), "Groq factory should succeed: {:?}", result.err());
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
