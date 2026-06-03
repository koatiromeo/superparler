/// hotkey/mod.rs — global shortcut registration and dispatch.
///
/// Two modes (from AppConfig):
///   PushToTalk — key DOWN → start, key UP → stop (walkie-talkie style)
///   Toggle     — key DOWN → start if idle / stop if recording
///
/// The callback is sync (OS constraint). We NEVER block_on or read AppState sync
/// inside it — instead we spawn a tokio task that reads state async, which is
/// the safe way to bridge sync OS callbacks into the async pipeline.
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    config::RecordingMode,
    error::{AppError, Result},
    state::{AppState, RecordingState},
};

/// Register `hotkey` string (Tauri accelerator format, e.g. "CmdOrCtrl+Shift+Space").
/// Unregisters any previously registered shortcut first.
pub async fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<()> {
    // Unregister previous binding (no-op if nothing was registered)
    let _ = app.global_shortcut().unregister_all();

    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|_| AppError::InvalidHotkey(hotkey.to_string()))?;

    let hotkey_str = hotkey.to_string();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app_handle, _shortcut, event| {
            dispatch(app_handle, event.state(), &hotkey_str);
        })
        .map_err(|e| AppError::HotkeyRegister(e.to_string()))?;

    tracing::info!(hotkey, "global hotkey registered");
    Ok(())
}

/// Re-register with a new hotkey string (called from settings update).
pub async fn reregister(app: &AppHandle, new_hotkey: &str) -> Result<()> {
    tracing::info!(new_hotkey, "re-registering global hotkey");
    register_hotkey(app, new_hotkey).await
}

pub fn unregister_all(app: &AppHandle) -> Result<()> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| AppError::HotkeyRegister(format!("unregister_all: {e}")))?;
    tracing::info!("all hotkeys unregistered");
    Ok(())
}

/// Sync dispatch function — called directly from the OS shortcut callback.
///
/// Rule: spawn a tokio task immediately and return. Never block, never lock,
/// never await. The spawned task owns all async state access.
fn dispatch(app: &AppHandle, event_state: ShortcutState, hotkey: &str) {
    let app = app.clone();
    let hotkey = hotkey.to_string();

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();

        // Read current recording_state + mode atomically inside the async task.
        // This is safe: tokio::sync::Mutex is used, no block_on needed.
        let (recording_state, mode) = {
            let guard = state.lock().await;
            (guard.recording_state.clone(), guard.config.mode.clone())
        };

        match event_state {
            ShortcutState::Pressed => {
                handle_press(&state, &app, recording_state, mode, &hotkey).await;
            }
            ShortcutState::Released => {
                handle_release(&state, &app, recording_state, mode, &hotkey).await;
            }
        }
    });
}

/// Handle key-down event.
async fn handle_press(
    state: &AppState,
    app: &AppHandle,
    recording_state: RecordingState,
    mode: RecordingMode,
    hotkey: &str,
) {
    match recording_state {
        RecordingState::Idle => {
            tracing::info!(hotkey, ?mode, "hotkey pressed → start recording");
            if let Err(e) = crate::pipeline::start_recording(state, app).await {
                tracing::error!(hotkey, "hotkey start_recording failed: {e}");
            }
        }
        RecordingState::Recording => match mode {
            RecordingMode::Toggle => {
                tracing::info!(hotkey, "hotkey pressed → stop recording (toggle: second press)");
                if let Err(e) = crate::pipeline::stop_recording(state, app).await {
                    tracing::error!(hotkey, "hotkey toggle stop failed: {e}");
                }
            }
            RecordingMode::PushToTalk => {
                // In PushToTalk, a second keydown while recording means the key is
                // being held and the OS is firing key-repeat events — ignore them.
                tracing::debug!(hotkey, "hotkey key-repeat while recording (push-to-talk) — ignored");
            }
        },
        RecordingState::Transcribing => {
            tracing::debug!(
                hotkey,
                "hotkey pressed while transcribing — ignored (transcription in progress)"
            );
        }
    }
}

/// Handle key-up event.
async fn handle_release(
    state: &AppState,
    app: &AppHandle,
    recording_state: RecordingState,
    mode: RecordingMode,
    hotkey: &str,
) {
    match mode {
        RecordingMode::PushToTalk => {
            if recording_state == RecordingState::Recording {
                tracing::info!(hotkey, "hotkey released → stop recording (push-to-talk)");
                if let Err(e) = crate::pipeline::stop_recording(state, app).await {
                    tracing::error!(hotkey, "hotkey push-to-talk stop failed: {e}");
                }
            } else {
                tracing::debug!(
                    hotkey,
                    ?recording_state,
                    "hotkey released but not recording (push-to-talk) — ignored"
                );
            }
        }
        RecordingMode::Toggle => {
            // Toggle mode: key release is not meaningful (stop happens on press)
            tracing::debug!(hotkey, "hotkey released (toggle mode) — no action on release");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_hotkey_strings_parse() {
        // Tauri accelerator format — verify common hotkeys parse without error
        let valid = [
            "CmdOrCtrl+Shift+Space",
            "Alt+F4",
            "Ctrl+Shift+D",
            "Super+Space",
            "F13",
        ];
        for hk in &valid {
            let result: std::result::Result<Shortcut, _> = hk.parse();
            assert!(result.is_ok(), "should parse: {hk} — got: {:?}", result.err());
        }
    }

    #[test]
    fn test_invalid_hotkey_string_fails() {
        // Garbage strings must not panic — they should return a parse error
        let invalid = ["", "not-a-key", "Ctrl+", "⌘⇧Space"];
        for hk in &invalid {
            let result: std::result::Result<Shortcut, _> = hk.parse();
            // We only test that they don't panic; some may succeed on some platforms
            let _ = result;
        }
    }
}
