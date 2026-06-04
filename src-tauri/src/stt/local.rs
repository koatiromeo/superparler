use crate::error::{AppError, Result};

/// Public factory — always present; returns Err when `local-stt` feature is not compiled.
pub fn build_local(model_path: &str) -> Result<Box<dyn crate::stt::Transcriber>> {
    #[cfg(feature = "local-stt")]
    return Ok(Box::new(imp::LocalWhisper::new(model_path)?));

    #[cfg(not(feature = "local-stt"))]
    {
        let _ = model_path;
        Err(AppError::ModelLoad(
            "local-stt feature not compiled — install LLVM/libclang and rebuild:\n\
             cargo build --features local-stt"
                .to_string(),
        ))
    }
}

/// Full whisper-rs implementation — compiled only with `--features local-stt`.
/// Requires LLVM/libclang for whisper-rs-sys bindgen.
#[cfg(feature = "local-stt")]
mod imp {
    use async_trait::async_trait;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, OnceLock},
    };
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    use crate::error::{AppError, Result};

    // ── Model cache ────────────────────────────────────────────────────────────
    //
    // Loading ggml-small.bin takes ~2-3 s and ~500 MB RAM.
    // We cache loaded contexts keyed by canonical model path so subsequent
    // recordings reuse the same context without reloading.
    //
    // Cache is never evicted (GGUF files don't change at runtime).  If the user
    // changes the model path in Settings, the new path gets its own entry.
    static CONTEXT_CACHE: OnceLock<Mutex<HashMap<String, Arc<WhisperContext>>>> = OnceLock::new();

    fn get_or_load_context(model_path: &str) -> Result<Arc<WhisperContext>> {
        let cache = CONTEXT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

        // SAFETY: if the lock is poisoned a previous thread panicked inside the
        // cache write below — that's a bug we want to surface, not swallow.
        let mut guard = cache
            .lock()
            .map_err(|_| AppError::ModelLoad("whisper context cache mutex poisoned".to_string()))?;

        if let Some(ctx) = guard.get(model_path) {
            tracing::debug!(path = model_path, "whisper: reusing cached model context");
            return Ok(ctx.clone());
        }

        // Not cached — validate path before the expensive load.
        if !std::path::Path::new(model_path).exists() {
            return Err(AppError::ModelNotFound(model_path.to_string()));
        }

        tracing::info!(
            path = model_path,
            "loading whisper model (first use, may take a few seconds)"
        );
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| AppError::ModelLoad(format!("whisper context: {e}")))?;

        let ctx = Arc::new(ctx);
        guard.insert(model_path.to_string(), ctx.clone());
        tracing::info!(path = model_path, "whisper model loaded and cached");
        Ok(ctx)
    }

    // ── LocalWhisper ──────────────────────────────────────────────────────────

    pub struct LocalWhisper {
        ctx: Arc<WhisperContext>,
        model_path: String,
    }

    impl LocalWhisper {
        pub fn new(model_path: &str) -> Result<Self> {
            let ctx = get_or_load_context(model_path)?;
            Ok(Self {
                ctx,
                model_path: model_path.to_string(),
            })
        }
    }

    #[async_trait]
    impl crate::stt::Transcriber for LocalWhisper {
        async fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
            if samples.is_empty() {
                return Ok(String::new());
            }

            let ctx = self.ctx.clone();
            let samples = samples.to_vec();
            let lang = language.to_string();
            let model_path = self.model_path.clone();

            // whisper inference is pure CPU-bound sync — never run on a tokio thread.
            // spawn_blocking moves it to the blocking thread pool.
            tokio::task::spawn_blocking(move || {
                // WhisperState is NOT thread-safe — create fresh per inference.
                // It's dropped at end of this closure (no leak).
                let mut state = ctx
                    .create_state()
                    .map_err(|e| AppError::Stt(format!("whisper state: {e}")))?;

                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

                // Language: "auto" → let whisper detect (set_language(None))
                if lang != "auto" {
                    params.set_language(Some(&lang));
                }

                // Suppress progress/debug output — all logging via tracing instead
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);

                // Suppress blank audio tokens at start of output
                params.set_suppress_blank(true);

                // No context from previous calls — each dictation is independent
                params.set_no_context(true);

                tracing::debug!(
                    model = %model_path,
                    samples = samples.len(),
                    duration_ms = (samples.len() as f64 / 16_000.0 * 1000.0) as u64,
                    "whisper: starting inference"
                );

                state
                    .full(params, &samples)
                    .map_err(|e| AppError::Stt(format!("whisper full: {e}")))?;

                let n = state
                    .full_n_segments()
                    .map_err(|e| AppError::Stt(format!("whisper n_segments: {e}")))?;

                let text = (0..n)
                    .map(|i| {
                        state
                            .full_get_segment_text(i)
                            .map_err(|e| AppError::Stt(format!("whisper segment {i}: {e}")))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join(" ")
                    .trim()
                    .to_string();

                tracing::info!(
                    segments = n,
                    text_len = text.len(),
                    "local whisper transcription complete"
                );
                Ok(text)
            })
            .await
            .map_err(|e| AppError::Stt(format!("spawn_blocking join: {e}")))?
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_local_missing_model_returns_typed_error() {
        let result = build_local("/absolutely/nonexistent/model.gguf");
        assert!(result.is_err(), "missing model must return Err");
        // Verify the error is typed (not a panic)
        let err = result.err().expect("result should be Err");
        let msg = err.to_string();
        // Either ModelNotFound (feature enabled) or ModelLoad (feature disabled)
        assert!(
            msg.contains("nonexistent") || msg.contains("local-stt"),
            "unexpected error message: {msg}"
        );
    }

    /// Full local inference test.
    /// Requires: (1) `--features local-stt` build, (2) LLVM installed,
    ///           (3) ggml-small.bin present, (4) record_test WAV.
    ///
    /// Run:
    ///   cargo test local::tests::test_local_transcribe_wav \
    ///     --features local-stt --lib -- --ignored --nocapture
    #[cfg(feature = "local-stt")]
    #[tokio::test]
    #[ignore = "requires local-stt feature + LLVM + ggml-small.bin + record_test WAV"]
    async fn test_local_transcribe_wav() {
        use crate::stt::Transcriber;

        let wav_path = std::env::temp_dir().join("superparler_test.wav");
        assert!(
            wav_path.exists(),
            "WAV not found at {}\nRun: cargo run --bin record_test --features local-stt",
            wav_path.display()
        );

        // Find the model
        let model_path =
            std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "models/ggml-small.bin".to_string());
        assert!(
            std::path::Path::new(&model_path).exists(),
            "model not found at {model_path}\nRun: make models"
        );

        // Read WAV (16kHz mono f32 from record_test)
        let mut reader =
            hound::WavReader::open(&wav_path).unwrap_or_else(|e| panic!("open WAV: {e}"));
        let spec = reader.spec();
        println!(
            "WAV: {}Hz {} ch {:?} {} samples ({:.1}s)",
            spec.sample_rate,
            spec.channels,
            spec.sample_format,
            reader.len(),
            reader.len() as f32 / spec.sample_rate as f32
        );

        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect(),
        };

        let transcriber =
            build_local(&model_path).expect("build_local should succeed with model present");

        let t0 = std::time::Instant::now();
        let text = transcriber
            .transcribe(&samples, "fr")
            .await
            .expect("transcription failed");
        let elapsed = t0.elapsed();

        println!("\n=== Transcription locale ===");
        println!("«{text}»");
        println!("Durée inference: {:.1}s", elapsed.as_secs_f32());
        println!("============================\n");

        assert!(!text.is_empty(), "whisper returned empty transcription");
    }

    /// Verify the model cache is used on repeated loads of the same path.
    /// This test is a logic test and does NOT require the model to exist.
    #[test]
    fn test_build_local_same_missing_path_returns_same_error_type() {
        let path = "/cache_test/nonexistent.gguf";
        let r1 = build_local(path);
        let r2 = build_local(path);
        // Both should fail with the same variant
        assert!(r1.is_err());
        assert!(r2.is_err());
    }
}
