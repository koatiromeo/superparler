use serde::Serialize;
use tauri::State;

use crate::{
    error::{AppError, Result},
    stt::factory,
    state::AppState,
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
    let engine_name = format!("{:?}", config.engine).to_lowercase();

    // Try to build the transcriber — this validates model path (local) or API key (groq)
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
pub async fn list_local_models(_state: State<'_, AppState>) -> Result<Vec<String>> {
    // Scan the models/ directory for .gguf and .bin files
    let models_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("../../models")))
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("models"));

    if !models_dir.exists() {
        return Ok(Vec::new());
    }

    let mut models = Vec::new();
    let mut entries = tokio::fs::read_dir(&models_dir)
        .await
        .map_err(|e| AppError::Config(format!("read models dir: {e}")))?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "gguf" || ext == "bin" {
                models.push(path.to_string_lossy().to_string());
            }
        }
    }

    Ok(models)
}

#[tauri::command]
pub async fn set_groq_key(key: String, _state: State<'_, AppState>) -> Result<()> {
    if key.trim().is_empty() {
        return Err(AppError::Keyring("API key must not be empty".to_string()));
    }
    // service="superparler", account="groq" — must match stt/groq.rs KEYRING_* constants
    let entry = keyring::Entry::new("superparler", "groq")
        .map_err(|e| AppError::Keyring(e.to_string()))?;
    entry
        .set_password(key.trim())
        .map_err(|e| AppError::Keyring(format!("keyring write: {e}")))?;
    tracing::info!("Groq API key saved to OS keyring");
    Ok(())
}
