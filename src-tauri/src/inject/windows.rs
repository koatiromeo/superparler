// Windows text injection via enigo — no special permissions required.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

pub fn paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Inject(format!("enigo init (Windows): {e}")))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| AppError::Inject(format!("Ctrl press: {e}")))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| AppError::Inject(format!("V click: {e}")))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| AppError::Inject(format!("Ctrl release: {e}")))?;

    Ok(())
}
