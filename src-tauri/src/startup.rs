//! startup.rs — post-init status feedback + lazy model provisioning.
//!
//! SuperParler is a tray-only app: there is no window. Without an explicit
//! signal, a user who launches it sees nothing and assumes it failed to start.
//! `announce_ready` runs once after setup and:
//!   • confirms the app is live (notification + tray tooltip),
//!   • for the offline engine, downloads the Parakeet model on first launch, and
//!   • for the Groq engine, warns up-front when no API key is configured —
//!     instead of letting the first dictation fail silently.
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::config::{AppConfig, Engine};
use crate::state::RecordingState;
use crate::tray;

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Show a startup notification and make sure the active engine is ready to use.
pub async fn announce_ready(app: &AppHandle, config: &AppConfig) {
    let hotkey = tray::display_hotkey(&config.hotkey);

    match config.engine {
        Engine::Local => announce_local(app, &hotkey).await,
        Engine::Groq => announce_groq(app, &hotkey),
    }
}

/// Prewarm the model (instant first dictation) and start the idle-unload watcher.
fn start_local_engine() {
    if let Ok(dir) = crate::models::parakeet_v3_dir() {
        crate::stt::local::LocalParakeet::prewarm(dir);
    }
    crate::stt::local::LocalParakeet::spawn_idle_watcher();
}

/// Offline Parakeet: ready if the model is installed, otherwise kick off the
/// first-run download in the background and keep the user informed.
async fn announce_local(app: &AppHandle, hotkey: &str) {
    if crate::models::is_parakeet_v3_installed() {
        tracing::info!("startup: ready (engine=local parakeet)");
        notify(
            app,
            "SuperParler est prêt ✓",
            &format!("Appuie sur {hotkey} pour dicter (Parakeet — 100 % hors-ligne)."),
        );
        tray::update_tray_state(app, &RecordingState::Idle);
        start_local_engine();
        return;
    }

    tracing::info!("startup: Parakeet model missing — downloading");
    notify(
        app,
        "SuperParler — Téléchargement du modèle",
        "Premier lancement : téléchargement du modèle vocal Parakeet (~456 Mo). \
         La dictée sera disponible dès la fin du téléchargement.",
    );

    let app = app.clone();
    let hotkey = hotkey.to_string();
    tauri::async_runtime::spawn(async move {
        match crate::models::ensure_parakeet_v3(&app).await {
            Ok(_) => {
                tracing::info!("startup: Parakeet model installed");
                notify(
                    &app,
                    "SuperParler est prêt ✓",
                    &format!("Modèle installé. Appuie sur {hotkey} pour dicter (hors-ligne)."),
                );
                tray::update_tray_state(&app, &RecordingState::Idle);
                start_local_engine();
            }
            Err(e) => {
                tracing::error!("Parakeet download failed: {e}");
                tray::show_tray_error(&app, "Échec du téléchargement du modèle");
                notify(
                    &app,
                    "SuperParler — Erreur",
                    &format!(
                        "Le téléchargement du modèle a échoué : {e}\n\
                         Vérifie ta connexion puis relance l'application."
                    ),
                );
            }
        }
    });
}

/// Groq cloud: warn immediately if there is no API key.
fn announce_groq(app: &AppHandle, hotkey: &str) {
    if !crate::stt::groq::GroqWhisper::has_api_key() {
        tracing::warn!("startup: Groq engine selected but no API key found");
        tray::show_tray_error(app, "Clé Groq manquante — clic droit → « Coller clé Groq »");
        notify(
            app,
            "SuperParler — Action requise",
            "Aucune clé Groq trouvée. Copie ta clé (gsk_…), puis fais un clic droit \
             sur l'icône de la barre des tâches → « Coller clé Groq ».",
        );
        return;
    }
    tracing::info!("startup: ready (engine=groq)");
    notify(
        app,
        "SuperParler est prêt ✓",
        &format!("Appuie sur {hotkey} pour dicter (moteur Groq cloud)."),
    );
    tray::update_tray_state(app, &RecordingState::Idle);
}
