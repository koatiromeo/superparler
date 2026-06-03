use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    config::{AppConfig, Engine, InjectMethod, RecordingMode},
    error::Result,
    hotkey,
    state::AppState,
};

/// Partial config patch — all fields optional for granular updates.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialConfig {
    pub engine: Option<Engine>,
    pub local_model_path: Option<String>,
    pub groq_model: Option<String>,
    pub language: Option<String>,
    pub hotkey: Option<String>,
    pub mode: Option<RecordingMode>,
    pub inject_method: Option<InjectMethod>,
    pub enhance_enabled: Option<bool>,
    pub enhance_prompt: Option<String>,
    pub launch_at_startup: Option<bool>,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppConfig> {
    let guard = state.lock().await;
    Ok(guard.config.clone())
}

#[tauri::command]
pub async fn update_settings(
    patch: PartialConfig,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AppConfig> {
    let mut guard = state.lock().await;
    let config = &mut guard.config;

    let hotkey_changed = patch.hotkey.as_deref().map(|h| h != config.hotkey).unwrap_or(false);

    if let Some(v) = patch.engine           { config.engine = v; }
    if let Some(v) = patch.local_model_path { config.local_model_path = v; }
    if let Some(v) = patch.groq_model       { config.groq_model = v; }
    if let Some(v) = patch.language         { config.language = v; }
    if let Some(v) = patch.hotkey           { config.hotkey = v; }
    if let Some(v) = patch.mode             { config.mode = v; }
    if let Some(v) = patch.inject_method    { config.inject_method = v; }
    if let Some(v) = patch.enhance_enabled  { config.enhance_enabled = v; }
    if let Some(v) = patch.enhance_prompt   { config.enhance_prompt = v; }
    if let Some(v) = patch.launch_at_startup { config.launch_at_startup = v; }

    config.save(&app)?;
    let new_config = config.clone();
    let new_hotkey = new_config.hotkey.clone();
    drop(guard);

    // Re-register hotkey if it changed
    if hotkey_changed {
        hotkey::unregister_all(&app)?;
        hotkey::register_hotkey(&app, &new_hotkey).await?;
    }

    tracing::info!("settings updated and saved");
    Ok(new_config)
}
