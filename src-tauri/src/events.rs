use serde::{Deserialize, Serialize};

// Event name constants — must match src/src/lib/events.ts
pub const EVT_RECORDING_STARTED: &str = "recording:started";
pub const EVT_RECORDING_STOPPED: &str = "recording:stopped";
pub const EVT_RECORDING_TRANSCRIBING: &str = "recording:transcribing";
pub const EVT_RECORDING_RESULT: &str = "recording:result";
pub const EVT_RECORDING_ERROR: &str = "recording:error";
pub const EVT_ENGINE_CHANGED: &str = "engine:changed";

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
