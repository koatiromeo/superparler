// macOS text injection via enigo.
// IMPORTANT: requires Accessibility permission in System Settings → Privacy & Security → Accessibility.
// SuperParler must be listed and toggled ON. Without it, enigo key events are silently dropped.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

pub fn paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Inject(format!("enigo init (macOS): {e} — check Accessibility permission")))?;

    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|e| AppError::Inject(format!("Cmd press: {e}")))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| AppError::Inject(format!("V click: {e}")))?;
    enigo
        .key(Key::Meta, Direction::Release)
        .map_err(|e| AppError::Inject(format!("Cmd release: {e}")))?;

    Ok(())
}
