use tauri::{AppHandle, State};

use crate::{
    error::Result,
    pipeline,
    state::{AppState, RecordingState},
};

#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>, app: AppHandle) -> Result<()> {
    pipeline::start_recording(&state, &app).await
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>, app: AppHandle) -> Result<()> {
    pipeline::stop_recording(&state, &app).await
}

#[tauri::command]
pub async fn get_recording_state(state: State<'_, AppState>) -> Result<RecordingState> {
    let guard = state.lock().await;
    Ok(guard.recording_state.clone())
}
