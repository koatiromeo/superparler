/// Windows text injection — no special permissions required.
///
/// Uses enigo to send Ctrl+V to the currently focused window.
/// A small sleep between key Press and Release improves reliability
/// with apps that debounce rapid key events (e.g. some Electron apps).
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

/// Simulate Ctrl+V in the active window.
pub fn paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Inject(format!("enigo init (Windows): {e}")))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| AppError::Inject(format!("Ctrl press: {e}")))?;

    // 10 ms hold — some Electron/WPF apps need a brief key-down before releasing
    std::thread::sleep(std::time::Duration::from_millis(10));

    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| AppError::Inject(format!("V click: {e}")))?;

    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| AppError::Inject(format!("Ctrl release: {e}")))?;

    Ok(())
}
