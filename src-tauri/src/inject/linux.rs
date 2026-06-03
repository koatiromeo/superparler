/// Linux text injection.
///
/// X11 (DISPLAY set):  enigo sends Ctrl+V via X11 protocol.
/// Wayland (no DISPLAY or WAYLAND_DISPLAY set): enigo may fail depending on
///   the compositor. Fallback: ydotool (requires `sudo ydotoold &` running).
///
/// ─── WAYLAND SETUP ─────────────────────────────────────────────────────────
/// Install ydotool and start the daemon once per session:
///   sudo apt install ydotool     # or: cargo install ydotool
///   sudo ydotoold &              # run as background daemon
///
/// Verify: ydotool key 29:1 47:1 47:0 29:0   (should paste whatever is in clipboard)
/// ───────────────────────────────────────────────────────────────────────────
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{AppError, Result};

/// Simulate Ctrl+V. Tries enigo first, falls back to ydotool.
pub fn paste() -> Result<()> {
    match try_enigo_paste() {
        Ok(()) => Ok(()),
        Err(enigo_err) => {
            let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
                && std::env::var("DISPLAY").is_err();

            if is_wayland {
                tracing::warn!(
                    "Wayland detected, enigo failed ({enigo_err}); trying ydotool"
                );
            } else {
                tracing::warn!(
                    "enigo paste failed ({enigo_err}); trying ydotool fallback"
                );
            }

            try_ydotool_paste().map_err(|ydotool_err| {
                AppError::Inject(format!(
                    "Both injection methods failed.\n\
                     enigo: {enigo_err}\n\
                     ydotool: {ydotool_err}\n\
                     On Wayland: install ydotool and run `sudo ydotoold &`"
                ))
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

    std::thread::sleep(std::time::Duration::from_millis(10));

    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| AppError::Inject(format!("V click: {e}")))?;

    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| AppError::Inject(format!("Ctrl release: {e}")))?;

    Ok(())
}

fn try_ydotool_paste() -> Result<()> {
    // Key codes (Linux evdev): 29=KEY_LEFTCTRL, 47=KEY_V
    // Format: <keycode>:<down=1|up=0>
    let status = std::process::Command::new("ydotool")
        .args(["key", "29:1", "47:1", "47:0", "29:0"])
        .status()
        .map_err(|e| {
            AppError::Inject(format!(
                "ydotool not found or failed to execute: {e}\n\
                 Install: sudo apt install ydotool && sudo ydotoold &"
            ))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::Inject(format!(
            "ydotool exited with {}",
            status.code().map_or("signal".to_string(), |c| c.to_string())
        )))
    }
}
