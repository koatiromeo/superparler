use serde::Serialize;
use tauri::State;

use crate::{
    error::Result,
    state::AppState,
    stt::factory,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub engine: String,
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub async fn test_engine(state: State<'_, AppState>) -> Result<EngineStatus> {
    let config = state.lock().await.config.clone();
    let engine_name = config.engine.as_str().to_string();

    match factory::build_transcriber(&config) {
        Ok(_) => Ok(EngineStatus {
            engine: engine_name,
            ok: true,
            message: "Engine ready".to_string(),
        }),
        Err(e) => Ok(EngineStatus {
            engine: engine_name,
            ok: false,
            message: e.to_string(),
        }),
    }
}

#[tauri::command]
pub async fn set_groq_key(key: String, _state: State<'_, AppState>) -> Result<()> {
    if key.trim().is_empty() {
        return Err(crate::error::AppError::Keyring("API key must not be empty".to_string()));
    }
    let entry =
        keyring::Entry::new("superparler", "groq").map_err(|e| crate::error::AppError::Keyring(e.to_string()))?;
    entry
        .set_password(key.trim())
        .map_err(|e| crate::error::AppError::Keyring(format!("keyring write: {e}")))?;
    tracing::info!("Groq API key saved to OS keyring");
    Ok(())
}
