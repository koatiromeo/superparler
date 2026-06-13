//! local.rs — offline speech-to-text via NVIDIA Parakeet (ONNX Runtime).
//!
//! No API key, no network at transcription time. Loading the ONNX model costs a
//! few seconds + a few hundred MB of RAM, so we load it ONCE into a process-wide
//! cell and reuse it across dictations rather than rebuilding per request.
//!
//! RAM hygiene: an idle watcher unloads the model after `IDLE_UNLOAD_SECS` of
//! inactivity; the next dictation transparently reloads it. Robustness: inference
//! runs inside `catch_unwind` so a model panic unloads the engine and surfaces a
//! retryable error instead of poisoning the lock for the rest of the session.
//! Inference is CPU-bound sync code, so `transcribe` always runs on `spawn_blocking`.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use transcribe_rs::onnx::Quantization;
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};

use super::Transcriber;
use crate::error::{AppError, Result};

/// Process-wide loaded Parakeet model. `None` until first use / after unload.
static MODEL: OnceLock<Mutex<Option<ParakeetModel>>> = OnceLock::new();
/// Epoch-ms of the last transcription, for the idle watcher.
static LAST_USE_MS: AtomicU64 = AtomicU64::new(0);
/// Ensures we only ever spawn one idle watcher thread.
static WATCHER_STARTED: OnceLock<()> = OnceLock::new();

/// Free the model after this many seconds without a dictation.
const IDLE_UNLOAD_SECS: u64 = 300;

fn model_cell() -> &'static Mutex<Option<ParakeetModel>> {
    MODEL.get_or_init(|| Mutex::new(None))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct LocalParakeet {
    model_dir: PathBuf,
}

impl LocalParakeet {
    pub fn new(model_dir: PathBuf) -> Result<Self> {
        Ok(Self { model_dir })
    }

    /// Whether the model is currently resident in memory.
    pub fn is_loaded() -> bool {
        MODEL
            .get()
            .and_then(|c| c.lock().ok().map(|g| g.is_some()))
            .unwrap_or(false)
    }

    /// Drop the loaded model and free its RAM. No-op if not loaded.
    pub fn unload() {
        if let Some(cell) = MODEL.get() {
            if let Ok(mut guard) = cell.lock() {
                if guard.take().is_some() {
                    tracing::info!("Parakeet model unloaded");
                }
            }
        }
    }

    /// Load the model into the process-wide cell if not already loaded.
    /// Blocking — call from `spawn_blocking` or a dedicated thread.
    pub fn ensure_loaded(model_dir: &Path) -> Result<()> {
        let cell = model_cell();
        let mut guard = cell
            .lock()
            .map_err(|_| AppError::ModelLoad("Parakeet model lock poisoned".into()))?;
        if guard.is_none() {
            let st = std::time::Instant::now();
            tracing::info!(dir = %model_dir.display(), "loading Parakeet model…");
            let model = ParakeetModel::load(model_dir, &Quantization::Int8)
                .map_err(|e| AppError::ModelLoad(format!("Parakeet load: {e}")))?;
            *guard = Some(model);
            tracing::info!(ms = st.elapsed().as_millis(), "Parakeet model loaded");
        }
        Ok(())
    }

    /// Load the model in the background so the first dictation is instant.
    /// Call once at startup when the offline engine is active and installed.
    pub fn prewarm(model_dir: PathBuf) {
        std::thread::Builder::new()
            .name("parakeet-prewarm".into())
            .spawn(move || match Self::ensure_loaded(&model_dir) {
                Ok(()) => LAST_USE_MS.store(now_ms(), Ordering::Relaxed),
                Err(e) => tracing::warn!("Parakeet prewarm failed: {e}"),
            })
            .ok();
    }

    /// Spawn the idle watcher that unloads the model after inactivity to free RAM.
    /// Idempotent — only the first call spawns a thread.
    pub fn spawn_idle_watcher() {
        if WATCHER_STARTED.set(()).is_err() {
            return; // already running
        }
        std::thread::Builder::new()
            .name("parakeet-idle-watcher".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    if !Self::is_loaded() {
                        continue;
                    }
                    let idle_ms = now_ms().saturating_sub(LAST_USE_MS.load(Ordering::Relaxed));
                    if idle_ms > IDLE_UNLOAD_SECS * 1000 {
                        tracing::info!(idle_s = idle_ms / 1000, "Parakeet idle — freeing RAM");
                        Self::unload();
                    }
                }
            })
            .ok();
    }
}

#[async_trait]
impl Transcriber for LocalParakeet {
    async fn transcribe(&self, samples: &[f32], _language: &str) -> Result<String> {
        // Parakeet V3 auto-detects the language (no explicit selection), so the
        // `language` argument is intentionally ignored.
        if samples.is_empty() {
            return Ok(String::new());
        }
        LAST_USE_MS.store(now_ms(), Ordering::Relaxed);
        let samples = samples.to_vec();
        let model_dir = self.model_dir.clone();

        let text = tokio::task::spawn_blocking(move || -> Result<String> {
            LocalParakeet::ensure_loaded(&model_dir)?;
            let cell = model_cell();
            let mut guard = cell
                .lock()
                .map_err(|_| AppError::Stt("Parakeet model lock poisoned".into()))?;
            let model = guard
                .as_mut()
                .ok_or_else(|| AppError::Stt("Parakeet model not loaded".into()))?;

            let st = std::time::Instant::now();
            // Guard against an engine panic: rather than poisoning the lock for the
            // whole session, we drop the model so the next dictation reloads it.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                model.transcribe_with(&samples, &ParakeetParams::default())
            }));

            match outcome {
                Ok(Ok(result)) => {
                    tracing::info!(ms = st.elapsed().as_millis(), "Parakeet inference done");
                    Ok(result.text)
                }
                Ok(Err(e)) => Err(AppError::Stt(format!("Parakeet transcribe: {e}"))),
                Err(_panic) => {
                    *guard = None; // model is in an unknown state — drop it
                    tracing::error!("Parakeet inference panicked — model unloaded");
                    Err(AppError::Stt(
                        "Parakeet inference panicked — modèle déchargé, réessaie.".into(),
                    ))
                }
            }
        })
        .await
        .map_err(|e| AppError::Stt(format!("spawn_blocking join: {e}")))??;

        LAST_USE_MS.store(now_ms(), Ordering::Relaxed);
        Ok(text.trim().to_string())
    }
}
