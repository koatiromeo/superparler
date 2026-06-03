use tauri::State;

use crate::{
    error::{AppError, Result},
    state::AppState,
    storage::models::{self, Transcription},
};

fn db_not_init() -> AppError {
    AppError::Config("database not initialized".to_string())
}

#[tauri::command]
pub async fn list_transcriptions(
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<Transcription>> {
    let pool = state.lock().await.db_pool.clone().ok_or_else(db_not_init)?;
    models::list_transcriptions(&pool, limit.unwrap_or(50), offset.unwrap_or(0)).await
}

#[tauri::command]
pub async fn delete_transcription(id: String, state: State<'_, AppState>) -> Result<()> {
    let pool = state.lock().await.db_pool.clone().ok_or_else(db_not_init)?;
    models::delete_transcription(&pool, &id).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<()> {
    let pool = state.lock().await.db_pool.clone().ok_or_else(db_not_init)?;
    models::clear_all(&pool).await
}
