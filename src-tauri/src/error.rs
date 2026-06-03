use serde::Serializer;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("audio device not found: {0}")]
    AudioDeviceNotFound(String),
    #[error("audio stream error: {0}")]
    AudioStream(String),
    #[error("STT error: {0}")]
    Stt(String),
    #[error("model load failed: {0}")]
    ModelLoad(String),
    #[error("model not found at path: {0}")]
    ModelNotFound(String),
    #[error("inject failed: {0}")]
    Inject(String),
    #[error("storage error: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("hotkey register failed: {0}")]
    HotkeyRegister(String),
    #[error("invalid hotkey: {0}")]
    InvalidHotkey(String),
    #[error("event emit failed: {0}")]
    Emit(String),
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("not recording")]
    NotRecording,
    #[error("already recording")]
    AlreadyRecording,
    #[error("enhance error: {0}")]
    Enhance(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// Required: Tauri commands return Result<T, AppError> and AppError must be serializable for IPC.
// Use std::result::Result explicitly to avoid conflict with our Result<T> type alias below.
impl serde::Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e.to_string())
    }
}
