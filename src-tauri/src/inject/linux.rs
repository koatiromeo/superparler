// Linux text injection via enigo — works on X11.
// Wayland: enigo may fail depending on compositor.
// Fallback: ydotool (requires running `sudo ydotoold &` daemon).

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

pub fn paste() -> Result<()> {
    // Try enigo first (works on X11, may work on XWayland)
    match try_enigo_paste() {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::warn!("enigo paste failed on Linux ({e}); trying ydotool fallback");
            try_ydotool_paste().map_err(|_| {
                AppError::Inject(
                    "Injection failed on Linux. On Wayland, install ydotool and run: sudo ydotoold &".to_string(),
                )
            })
        }
    }
}

fn try_enigo_paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| AppError::Inject(format!("enigo init (Linux): {e}")))?;

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

fn try_ydotool_paste() -> Result<()> {
    // ydotool key Ctrl+V (key codes: 29=Ctrl, 47=V)
    let status = std::process::Command::new("ydotool")
        .args(["key", "29:1", "47:1", "47:0", "29:0"])
        .status()
        .map_err(|e| AppError::Inject(format!("ydotool exec: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::Inject("ydotool exited with non-zero status".to_string()))
    }
}
