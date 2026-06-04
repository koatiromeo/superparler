use tauri::{LogicalPosition, Manager};
use tauri_plugin_autostart::ManagerExt;
use tracing_subscriber::EnvFilter;

pub mod audio;
pub mod commands;
pub mod config;
pub mod enhance;
pub mod error;
pub mod events;
pub mod hotkey;
pub mod inject;
pub mod pipeline;
pub mod state;
pub mod storage;
pub mod stt;
pub mod tray;

use commands::{engine, history, recording, settings, system};
use state::AppState;

pub fn run() {
    tracing_subscriber::fmt()
        // INVARIANT: "superparler=info" is a valid constant directive string
        .with_env_filter(EnvFilter::from_default_env().add_directive("superparler=info".parse().expect("constant directive is valid")))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            let config = config::AppConfig::load(app.handle()).unwrap_or_default();
            let state = AppState::new(config.clone());
            app.manage(state);

            // Setup system tray (pass config for hotkey + engine display)
            tray::setup_tray(app, &config)?;

            // Sync autostart with config (keeps OS state and config in sync on restart)
            {
                let autostart = app.autolaunch();
                let synced = autostart.is_enabled().unwrap_or(false);
                if config.launch_at_startup != synced {
                    if config.launch_at_startup {
                        if let Err(e) = autostart.enable() {
                            tracing::warn!("autostart enable failed: {e}");
                        }
                    } else if let Err(e) = autostart.disable() {
                        tracing::warn!("autostart disable failed: {e}");
                    }
                }
            }

            // Position overlay at center-bottom of the primary monitor.
            // visible:true + focus:false in tauri.conf.json means the window is already
            // shown without stealing focus; we only need to move it to the right position.
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Ok(Some(monitor)) = overlay.primary_monitor() {
                    let scale = monitor.scale_factor();
                    let mon_w = monitor.size().width as f64 / scale;
                    let mon_h = monitor.size().height as f64 / scale;
                    let win_w = 400.0_f64;
                    let win_h = 120.0_f64;
                    let x = (mon_w - win_w) / 2.0;
                    let y = mon_h - win_h - 80.0; // 80 px above taskbar
                    let _ = overlay.set_position(LogicalPosition::new(x, y));
                    tracing::info!(x, y, "overlay positioned center-bottom");
                }

                // macOS: set NSWindowCollectionBehavior so the overlay stays visible
                // above fullscreen apps and on all Mission Control spaces.
                // TODO: implement via objc2 raw window handle — deferred to macOS platform PR.
                #[cfg(target_os = "macos")]
                tracing::debug!("macOS: NSWindowCollectionBehavior not yet set (fullscreen overlay pending)");
            }

            // Register hotkey from config
            {
                let hotkey = config.hotkey.clone();
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = hotkey::register_hotkey(&handle, &hotkey).await {
                        tracing::warn!("Failed to register hotkey '{}': {e}", hotkey);
                    }
                });
            }

            // Initialize SQLite storage
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = storage::db::initialize(&handle).await {
                    tracing::error!("Storage init failed: {e}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            recording::start_recording,
            recording::stop_recording,
            recording::get_recording_state,
            settings::get_settings,
            settings::update_settings,
            history::list_transcriptions,
            history::delete_transcription,
            history::clear_history,
            engine::test_engine,
            engine::list_local_models,
            engine::set_groq_key,
            system::set_launch_at_startup,
            system::open_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SuperParler");
}
