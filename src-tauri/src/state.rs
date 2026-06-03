use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RecordingState {
    Idle,
    Recording,
    Transcribing,
}

pub struct Inner {
    pub config: AppConfig,
    pub recording_state: RecordingState,
    pub audio_samples: Option<Vec<f32>>,
    pub db_pool: Option<sqlx::SqlitePool>,
}

impl Inner {
    fn new(config: AppConfig) -> Self {
        Self {
            config,
            recording_state: RecordingState::Idle,
            audio_samples: None,
            db_pool: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState(pub Arc<Mutex<Inner>>);

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self(Arc::new(Mutex::new(Inner::new(config))))
    }

    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, Inner> {
        self.0.lock().await
    }
}
