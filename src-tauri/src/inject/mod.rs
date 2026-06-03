pub mod linux;
pub mod macos;
pub mod windows;

use crate::error::{AppError, Result};

pub trait TextInjector: Send + Sync {
    fn paste(&self) -> Result<()>;
}

/// Inject text at the cursor position of the currently active application.
/// Strategy: set clipboard → simulate paste shortcut (Cmd/Ctrl+V).
pub async fn inject_text(text: &str, app: &tauri::AppHandle) -> Result<()> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    // Step 1: write to clipboard
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| AppError::Inject(format!("clipboard write: {e}")))?;

    // Brief yield to ensure clipboard is set before paste simulation
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Step 2: simulate paste
    #[cfg(target_os = "macos")]
    macos::paste()?;
    #[cfg(target_os = "windows")]
    windows::paste()?;
    #[cfg(target_os = "linux")]
    linux::paste()?;

    tracing::info!(text_len = text.len(), "text injected via clipboard paste");
    Ok(())
}
