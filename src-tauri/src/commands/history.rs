use tauri::State;

use crate::{
    error::Result,
    state::AppState,
    storage::models::{self, Transcription},
};

#[tauri::command]
pub async fn list_transcriptions(
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<Transcription>> {
    let guard = state.lock().await;
    let pool = guard.db_pool.as_ref().ok_or_else(|| {
        crate::error::AppError::Config("database not initialized".to_string())
    })?;
    models::list_transcriptions(pool, limit.unwrap_or(50), offset.unwrap_or(0)).await
}

#[tauri::command]
pub async fn delete_transcription(id: String, state: State<'_, AppState>) -> Result<()> {
    let guard = state.lock().await;
    let pool = guard.db_pool.as_ref().ok_or_else(|| {
        crate::error::AppError::Config("database not initialized".to_string())
    })?;
    models::delete_transcription(pool, &id).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<()> {
    let guard = state.lock().await;
    let pool = guard.db_pool.as_ref().ok_or_else(|| {
        crate::error::AppError::Config("database not initialized".to_string())
    })?;
    models::clear_all(pool).await
}
