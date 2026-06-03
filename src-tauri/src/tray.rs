use tauri::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager,
};

use crate::state::{AppState, RecordingState};

pub fn setup_tray(app: &mut App) -> tauri::Result<()> {
    let toggle_item = MenuItem::with_id(app, "toggle", "Démarrer la dictée", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Réglages", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "Historique", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quitter SuperParler", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &toggle_item,
        &sep,
        &settings_item,
        &history_item,
        &sep,
        &quit_item,
    ])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(
            // INVARIANT: tauri.conf.json always specifies a bundle icon; missing icon is a build error
            app.default_window_icon()
                .cloned()
                .unwrap_or_else(|| tauri::image::Image::new(&[], 1, 1)),
        )
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
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "quit" => {
            tracing::info!("user requested quit");
            app.exit(0);
        }
        "settings" | "history" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "toggle" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let recording_state = {
                    let guard = state.lock().await;
                    guard.recording_state.clone()
                };
                match recording_state {
                    RecordingState::Idle => {
                        if let Err(e) = crate::pipeline::start_recording(&state, &app).await {
                            tracing::error!("Tray start recording failed: {e}");
                        }
                    }
                    RecordingState::Recording => {
                        if let Err(e) = crate::pipeline::stop_recording(&state, &app).await {
                            tracing::error!("Tray stop recording failed: {e}");
                        }
                    }
                    RecordingState::Transcribing => {
                        tracing::debug!("Tray toggle: transcription in progress, ignoring");
                    }
                }
            });
        }
        _ => {}
    }
}

pub fn update_tray_state(app: &AppHandle, state: &RecordingState) {
    let tooltip = match state {
        RecordingState::Idle => "SuperParler — Prêt",
        RecordingState::Recording => "SuperParler — Enregistrement...",
        RecordingState::Transcribing => "SuperParler — Transcription...",
    };
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(tooltip));
    }
}
