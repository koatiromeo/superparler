/// macOS text injection via enigo (Cmd+V simulation).
///
/// ─── PERMISSION REQUIRED ───────────────────────────────────────────────────
/// SuperParler needs the Accessibility permission to send synthetic key events.
///
/// Grant it once:
///   System Settings → Privacy & Security → Accessibility → enable SuperParler
///
/// Without it, enigo calls succeed (no error returned by macOS) but keystrokes
/// are silently dropped — the paste never arrives in the target app.
///
/// Detection: enigo 0.2 does not expose a runtime check for Accessibility.
/// If injection appears to do nothing on macOS, direct the user to the permission.
/// ───────────────────────────────────────────────────────────────────────────
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

/// Simulate Cmd+V in the active window.
pub fn paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
        AppError::Inject(format!(
            "enigo init (macOS): {e}\n\
             → Check: System Settings → Privacy → Accessibility → SuperParler ✓"
        ))
    })?;

    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|e| AppError::Inject(format!("Cmd press: {e}")))?;

    std::thread::sleep(std::time::Duration::from_millis(10));

    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| AppError::Inject(format!("V click: {e}")))?;

    enigo
        .key(Key::Meta, Direction::Release)
        .map_err(|e| AppError::Inject(format!("Cmd release: {e}")))?;

    Ok(())
}
