use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub engine: Engine,
    pub local_model_path: String,
    pub groq_model: String,
    pub language: String,
    pub hotkey: String,
    pub mode: RecordingMode,
    pub inject_method: InjectMethod,
    pub enhance_enabled: bool,
    pub enhance_prompt: String,
    pub enhance_model: String,
    pub launch_at_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    Local,
    Groq,
}

impl Engine {
    pub fn as_str(&self) -> &'static str {
        match self {
            Engine::Local => "local",
            Engine::Groq => "groq",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RecordingMode {
    PushToTalk,
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum InjectMethod {
    Paste,
    Type,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            engine: Engine::Local,
            local_model_path: default_model_path(),
            groq_model: "whisper-large-v3-turbo".to_string(),
            language: "fr".to_string(),
            hotkey: "CmdOrCtrl+Shift+Space".to_string(),
            mode: RecordingMode::PushToTalk,
            inject_method: InjectMethod::Paste,
            enhance_enabled: false,
            enhance_prompt: String::new(),
            enhance_model: "llama-3.3-70b-versatile".to_string(),
            launch_at_startup: false,
        }
    }
}

fn default_model_path() -> String {
    // Try to find models/ relative to the binary location
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("../../models/ggml-small.bin")))
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "models/ggml-small.bin".to_string())
}

impl AppConfig {
    pub fn config_path(_app: &AppHandle) -> Option<PathBuf> {
        ProjectDirs::from("com", "nemaleu", "superparler")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    pub fn load(app: &AppHandle) -> Result<Self> {
        let path = Self::config_path(app)
            .ok_or_else(|| AppError::Config("cannot determine config dir".to_string()))?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content =
            std::fs::read_to_string(&path).map_err(|e| AppError::Config(e.to_string()))?;
        toml::from_str(&content).map_err(|e| AppError::Config(e.to_string()))
    }

    pub fn save(&self, app: &AppHandle) -> Result<()> {
        let path = Self::config_path(app)
            .ok_or_else(|| AppError::Config("cannot determine config dir".to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Config(e.to_string()))?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| AppError::Config(e.to_string()))?;
        std::fs::write(&path, content).map_err(|e| AppError::Config(e.to_string()))
    }
}
