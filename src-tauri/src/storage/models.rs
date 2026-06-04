use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    pub id: String,
    pub created_at: String,
    pub text: String,
    pub duration_ms: i64,
    pub engine: String,
    pub language: String,
    pub target_app: Option<String>,
    pub enhanced: i64, // SQLite bool as INTEGER (0/1)
}

pub async fn insert_transcription(
    pool: &SqlitePool,
    text: &str,
    duration_ms: i64,
    engine: &str,
    language: &str,
    enhanced: bool,
) -> Result<Transcription> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let enhanced_int: i64 = if enhanced { 1 } else { 0 };

    sqlx::query(
        "INSERT INTO transcriptions (id, created_at, text, duration_ms, engine, language, target_app, enhanced)
         VALUES (?, ?, ?, ?, ?, ?, NULL, ?)",
    )
    .bind(&id)
    .bind(&created_at)
    .bind(text)
    .bind(duration_ms)
    .bind(engine)
    .bind(language)
    .bind(enhanced_int)
    .execute(pool)
    .await
    .map_err(AppError::Storage)?;

    Ok(Transcription {
        id,
        created_at,
        text: text.to_string(),
        duration_ms,
        engine: engine.to_string(),
        language: language.to_string(),
        target_app: None,
        enhanced: enhanced_int,
    })
}

pub async fn list_transcriptions(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transcription>> {
    sqlx::query_as::<_, Transcription>(
        "SELECT id, created_at, text, duration_ms, engine, language, target_app, enhanced
         FROM transcriptions ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::Storage)
}

pub async fn delete_transcription(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM transcriptions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(AppError::Storage)?;
    Ok(())
}

pub async fn clear_all(pool: &SqlitePool) -> Result<()> {
    sqlx::query("DELETE FROM transcriptions")
        .execute(pool)
        .await
        .map_err(AppError::Storage)?;
    Ok(())
}
