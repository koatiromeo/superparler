use serde::{Deserialize, Serialize};

// Event name constants — must match src/src/lib/events.ts
pub const EVT_RECORDING_STARTED: &str = "recording:started";
pub const EVT_RECORDING_STOPPED: &str = "recording:stopped";
pub const EVT_RECORDING_TRANSCRIBING: &str = "recording:transcribing";
pub const EVT_RECORDING_RESULT: &str = "recording:result";
pub const EVT_RECORDING_ERROR: &str = "recording:error";
pub const EVT_ENGINE_CHANGED: &str = "engine:changed";
pub const EVT_MODEL_DOWNLOAD_PROGRESS: &str = "model:download:progress";
pub const EVT_MODEL_DOWNLOAD_DONE: &str = "model:download:done";
pub const EVT_MODEL_DOWNLOAD_ERROR: &str = "model:download:error";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResultPayload {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingErrorPayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineChangedPayload {
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgressPayload {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}
