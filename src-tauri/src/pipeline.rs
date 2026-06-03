/// pipeline.rs — single orchestration point for dictation.
/// Flow: capture → resample → VAD → STT → enhance? → inject → persist → emit
use tauri::{AppHandle, Emitter};

use crate::{
    audio::capture::AudioRecorder,
    enhance,
    error::{AppError, Result},
    events::{self, RecordingErrorPayload, RecordingResultPayload},
    inject,
    state::{AppState, RecordingState},
    storage,
    tray,
};

/// Begin audio capture. Emits "recording:started".
pub async fn start_recording(state: &AppState, app: &AppHandle) -> Result<()> {
    {
        let mut guard = state.lock().await;
        if guard.recording_state != RecordingState::Idle {
            return Err(AppError::AlreadyRecording);
        }
        guard.recording_state = RecordingState::Recording;
    }

    // AudioRecorder::start() opens the cpal stream (fast, not CPU-bound).
    // It is now Send because the !Send cpal::Stream lives inside a std::thread.
    let recorder = AudioRecorder::start()?;

    // Notify frontend and update tray
    app.emit(events::EVT_RECORDING_STARTED, ())
        .map_err(|e| AppError::Emit(e.to_string()))?;
    tray::update_tray_state(app, &RecordingState::Recording);

    // Background tokio task: poll state for stop signal, then resample + transcribe.
    let state_clone = state.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        // Poll until state transitions away from Recording (stop signal from stop_recording())
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let guard = state_clone.lock().await;
            if guard.recording_state != RecordingState::Recording {
                break;
            }
        }

        // recorder.stop() blocks the std thread doing resampling (~100 ms).
        // Use spawn_blocking so we don't block the tokio runtime thread.
        // AudioRecorder is Send so this is safe.
        let result = tokio::task::spawn_blocking(move || recorder.stop())
            .await
            .map_err(|e| AppError::AudioStream(format!("join error: {e}")));

        match result {
            Ok(Ok(samples)) => {
                let mut guard = state_clone.lock().await;
                guard.audio_samples = Some(samples);
            }
            Ok(Err(e)) => tracing::error!("recorder.stop() failed: {e}"),
            Err(e) => tracing::error!("spawn_blocking join error: {e}"),
        }

        // Run STT → enhance → inject → persist
        if let Err(e) = run_transcription(&state_clone, &app_clone).await {
            tracing::error!("transcription pipeline failed: {e}");
            let _ = app_clone.emit(
                events::EVT_RECORDING_ERROR,
                RecordingErrorPayload { message: e.to_string() },
            );
        }
    });

    Ok(())
}

/// Signal stop (transitions state to Transcribing; background task detects this and calls stop()).
pub async fn stop_recording(state: &AppState, app: &AppHandle) -> Result<()> {
    {
        let mut guard = state.lock().await;
        if guard.recording_state != RecordingState::Recording {
            return Err(AppError::NotRecording);
        }
        guard.recording_state = RecordingState::Transcribing;
    }

    app.emit(events::EVT_RECORDING_STOPPED, ())
        .map_err(|e| AppError::Emit(e.to_string()))?;
    app.emit(events::EVT_RECORDING_TRANSCRIBING, ())
        .map_err(|e| AppError::Emit(e.to_string()))?;
    tray::update_tray_state(app, &RecordingState::Transcribing);

    Ok(())
}

/// VAD → STT → enhance → inject → persist → emit result.
async fn run_transcription(state: &AppState, app: &AppHandle) -> Result<()> {
    let (samples, config) = {
        let guard = state.lock().await;
        let samples = guard.audio_samples.clone().ok_or(AppError::NotRecording)?;
        (samples, guard.config.clone())
    };

    // samples are already VAD-trimmed by AudioRecorder::stop()
    if samples.is_empty() {
        tracing::warn!("pipeline: no speech in buffer after VAD — skipping STT");
        reset_state(state, app).await;
        return Ok(());
    }

    // STT
    let transcriber = crate::stt::factory::build_transcriber(&config)?;
    let text = transcriber.transcribe(&samples, &config.language).await?;
    if text.is_empty() {
        tracing::warn!("transcription returned empty text");
        reset_state(state, app).await;
        return Ok(());
    }

    // Optional LLM enhancement
    let enhancer = enhance::build_enhancer(config.enhance_enabled, &config.groq_model);
    let final_text = enhancer.enhance(&text, &config.enhance_prompt).await?;

    // Inject into active application
    inject::inject_text(&final_text, app).await?;

    // Persist
    let duration_ms = (samples.len() as f64 / 16_000.0 * 1000.0) as i64;
    let engine_name = format!("{:?}", config.engine).to_lowercase();
    if let Some(pool) = &state.lock().await.db_pool {
        storage::models::insert_transcription(
            pool,
            &final_text,
            duration_ms,
            &engine_name,
            &config.language,
            config.enhance_enabled,
        )
        .await?;
    }

    // Emit result to frontend
    app.emit(events::EVT_RECORDING_RESULT, RecordingResultPayload { text: final_text })
        .map_err(|e| AppError::Emit(e.to_string()))?;

    reset_state(state, app).await;
    Ok(())
}

async fn reset_state(state: &AppState, app: &AppHandle) {
    let mut guard = state.lock().await;
    guard.recording_state = RecordingState::Idle;
    guard.audio_samples = None;
    tray::update_tray_state(app, &RecordingState::Idle);
}
