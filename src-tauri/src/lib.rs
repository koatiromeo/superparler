use tauri::Manager;
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
    // Single-instance guard: TcpListener stays open for the lifetime of run().
    // If the port is already bound, another instance is running — exit silently.
    let _instance_guard = match std::net::TcpListener::bind("127.0.0.1:57321") {
        Ok(l) => l,
        Err(_) => {
            eprintln!("SuperParler est déjà en cours d'exécution.");
            std::process::exit(0);
        }
    };

    tracing_subscriber::fmt()
        // INVARIANT: "superparler=info" is a valid constant directive string
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(
                "superparler=info"
                    .parse()
                    .expect("constant directive is valid"),
            ),
        )
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
            engine::set_groq_key,
            system::set_launch_at_startup,
        ])
        .build(tauri::generate_context!())
        .expect("error building SuperParler")
        .run(|_app, event| {
            // Prevent the app from exiting when the settings window is closed —
            // it is a tray-only app; the user quits via the tray menu.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
