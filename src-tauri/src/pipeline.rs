/// pipeline.rs — single orchestration point for dictation.
/// Flow: capture → resample → VAD → STT → enhance? → inject → persist → emit
use tauri::{AppHandle, Manager};

use crate::{
    audio::{capture::AudioRecorder, resample, vad},
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
    let mut guard = state.lock().await;

    if guard.recording_state != RecordingState::Idle {
        return Err(AppError::AlreadyRecording);
    }

    // Start the audio stream in a blocking thread; store samples in state
    // AudioRecorder::start() is sync (cpal), wrap in spawn_blocking
    let recorder = tokio::task::spawn_blocking(AudioRecorder::start)
        .await
        .map_err(|e| AppError::AudioStream(format!("spawn_blocking: {e}")))??;

    guard.recording_state = RecordingState::Recording;

    // Store recorder handle — we need it on stop
    // We serialize by storing samples in state instead of the recorder
    // The recorder is moved into a background task that drains on stop signal
    drop(guard);

    // Notify frontend and update tray
    app.emit(events::EVT_RECORDING_STARTED, ())
        .map_err(|e| AppError::Emit(e.to_string()))?;
    tray::update_tray_state(app, &RecordingState::Recording);

    // Spawn background task to hold the recorder until stop is called
    let state_clone = state.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        // Wait for recording state to change (stop signal)
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let guard = state_clone.lock().await;
            if guard.recording_state != RecordingState::Recording {
                break;
            }
        }

        // Stop the recorder and get samples
        let result = tokio::task::spawn_blocking(move || recorder.stop())
            .await
            .map_err(|e| AppError::AudioStream(e.to_string()));

        match result {
            Ok(Ok((samples, source_rate, channels))) => {
                let mut guard = state_clone.lock().await;
                // Store resampled samples
                match resample::to_16khz_mono(&samples, source_rate, channels) {
                    Ok(mono) => guard.audio_samples = Some(mono),
                    Err(e) => {
                        tracing::error!("resample failed: {e}");
                        guard.audio_samples = None;
                    }
                }
            }
            Ok(Err(e)) => tracing::error!("recorder.stop() failed: {e}"),
            Err(e) => tracing::error!("spawn_blocking join failed: {e}"),
        }

        // Trigger transcription
        let state_for_transcribe = state_clone.clone();
        let app_for_transcribe = app_clone.clone();
        if let Err(e) = run_transcription(&state_for_transcribe, &app_for_transcribe).await {
            tracing::error!("transcription pipeline failed: {e}");
            let _ = app_clone.emit(
                events::EVT_RECORDING_ERROR,
                RecordingErrorPayload { message: e.to_string() },
            );
        }
    });

    Ok(())
}

/// Signal stop and begin transcription. Emits "recording:stopped" then "recording:transcribing".
pub async fn stop_recording(state: &AppState, app: &AppHandle) -> Result<()> {
    let mut guard = state.lock().await;
    if guard.recording_state != RecordingState::Recording {
        return Err(AppError::NotRecording);
    }
    guard.recording_state = RecordingState::Transcribing;
    drop(guard);

    app.emit(events::EVT_RECORDING_STOPPED, ())
        .map_err(|e| AppError::Emit(e.to_string()))?;
    app.emit(events::EVT_RECORDING_TRANSCRIBING, ())
        .map_err(|e| AppError::Emit(e.to_string()))?;
    tray::update_tray_state(app, &RecordingState::Transcribing);

    Ok(())
}

/// Run STT → enhance → inject → persist → emit result. Called after samples are ready.
async fn run_transcription(state: &AppState, app: &AppHandle) -> Result<()> {
    let (samples, config) = {
        let guard = state.lock().await;
        let samples = guard
            .audio_samples
            .clone()
            .ok_or(AppError::NotRecording)?;
        (samples, guard.config.clone())
    };

    // VAD trim silence
    let trimmed = vad::trim_silence(&samples)?;
    if trimmed.is_empty() {
        tracing::warn!("VAD: no speech detected, skipping transcription");
        let mut guard = state.lock().await;
        guard.recording_state = RecordingState::Idle;
        guard.audio_samples = None;
        tray::update_tray_state(app, &RecordingState::Idle);
        return Ok(());
    }

    // Build transcriber and run
    let transcriber = crate::stt::factory::build_transcriber(&config)?;
    let text = transcriber.transcribe(&trimmed, &config.language).await?;

    if text.is_empty() {
        tracing::warn!("transcription returned empty text");
        let mut guard = state.lock().await;
        guard.recording_state = RecordingState::Idle;
        guard.audio_samples = None;
        tray::update_tray_state(app, &RecordingState::Idle);
        return Ok(());
    }

    // Optional enhancement
    let enhancer = enhance::build_enhancer(config.enhance_enabled, &config.groq_model);
    let final_text = enhancer.enhance(&text, &config.enhance_prompt).await?;

    // Inject into active application
    inject::inject_text(&final_text, app).await?;

    // Persist to SQLite
    let duration_ms = (trimmed.len() as f64 / 16_000.0 * 1000.0) as i64;
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

    // Reset state
    let mut guard = state.lock().await;
    guard.recording_state = RecordingState::Idle;
    guard.audio_samples = None;
    tray::update_tray_state(app, &RecordingState::Idle);

    Ok(())
}
