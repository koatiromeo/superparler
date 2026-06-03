use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::{AppError, Result};
use crate::state::{AppState, RecordingState};

pub async fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<()> {
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|_| AppError::InvalidHotkey(hotkey.to_string()))?;

    let _app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app_handle, _shortcut, event| {
            let state = app_handle.state::<AppState>();
            let config = {
                // SAFETY: brief sync lock in callback — no await, just a clone
                let guard = tokio::runtime::Handle::current()
                    .block_on(async { state.lock().await });
                (guard.recording_state.clone(), guard.config.mode.clone())
            };

            let (recording_state, mode) = config;
            let app_handle = app_handle.clone();

            match event.state() {
                ShortcutState::Pressed => {
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        if recording_state == RecordingState::Idle {
                            if let Err(e) = crate::pipeline::start_recording(&state, &app_handle).await {
                                tracing::error!("hotkey start_recording failed: {e}");
                            }
                        } else if recording_state == RecordingState::Recording {
                            // Toggle mode: second press stops
                            if matches!(mode, crate::config::RecordingMode::Toggle) {
                                if let Err(e) = crate::pipeline::stop_recording(&state, &app_handle).await {
                                    tracing::error!("hotkey stop_recording (toggle) failed: {e}");
                                }
                            }
                        }
                    });
                }
                ShortcutState::Released => {
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        // PushToTalk mode: release key → stop
                        if matches!(mode, crate::config::RecordingMode::PushToTalk)
                            && recording_state == RecordingState::Recording
                        {
                            if let Err(e) = crate::pipeline::stop_recording(&state, &app_handle).await {
                                tracing::error!("hotkey stop_recording (push-to-talk) failed: {e}");
                            }
                        }
                    });
                }
            }
        })
        .map_err(|e| AppError::HotkeyRegister(e.to_string()))?;

    tracing::info!(hotkey = hotkey, "global hotkey registered");
    Ok(())
}

pub fn unregister_all(app: &AppHandle) -> Result<()> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| AppError::HotkeyRegister(format!("unregister_all: {e}")))?;
    tracing::info!("all hotkeys unregistered");
    Ok(())
}
