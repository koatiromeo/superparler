use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{error::Result, state::AppState};

#[tauri::command]
pub async fn set_launch_at_startup(enabled: bool, app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    } else {
        autostart.disable().map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    }

    let mut guard = state.lock().await;
    guard.config.launch_at_startup = enabled;
    guard.config.save(&app)?;

    tracing::info!(enabled, "launch at startup updated");
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| crate::error::AppError::Config(e.to_string()))?;
        window.set_focus().map_err(|e| crate::error::AppError::Config(e.to_string()))?;
    }
    Ok(())
}
