use directories::ProjectDirs;
use sqlx::{SqlitePool, migrate::MigrateDatabase};
use tauri::{AppHandle, Manager};

use crate::{
    error::{AppError, Result},
    state::AppState,
};

pub async fn initialize(app: &AppHandle) -> Result<()> {
    let pool = open_pool().await?;
    // Store pool in AppState
    let state = app.state::<AppState>();
    let mut guard = state.lock().await;
    guard.db_pool = Some(pool);
    Ok(())
}

/// Open (creating + migrating if needed) a pool to the shared SQLite DB without
/// touching `AppState`. Used by the standalone admin process; SQLite's file
/// locking lets it coexist with the running tray app.
pub async fn open_pool() -> Result<SqlitePool> {
    let db_path = get_db_path()?;

    // Create parent directory if needed
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Storage(sqlx::Error::Io(e)))?;
    }

    let db_url = format!("sqlite:{}", db_path.to_string_lossy());

    if !sqlx::Sqlite::database_exists(&db_url)
        .await
        .unwrap_or(false)
    {
        sqlx::Sqlite::create_database(&db_url)
            .await
            .map_err(AppError::Storage)?;
    }

    let pool = SqlitePool::connect(&db_url)
        .await
        .map_err(AppError::Storage)?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Storage(sqlx::Error::Migrate(Box::new(e))))?;

    tracing::info!(path = %db_path.display(), "SQLite initialized");
    Ok(pool)
}

fn get_db_path() -> Result<std::path::PathBuf> {
    ProjectDirs::from("com", "nemaleu", "superparler")
        .map(|dirs| dirs.data_dir().join("superparler.db"))
        .ok_or_else(|| AppError::Config("cannot determine data dir for SQLite".to_string()))
}
