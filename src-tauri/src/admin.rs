//! admin.rs — native egui admin/settings window for SuperParler (Windows).
//!
//! Launched as a SEPARATE PROCESS (`SuperParler.exe --admin`) so it owns a clean
//! main-thread event loop — no winit-on-secondary-thread hacks, no Chromium.
//! ~30 MB while open, 0 when closed (Strategy D of the tauri-hyperlight-ui skill).
//!
//! It edits the SAME shared stores the tray app uses — `config.toml`, the SQLite
//! DB, and the OS keyring — so no IPC is needed. The pipeline reloads config from
//! disk before each dictation, so changes here apply to the running app without a
//! restart (the global hotkey is the exception — it is registered at startup).

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::config::{AppConfig, Engine, RecordingMode};
use crate::{models, storage};

const KEYRING_SERVICE: &str = "superparler";
const KEYRING_ACCOUNT: &str = "groq";

/// Entry point for the `--admin` process mode. Blocks until the window closes.
pub fn run_admin() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 700.0])
            .with_min_inner_size([460.0, 480.0])
            .with_title("SuperParler — Administration"),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "SuperParler Admin",
        options,
        Box::new(|_cc| Ok(Box::new(AdminApp::new()) as Box<dyn eframe::App>)),
    ) {
        tracing::error!("admin window failed: {e}");
    }
}

#[derive(Default)]
struct DownloadState {
    active: bool,
    downloaded: u64,
    total: u64,
    done: bool,
    error: Option<String>,
}

struct AdminApp {
    rt: tokio::runtime::Runtime,
    cfg: AppConfig,
    groq_key_input: String,
    groq_key_present: bool,
    pool: Option<sqlx::SqlitePool>,
    history: Vec<storage::models::Transcription>,
    history_loaded: bool,
    model_installed: bool,
    download: Arc<Mutex<DownloadState>>,
    status: String,
}

impl AdminApp {
    fn new() -> Self {
        // INVARIANT: a fresh process can always create one Tokio runtime; if the
        // OS refuses, there is nothing useful the admin window could do anyway.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime for admin");
        let cfg = AppConfig::load_direct().unwrap_or_default();
        let groq_key_present = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .ok()
            .and_then(|e| e.get_password().ok())
            .map(|k| !k.is_empty())
            .unwrap_or(false);
        let pool = rt.block_on(async { storage::db::open_pool().await.ok() });
        Self {
            rt,
            cfg,
            groq_key_input: String::new(),
            groq_key_present,
            pool,
            history: Vec::new(),
            history_loaded: false,
            model_installed: models::is_parakeet_v3_installed(),
            download: Arc::new(Mutex::new(DownloadState::default())),
            status: String::new(),
        }
    }

    fn save_cfg(&mut self) {
        self.status = match self.cfg.save_direct() {
            Ok(()) => "Réglages enregistrés.".into(),
            Err(e) => format!("Échec d'enregistrement : {e}"),
        };
    }

    fn load_history(&mut self) {
        let Some(pool) = self.pool.clone() else {
            self.status = "Base de données indisponible".into();
            return;
        };
        match self
            .rt
            .block_on(storage::models::list_transcriptions(&pool, 100, 0))
        {
            Ok(rows) => {
                self.history = rows;
                self.history_loaded = true;
            }
            Err(e) => self.status = format!("Historique : {e}"),
        }
    }

    fn save_groq_key(&mut self) {
        if self.groq_key_input.is_empty() {
            return;
        }
        let res = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .and_then(|e| e.set_password(&self.groq_key_input));
        self.status = match res {
            Ok(()) => {
                self.groq_key_present = true;
                self.groq_key_input.clear();
                "Clé Groq enregistrée.".into()
            }
            Err(e) => format!("Keyring : {e}"),
        };
    }

    /// Toggle the HKCU Run key directly so it takes effect immediately. The tray
    /// app also syncs this from config on startup, so we persist config too.
    fn set_autostart(&mut self, enable: bool) {
        const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let res = if enable {
            let Ok(exe) = std::env::current_exe() else {
                self.status = "Autostart : exe introuvable".into();
                return;
            };
            std::process::Command::new("reg")
                .args([
                    "add",
                    RUN_KEY,
                    "/v",
                    "SuperParler",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &exe.to_string_lossy(),
                    "/f",
                ])
                .output()
        } else {
            std::process::Command::new("reg")
                .args(["delete", RUN_KEY, "/v", "SuperParler", "/f"])
                .output()
        };
        match res {
            Ok(o) if o.status.success() => {
                self.cfg.launch_at_startup = enable;
                self.save_cfg();
            }
            Ok(o) => self.status = format!("Autostart : {}", String::from_utf8_lossy(&o.stderr)),
            Err(e) => self.status = format!("Autostart : {e}"),
        }
    }

    fn start_download(&mut self) {
        {
            let Ok(mut g) = self.download.lock() else {
                return;
            };
            if g.active {
                return;
            }
            *g = DownloadState {
                active: true,
                ..Default::default()
            };
        }
        let dl = self.download.clone();
        self.rt.spawn(async move {
            let dl2 = dl.clone();
            let res = models::ensure_parakeet_v3_cb(move |d, t| {
                if let Ok(mut g) = dl2.lock() {
                    g.downloaded = d;
                    g.total = t;
                }
            })
            .await;
            if let Ok(mut g) = dl.lock() {
                g.active = false;
                match res {
                    Ok(_) => g.done = true,
                    Err(e) => g.error = Some(e.to_string()),
                }
            }
        });
    }
}

impl eframe::App for AdminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Snapshot the (mutex-guarded) download state for this frame.
        let (dl_active, dl_done, dl_dl, dl_total, dl_err) = self
            .download
            .lock()
            .map(|g| (g.active, g.done, g.downloaded, g.total, g.error.clone()))
            .unwrap_or((false, false, 0, 0, None));
        if dl_active {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        if dl_done && !self.model_installed {
            self.model_installed = true;
            self.status = "Modèle téléchargé.".into();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("SuperParler — Administration");
                ui.separator();

                // ── Moteur STT ───────────────────────────────────────────────
                ui.label(egui::RichText::new("Moteur STT").strong());
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.cfg.engine, Engine::Local, "Offline (Parakeet)");
                    ui.radio_value(&mut self.cfg.engine, Engine::Groq, "Groq (cloud)");
                });
                if self.cfg.engine == Engine::Groq {
                    ui.horizontal(|ui| {
                        ui.label("Modèle Groq :");
                        ui.text_edit_singleline(&mut self.cfg.groq_model);
                    });
                    ui.label(if self.groq_key_present {
                        "Clé API : définie"
                    } else {
                        "Clé API : absente"
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.groq_key_input)
                                .password(true)
                                .hint_text("Nouvelle clé Groq"),
                        );
                        if ui.button("Enregistrer la clé").clicked() {
                            self.save_groq_key();
                        }
                    });
                }
                ui.add_space(10.0);

                // ── Langue ───────────────────────────────────────────────────
                ui.label(egui::RichText::new("Langue").strong());
                ui.horizontal(|ui| {
                    ui.label("Code langue (ex. fr, en) :");
                    ui.text_edit_singleline(&mut self.cfg.language);
                });
                ui.add_space(10.0);

                // ── Raccourci + démarrage ────────────────────────────────────
                ui.label(egui::RichText::new("Raccourci & démarrage").strong());
                ui.horizontal(|ui| {
                    ui.label("Raccourci :");
                    ui.text_edit_singleline(&mut self.cfg.hotkey);
                    ui.weak("(redémarrage requis)");
                });
                ui.horizontal(|ui| {
                    ui.label("Mode :");
                    ui.radio_value(&mut self.cfg.mode, RecordingMode::Toggle, "Bascule");
                    ui.radio_value(&mut self.cfg.mode, RecordingMode::PushToTalk, "Maintien");
                });
                let mut autostart = self.cfg.launch_at_startup;
                if ui
                    .checkbox(&mut autostart, "Démarrage automatique avec Windows")
                    .changed()
                {
                    self.set_autostart(autostart);
                }
                ui.add_space(10.0);

                // ── Modèle offline ───────────────────────────────────────────
                ui.label(egui::RichText::new("Modèle offline (Parakeet)").strong());
                if self.model_installed {
                    ui.label("Installé.");
                } else if dl_active {
                    let frac = if dl_total > 0 {
                        dl_dl as f32 / dl_total as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(frac).show_percentage());
                    ui.weak(format!(
                        "{:.1} / {:.1} Mo",
                        dl_dl as f64 / 1e6,
                        dl_total as f64 / 1e6
                    ));
                } else {
                    if let Some(err) = &dl_err {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                    }
                    if ui
                        .button("Télécharger le modèle (plusieurs centaines de Mo)")
                        .clicked()
                    {
                        self.start_download();
                    }
                }
                ui.add_space(10.0);

                // ── Save / reload ────────────────────────────────────────────
                ui.horizontal(|ui| {
                    if ui.button("Enregistrer les réglages").clicked() {
                        self.save_cfg();
                    }
                    if ui.button("Recharger depuis le disque").clicked() {
                        self.cfg = AppConfig::load_direct().unwrap_or_default();
                        self.status = "Réglages rechargés".into();
                    }
                });
                ui.separator();

                // ── Historique ───────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Historique").strong());
                    if ui
                        .button(if self.history_loaded {
                            "Rafraîchir"
                        } else {
                            "Charger"
                        })
                        .clicked()
                    {
                        self.load_history();
                    }
                    if ui.button("Tout effacer").clicked() {
                        if let Some(pool) = self.pool.clone() {
                            let _ = self.rt.block_on(storage::models::clear_all(&pool));
                            self.load_history();
                        }
                    }
                });
                let mut to_delete: Option<String> = None;
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .id_salt("history")
                    .show(ui, |ui| {
                        for t in &self.history {
                            ui.horizontal(|ui| {
                                if ui.small_button("Suppr").clicked() {
                                    to_delete = Some(t.id.clone());
                                }
                                let when = t.created_at.get(0..16).unwrap_or(&t.created_at);
                                ui.weak(format!("{when}  [{}]", t.engine));
                                let preview: String = t.text.chars().take(60).collect();
                                ui.label(preview);
                            });
                        }
                        if self.history_loaded && self.history.is_empty() {
                            ui.weak("(vide)");
                        }
                    });
                if let Some(id) = to_delete {
                    if let Some(pool) = self.pool.clone() {
                        let _ = self
                            .rt
                            .block_on(storage::models::delete_transcription(&pool, &id));
                        self.load_history();
                    }
                }

                if !self.status.is_empty() {
                    ui.separator();
                    ui.label(&self.status);
                }
            });
        });
    }
}
