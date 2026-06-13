//! overlay.rs — SuperWhisper-style floating "pill" shown during dictation.
//!
//! Deliberately LAZY: the WebView2 window is created on the FIRST dictation,
//! never at startup — this preserves the app's zero-WebView2 instant launch
//! (the whole point of the tray-only refactor). Once created, the window is
//! kept hidden between sessions and reused, so every show after the first is
//! instant.
//!
//! Best-effort everywhere: failing to create or show the overlay must NEVER
//! break the dictation pipeline, so these helpers swallow errors (logged) and
//! return `()`. The pill is pure visual feedback — the tray icon remains the
//! source-of-truth status indicator.
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, Position, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::events::EVT_OVERLAY_STATE;

/// Window label — MUST match the `"overlay"` entry in `capabilities/default.json`
/// so the webview is granted `core:event` (listen) + cursor/position perms.
const OVERLAY_LABEL: &str = "overlay";
const OVERLAY_WIDTH: f64 = 360.0;
const OVERLAY_HEIGHT: f64 = 96.0;
/// Gap between the panel and the bottom edge of the screen (logical px).
const OVERLAY_BOTTOM_GAP: f64 = 56.0;

/// Show the pill in `state` ("recording" | "transcribing").
///
/// Creates the window lazily on first call. Everything runs on the main thread:
/// WebView window creation and z-order changes must not happen off it.
pub fn show(app: &AppHandle, state: &str) {
    let handle = app.clone();
    let state = state.to_string();
    let dispatched = app.run_on_main_thread(move || {
        let window = match ensure_window(&handle) {
            Some(w) => w,
            None => return,
        };
        position_bottom_center(&handle, &window);
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        // Click-through: the pill must never intercept the user's clicks.
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.emit(EVT_OVERLAY_STATE, &state);
    });
    if let Err(e) = dispatched {
        tracing::warn!("overlay show dispatch failed: {e}");
    }
}

/// Hide the pill: ask the webview to fade out, then hide the native window.
pub fn hide(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window(OVERLAY_LABEL) {
            let _ = window.emit(EVT_OVERLAY_STATE, "hide");
            // Let the CSS fade-out play before hiding the native window.
            let w = window.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(220));
                let _ = w.hide();
            });
        }
    });
}

/// Return the existing overlay window, creating it (hidden) on first use.
fn ensure_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        return Some(window);
    }
    match WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("SuperParler")
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focused(false)
        .visible(false)
        .build()
    {
        Ok(window) => {
            tracing::info!("overlay window created (lazy, first dictation)");
            Some(window)
        }
        Err(e) => {
            tracing::warn!("overlay window creation failed: {e}");
            None
        }
    }
}

/// Center the pill horizontally, near the bottom of the primary monitor.
///
/// Monitor geometry is physical pixels; we divide by the scale factor to get
/// logical coordinates, then set a `LogicalPosition` so the result is correct
/// regardless of the monitor's DPI.
fn position_bottom_center(app: &AppHandle, window: &WebviewWindow) {
    let monitor = match app.primary_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };
    let scale = monitor.scale_factor();
    let mon_x = monitor.position().x as f64 / scale;
    let mon_y = monitor.position().y as f64 / scale;
    let mon_w = monitor.size().width as f64 / scale;
    let mon_h = monitor.size().height as f64 / scale;

    let x = mon_x + (mon_w - OVERLAY_WIDTH) / 2.0;
    let y = mon_y + mon_h - OVERLAY_HEIGHT - OVERLAY_BOTTOM_GAP;
    let _ = window.set_position(Position::Logical(LogicalPosition { x, y }));
}
