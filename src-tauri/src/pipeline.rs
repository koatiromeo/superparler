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
    storage, tray,
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

    // AudioRecorder::start() opens the cpal stream; the !Send cpal::Stream
    // lives inside a dedicated std::thread, so AudioRecorder itself is Send.
    // Fix: reset state if start() fails so future recordings aren't locked out.
    let recorder = match AudioRecorder::start() {
        Ok(r) => r,
        Err(e) => {
            state.lock().await.recording_state = RecordingState::Idle;
            return Err(e);
        }
    };

    // Fix: reset state if emit fails. recorder drops here → audio thread exits.
    if let Err(e) = app.emit(events::EVT_RECORDING_STARTED, ()) {
        state.lock().await.recording_state = RecordingState::Idle;
        return Err(AppError::Emit(e.to_string()));
    }
    tray::update_tray_state(app, &RecordingState::Recording);

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

        // recorder.stop() joins the audio thread (sync, ~100 ms). spawn_blocking
        // prevents blocking the tokio runtime.
        let result = tokio::task::spawn_blocking(move || recorder.stop())
            .await
            .map_err(|e| AppError::AudioStream(format!("join error: {e}")));

        let samples = match result {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                tracing::error!("recorder.stop() failed: {e}");
                let _ = app_clone.emit(
                    events::EVT_RECORDING_ERROR,
                    RecordingErrorPayload {
                        message: e.to_string(),
                    },
                );
                reset_state(&state_clone, &app_clone).await;
                return;
            }
            Err(e) => {
                tracing::error!("spawn_blocking join error: {e}");
                let _ = app_clone.emit(
                    events::EVT_RECORDING_ERROR,
                    RecordingErrorPayload {
                        message: e.to_string(),
                    },
                );
                reset_state(&state_clone, &app_clone).await;
                return;
            }
        };

        {
            let mut guard = state_clone.lock().await;
            guard.audio_samples = Some(samples);
        }

        if let Err(e) = run_transcription(&state_clone, &app_clone).await {
            tracing::error!("transcription pipeline failed: {e}");
            let _ = app_clone.emit(
                events::EVT_RECORDING_ERROR,
                RecordingErrorPayload {
                    message: e.to_string(),
                },
            );
            reset_state(&state_clone, &app_clone).await;
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

    // Fix: always emit result so the frontend exits Transcribing state.
    if samples.is_empty() {
        tracing::warn!("pipeline: VAD found no speech — skipping STT");
        let _ = app.emit(
            events::EVT_RECORDING_RESULT,
            RecordingResultPayload {
                text: String::new(),
            },
        );
        reset_state(state, app).await;
        return Ok(());
    }

    // STT — local whisper runs spawn_blocking internally; Groq is async HTTP.
    let transcriber = crate::stt::factory::build_transcriber(&config)?;
    let text = transcriber.transcribe(&samples, &config.language).await?;

    if text.trim().is_empty() {
        tracing::warn!("STT returned empty text — skipping inject/persist");
        let _ = app.emit(
            events::EVT_RECORDING_RESULT,
            RecordingResultPayload {
                text: String::new(),
            },
        );
        reset_state(state, app).await;
        return Ok(());
    }

    // Optional LLM enhancement — fall back to raw text on error so the transcription
    // is never silently lost (e.g. missing API key, network failure).
    let (final_text, was_enhanced) = if config.enhance_enabled {
        let enhancer = enhance::build_enhancer(true, &config.enhance_model);
        match enhancer.enhance(&text, &config.enhance_prompt).await {
            Ok(enhanced) => (enhanced, true),
            Err(e) => {
                tracing::warn!(error = %e, "LLM enhance failed — injecting raw transcription");
                (text, false)
            }
        }
    } else {
        (text, false)
    };

    inject::inject_text(&final_text, app).await?;

    // Persist best-effort — a DB failure must not prevent emitting the result event
    // since the text has already been injected at the cursor.
    let pool_opt = state.lock().await.db_pool.clone();
    if let Some(pool) = pool_opt {
        let duration_ms = (samples.len() as f64 / 16_000.0 * 1000.0) as i64;
        let engine_name = config.engine.as_str();
        if let Err(e) = storage::models::insert_transcription(
            &pool,
            &final_text,
            duration_ms,
            engine_name,
            &config.language,
            was_enhanced,
        )
        .await
        {
            tracing::error!("persist failed (non-fatal, text already injected): {e}");
        } else {
            tracing::info!(duration_ms, engine = engine_name, "transcription persisted");
        }
    } else {
        tracing::warn!("DB pool not available — transcription not persisted");
    }

    app.emit(
        events::EVT_RECORDING_RESULT,
        RecordingResultPayload { text: final_text },
    )
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
