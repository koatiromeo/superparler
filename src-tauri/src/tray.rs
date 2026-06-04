use tauri::{
    App, AppHandle, Manager,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    config::AppConfig,
    state::{AppState, RecordingState},
};

/// Managed state: dynamic menu items and cached idle icon for runtime updates.
/// MenuItem<Wry> and Image<'static> are Send + Sync.
pub struct TrayItems {
    pub toggle: MenuItem<tauri::Wry>,
    pub idle_icon: Image<'static>,
}

/// Build the tray icon and menu. Takes `config` to display the current hotkey
/// and engine. Must run inside Tauri's `setup` closure.
pub fn setup_tray(app: &mut App, config: &AppConfig) -> tauri::Result<()> {
    let hotkey_label = format_hotkey(cfg!(target_os = "macos"), &config.hotkey);
    let toggle_text = format!("Démarrer la dictée  {hotkey_label}");
    let toggle = MenuItem::with_id(app, "toggle", &toggle_text, true, None::<&str>)?;

    let hotkey_info = MenuItem::with_id(app, "hotkey_info", &hotkey_label, false, None::<&str>)?;
    let engine_text = format!("Moteur : {}", config.engine.as_str());
    let engine_info = MenuItem::with_id(app, "engine_info", &engine_text, false, None::<&str>)?;

    let settings = MenuItem::with_id(app, "settings", "Réglages…", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "Historique…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter SuperParler", true, None::<&str>)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &sep1,
            &hotkey_info,
            &engine_info,
            &sep2,
            &settings,
            &history,
            &sep3,
            &quit,
        ],
    )?;

    // Build an owned Image<'static> from the bundle icon's raw RGBA pixels
    // so it can be stored in managed state (which requires 'static).
    let idle_icon: Image<'static> = app
        .default_window_icon()
        .map(|icon| Image::new_owned(icon.rgba().to_vec(), icon.width(), icon.height()))
        .unwrap_or_else(|| circle_icon(160, 160, 160, 22));

    // Register dynamic items for runtime updates from pipeline
    app.manage(TrayItems {
        toggle: toggle.clone(),
        idle_icon: idle_icon.clone(),
    });

    TrayIconBuilder::with_id("main-tray")
        .icon(idle_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_or_create_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Open the settings window if it exists, or create it lazily (no WebView2 at startup).
pub(crate) fn open_or_create_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    match tauri::WebviewWindowBuilder::new(
        app,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("SuperParler — Réglages")
    .inner_size(680.0, 540.0)
    .min_inner_size(560.0, 420.0)
    .resizable(true)
    .center()
    .build()
    {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => tracing::error!("failed to create settings window: {e}"),
    }
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "quit" => {
            tracing::info!("user quit via tray menu");
            std::process::exit(0);
        }
        "settings" | "history" => {
            open_or_create_main(app);
        }
        "toggle" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let recording_state = state.lock().await.recording_state.clone();
                match recording_state {
                    RecordingState::Idle => {
                        if let Err(e) = crate::pipeline::start_recording(&state, &app).await {
                            tracing::error!("tray start_recording failed: {e}");
                        }
                    }
                    RecordingState::Recording => {
                        if let Err(e) = crate::pipeline::stop_recording(&state, &app).await {
                            tracing::error!("tray stop_recording failed: {e}");
                        }
                    }
                    RecordingState::Transcribing => {
                        tracing::debug!("tray toggle: transcription in progress — ignored");
                    }
                }
            });
        }
        _ => {}
    }
}

/// Update icon, tooltip, and toggle label to reflect the new recording state.
/// Called synchronously from pipeline.rs on every state transition.
pub fn update_tray_state(app: &AppHandle, state: &RecordingState) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tooltip = match state {
            RecordingState::Idle => "SuperParler — Prêt",
            RecordingState::Recording => "SuperParler — Enregistrement…",
            RecordingState::Transcribing => "SuperParler — Transcription…",
        };
        let _ = tray.set_tooltip(Some(tooltip));

        let items = app.state::<TrayItems>();
        let icon = match state {
            RecordingState::Idle => items.idle_icon.clone(),
            RecordingState::Recording => circle_icon(220, 55, 55, 22),
            RecordingState::Transcribing => circle_icon(220, 140, 30, 22),
        };
        let _ = tray.set_icon(Some(icon));

        // macOS: disable template mode for colored state icons so they render in full color.
        #[cfg(target_os = "macos")]
        let _ = tray.set_icon_as_template(matches!(state, RecordingState::Idle));
    }

    // Update toggle menu item label and enabled state
    let items = app.state::<TrayItems>();
    let (label, enabled) = match state {
        RecordingState::Idle => ("Démarrer la dictée", true),
        RecordingState::Recording => ("Arrêter la dictée", true),
        RecordingState::Transcribing => ("Transcription en cours…", false),
    };
    let _ = items.toggle.set_text(label);
    let _ = items.toggle.set_enabled(enabled);
}

// ── Icons ─────────────────────────────────────────────────────────────────────

/// Anti-aliased filled circle as a raw RGBA tray icon.
fn circle_icon(r: u8, g: u8, b: u8, size: u32) -> Image<'static> {
    let n = size as usize;
    let center = (n as f32 - 1.0) / 2.0;
    let radius = center * 0.72;
    let mut px = vec![0u8; n * n * 4];

    for y in 0..n {
        for x in 0..n {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                let alpha = if dist > radius - 1.5 {
                    ((radius - dist) / 1.5 * 230.0) as u8
                } else {
                    230u8
                };
                let i = (y * n + x) * 4;
                px[i] = r;
                px[i + 1] = g;
                px[i + 2] = b;
                px[i + 3] = alpha;
            }
        }
    }
    Image::new_owned(px, size, size)
}

// ── Hotkey display formatting ─────────────────────────────────────────────────

fn format_hotkey(macos: bool, hotkey: &str) -> String {
    if macos {
        hotkey
            .replace("CmdOrCtrl", "⌘")
            .replace("Shift", "⇧")
            .replace("Alt", "⌥")
            .replace('+', "")
    } else {
        hotkey
            .replace("CmdOrCtrl", "Ctrl")
            .replace("Space", "Espace")
    }
}

#[cfg(test)]
mod tests {
    use super::format_hotkey;

    #[test]
    fn format_hotkey_windows() {
        assert_eq!(
            format_hotkey(false, "CmdOrCtrl+Shift+Space"),
            "Ctrl+Shift+Espace"
        );
    }

    #[test]
    fn format_hotkey_macos() {
        assert_eq!(format_hotkey(true, "CmdOrCtrl+Shift+Space"), "⌘⇧Space");
    }

    #[test]
    fn circle_icon_size() {
        let img = super::circle_icon(255, 0, 0, 22);
        assert_eq!(img.width(), 22);
        assert_eq!(img.height(), 22);
        assert_eq!(img.rgba().len(), 22 * 22 * 4);
    }
}
