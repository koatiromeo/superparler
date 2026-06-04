use tauri::{
    App, AppHandle, Manager,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    config::{AppConfig, RecordingMode},
    state::{AppState, RecordingState},
};

// ── Language list ─────────────────────────────────────────────────────────────

const LANGUAGES: &[(&str, &str)] = &[
    ("fr", "Français"),
    ("en", "English"),
    ("es", "Español"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("auto", "Auto"),
];

fn next_language(current: &str) -> &'static str {
    let idx = LANGUAGES
        .iter()
        .position(|(code, _)| *code == current)
        .unwrap_or(0);
    LANGUAGES[(idx + 1) % LANGUAGES.len()].0
}

fn lang_label(code: &str) -> String {
    let name = LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| *n)
        .unwrap_or(code);
    format!("Langue : {name}")
}

fn mode_label(mode: &RecordingMode) -> &'static str {
    match mode {
        RecordingMode::PushToTalk => "Mode : Push-to-Talk",
        RecordingMode::Toggle => "Mode : Toggle",
    }
}

// ── Managed state ─────────────────────────────────────────────────────────────

pub struct TrayItems {
    pub toggle: MenuItem<tauri::Wry>,
    pub lang_item: MenuItem<tauri::Wry>,
    pub mode_item: MenuItem<tauri::Wry>,
    pub idle_icon: Image<'static>,
}

// ── Setup ─────────────────────────────────────────────────────────────────────

pub fn setup_tray(app: &mut App, config: &AppConfig) -> tauri::Result<()> {
    let hotkey_label = format_hotkey(cfg!(target_os = "macos"), &config.hotkey);
    let toggle_text = format!("Démarrer la dictée  {hotkey_label}");
    let toggle = MenuItem::with_id(app, "toggle", &toggle_text, true, None::<&str>)?;

    let lang_item = MenuItem::with_id(
        app,
        "cycle_lang",
        lang_label(&config.language),
        true,
        None::<&str>,
    )?;
    let mode_item = MenuItem::with_id(
        app,
        "cycle_mode",
        mode_label(&config.mode),
        true,
        None::<&str>,
    )?;

    let paste_key = MenuItem::with_id(
        app,
        "paste_groq_key",
        "Coller clé Groq (presse-papiers)",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quitter SuperParler", true, None::<&str>)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &sep1,
            &lang_item,
            &mode_item,
            &sep2,
            &paste_key,
            &sep3,
            &quit,
        ],
    )?;

    let idle_icon: Image<'static> = app
        .default_window_icon()
        .map(|icon| Image::new_owned(icon.rgba().to_vec(), icon.width(), icon.height()))
        .unwrap_or_else(|| circle_icon(160, 160, 160, 22));

    app.manage(TrayItems {
        toggle: toggle.clone(),
        lang_item: lang_item.clone(),
        mode_item: mode_item.clone(),
        idle_icon: idle_icon.clone(),
    });

    TrayIconBuilder::with_id("main-tray")
        .icon(idle_icon)
        .menu(&menu)
        .tooltip("SuperParler — Prêt")
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            // Left click = start/stop dictation (no window to open)
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let rs = state.lock().await.recording_state.clone();
                    match rs {
                        RecordingState::Idle => {
                            if let Err(e) = crate::pipeline::start_recording(&state, &app).await {
                                tracing::error!("left-click start failed: {e}");
                            }
                        }
                        RecordingState::Recording => {
                            if let Err(e) = crate::pipeline::stop_recording(&state, &app).await {
                                tracing::error!("left-click stop failed: {e}");
                            }
                        }
                        RecordingState::Transcribing => {}
                    }
                });
            }
        })
        .build(app)?;

    Ok(())
}

// ── Menu events ───────────────────────────────────────────────────────────────

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "quit" => {
            tracing::info!("user quit via tray menu");
            std::process::exit(0);
        }

        "toggle" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let rs = state.lock().await.recording_state.clone();
                match rs {
                    RecordingState::Idle => {
                        if let Err(e) = crate::pipeline::start_recording(&state, &app).await {
                            tracing::error!("tray toggle start failed: {e}");
                        }
                    }
                    RecordingState::Recording => {
                        if let Err(e) = crate::pipeline::stop_recording(&state, &app).await {
                            tracing::error!("tray toggle stop failed: {e}");
                        }
                    }
                    RecordingState::Transcribing => {}
                }
            });
        }

        "cycle_lang" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let mut guard = state.lock().await;
                let next = next_language(&guard.config.language);
                guard.config.language = next.to_string();
                let label = lang_label(next);
                if let Err(e) = guard.config.save(app.app_handle()) {
                    tracing::error!("save config (lang): {e}");
                }
                drop(guard);
                if let Some(items) = app.try_state::<TrayItems>() {
                    let _ = items.lang_item.set_text(&label);
                }
                tracing::info!(lang = next, "language changed");
            });
        }

        "cycle_mode" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let mut guard = state.lock().await;
                guard.config.mode = match guard.config.mode {
                    RecordingMode::PushToTalk => RecordingMode::Toggle,
                    RecordingMode::Toggle => RecordingMode::PushToTalk,
                };
                let label = mode_label(&guard.config.mode);
                if let Err(e) = guard.config.save(app.app_handle()) {
                    tracing::error!("save config (mode): {e}");
                }
                drop(guard);
                if let Some(items) = app.try_state::<TrayItems>() {
                    let _ = items.mode_item.set_text(label);
                }
            });
        }

        "paste_groq_key" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                // arboard is not Send — run on blocking thread
                let result = tokio::task::spawn_blocking(|| {
                    arboard::Clipboard::new()
                        .and_then(|mut c| c.get_text())
                })
                .await;

                let key = match result {
                    Ok(Ok(text)) if !text.trim().is_empty() => text.trim().to_string(),
                    Ok(Ok(_)) => {
                        show_tray_error(&app, "Presse-papiers vide — copiez votre clé Groq d'abord");
                        return;
                    }
                    _ => {
                        show_tray_error(&app, "Impossible de lire le presse-papiers");
                        return;
                    }
                };

                // Basic sanity check — Groq keys start with "gsk_"
                if !key.starts_with("gsk_") {
                    show_tray_error(&app, "Ce n'est pas une clé Groq (doit commencer par gsk_)");
                    return;
                }

                match keyring::Entry::new("superparler", "groq")
                    .and_then(|e| { e.set_password(&key).map(|_| ()) })
                {
                    Ok(()) => {
                        tracing::info!("Groq API key saved from clipboard");
                        if let Some(tray) = app.tray_by_id("main-tray") {
                            let _ = tray.set_tooltip(Some("SuperParler — Clé Groq sauvegardée ✓"));
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        update_tray_state(&app, &RecordingState::Idle);
                    }
                    Err(e) => {
                        tracing::error!("keyring write failed: {e}");
                        show_tray_error(&app, "Impossible de sauvegarder la clé");
                    }
                }
            });
        }

        _ => {}
    }
}

// ── Tray state updates ────────────────────────────────────────────────────────

pub fn show_tray_error(app: &AppHandle, message: &str) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&format!("SuperParler — Erreur: {message}")));
        let _ = tray.set_icon(Some(circle_icon(220, 180, 0, 22)));
        #[cfg(target_os = "macos")]
        let _ = tray.set_icon_as_template(false);
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.toggle.set_text("Démarrer la dictée");
        let _ = items.toggle.set_enabled(true);
    }
}

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

        #[cfg(target_os = "macos")]
        let _ = tray.set_icon_as_template(matches!(state, RecordingState::Idle));
    }

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

// ── Hotkey formatting ─────────────────────────────────────────────────────────

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        let img = circle_icon(255, 0, 0, 22);
        assert_eq!(img.width(), 22);
        assert_eq!(img.height(), 22);
        assert_eq!(img.rgba().len(), 22 * 22 * 4);
    }

    #[test]
    fn lang_cycling_wraps() {
        let last = LANGUAGES.last().unwrap().0;
        assert_eq!(next_language(last), LANGUAGES[0].0);
    }

    #[test]
    fn lang_label_format() {
        assert_eq!(lang_label("fr"), "Langue : Français");
        assert_eq!(lang_label("auto"), "Langue : Auto");
    }
}
