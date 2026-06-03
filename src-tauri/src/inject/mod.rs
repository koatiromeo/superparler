/// inject/mod.rs — cross-platform text injection
///
/// Strategy: save clipboard → write text → simulate Ctrl/Cmd+V → restore clipboard.
///
/// Platform notes:
///   macOS   — requires Accessibility permission (System Settings → Privacy → Accessibility).
///             Without it, enigo key events are silently dropped.
///   Windows — no special permission needed.
///   Linux   — X11: works via enigo. Wayland: enigo may fail; fallback to ydotool
///             (`sudo ydotoold &` must be running beforehand).
pub mod linux;
pub mod macos;
pub mod windows;

use crate::error::{AppError, Result};

/// Timing constants (ms). Tuned to avoid race conditions with slow apps (Word, Electron).
/// Before paste: clipboard write must propagate to the OS before we send Ctrl+V.
const DELAY_BEFORE_PASTE_MS: u64 = 80;
/// Before restore: target app must have processed the paste event before we overwrite clipboard.
const DELAY_BEFORE_RESTORE_MS: u64 = 250;

/// Core paste-simulation abstraction.
/// Each platform impl sends the modifier+V keystrokes to the currently focused window.
pub trait TextInjector: Send + Sync {
    /// Simulate Ctrl+V (or Cmd+V on macOS) in the active window.
    /// Assumes the text is already in the clipboard.
    fn paste(&self) -> Result<()>;
    fn name(&self) -> &'static str;
}

/// Inject `text` at the cursor position of the currently focused application.
///
/// Full flow:
///   1. Save current clipboard text (best-effort — never fails the injection)
///   2. Write `text` to clipboard
///   3. Wait `DELAY_BEFORE_PASTE_MS`
///   4. Simulate paste shortcut via platform-specific key events
///   5. Wait `DELAY_BEFORE_RESTORE_MS`
///   6. Restore previous clipboard content (best-effort)
pub async fn inject_text(text: &str, app: &tauri::AppHandle) -> Result<()> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    if text.is_empty() {
        tracing::debug!("inject_text: empty text, nothing to do");
        return Ok(());
    }

    // ── 1. Save previous clipboard (best-effort: ignore errors) ────────────
    let previous = app.clipboard().read_text().ok();
    if previous.is_some() {
        tracing::debug!("inject: saved previous clipboard content");
    }

    // ── 2. Write our text to clipboard ──────────────────────────────────────
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| AppError::Inject(format!("clipboard write: {e}")))?;

    // ── 3. Let the OS propagate the clipboard update ─────────────────────────
    tokio::time::sleep(std::time::Duration::from_millis(DELAY_BEFORE_PASTE_MS)).await;

    // ── 4. Simulate paste shortcut ───────────────────────────────────────────
    #[cfg(target_os = "macos")]
    macos::paste()?;
    #[cfg(target_os = "windows")]
    windows::paste()?;
    #[cfg(target_os = "linux")]
    linux::paste()?;

    tracing::info!(
        text_len = text.len(),
        delay_before_ms = DELAY_BEFORE_PASTE_MS,
        "text injected via clipboard paste"
    );

    // ── 5. Wait for target app to consume the paste event ────────────────────
    tokio::time::sleep(std::time::Duration::from_millis(DELAY_BEFORE_RESTORE_MS)).await;

    // ── 6. Restore previous clipboard (best-effort) ──────────────────────────
    match previous {
        Some(prev) => {
            if let Err(e) = app.clipboard().write_text(prev) {
                // Non-fatal: clipboard restore failed, but injection succeeded
                tracing::warn!("clipboard restore failed (injection still succeeded): {e}");
            } else {
                tracing::debug!("previous clipboard content restored");
            }
        }
        None => {
            // Clipboard was empty or non-text — nothing to restore.
            // We intentionally leave our transcription text there so the user can
            // still paste it manually if the automatic injection failed.
        }
    }

    Ok(())
}
