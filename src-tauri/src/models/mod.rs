//! models/mod.rs — offline STT model download, checksum verification, extraction.
//!
//! The Parakeet V3 model ships as a sha256-verified `.tar.gz` from the Handy CDN.
//! On first launch we stream it to disk (emitting progress events), verify its
//! checksum, extract it into the app data dir, and drop a `.complete` marker so
//! a half-finished download is never mistaken for an installed model.
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tar::Archive;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, Result};
use crate::events::{self, ModelDownloadProgressPayload};

/// Engine/model id surfaced in events and history rows.
pub const PARAKEET_V3_ID: &str = "parakeet-tdt-0.6b-v3";
/// Directory name the archive extracts to (must match what `ParakeetModel::load` expects).
const PARAKEET_V3_DIRNAME: &str = "parakeet-tdt-0.6b-v3-int8";
const PARAKEET_V3_URL: &str = "https://blob.handy.computer/parakeet-v3-int8.tar.gz";
const PARAKEET_V3_SHA256: &str = "43d37191602727524a7d8c6da0eef11c4ba24320f5b4730f1a2497befc2efa77";

fn models_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "nemaleu", "superparler")
        .ok_or_else(|| AppError::Config("cannot determine data dir".to_string()))?;
    Ok(dirs.data_dir().join("models"))
}

/// Absolute path to the extracted Parakeet V3 model directory.
pub fn parakeet_v3_dir() -> Result<PathBuf> {
    Ok(models_dir()?.join(PARAKEET_V3_DIRNAME))
}

/// A model counts as installed only when extraction fully completed.
pub fn is_parakeet_v3_installed() -> bool {
    parakeet_v3_dir()
        .map(|d| d.join(".complete").exists())
        .unwrap_or(false)
}

/// Ensure the Parakeet V3 model is present, downloading + extracting if needed.
/// Idempotent; safe to call on every startup.
pub async fn ensure_parakeet_v3(app: &AppHandle) -> Result<PathBuf> {
    let dir = parakeet_v3_dir()?;
    if is_parakeet_v3_installed() {
        return Ok(dir);
    }

    let models = models_dir()?;
    fs::create_dir_all(&models)?;

    let archive = models.join("parakeet-v3-int8.tar.gz.part");

    tracing::info!(url = PARAKEET_V3_URL, "downloading Parakeet V3 model");
    if let Err(e) = download_with_progress(app, PARAKEET_V3_URL, &archive, PARAKEET_V3_ID).await {
        let _ = app.emit(events::EVT_MODEL_DOWNLOAD_ERROR, e.to_string());
        return Err(e);
    }

    tracing::info!("verifying Parakeet V3 checksum");
    verify_sha256(&archive, PARAKEET_V3_SHA256)?;

    tracing::info!("extracting Parakeet V3");
    extract_tar_gz(&archive, &models)?;

    // Mark complete, then clean up the archive.
    fs::write(dir.join(".complete"), b"ok")?;
    let _ = fs::remove_file(&archive);

    let _ = app.emit(events::EVT_MODEL_DOWNLOAD_DONE, PARAKEET_V3_ID);
    tracing::info!("Parakeet V3 ready at {}", dir.display());
    Ok(dir)
}

async fn download_with_progress(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    model_id: &str,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Download(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(AppError::Download(format!(
            "HTTP {} while fetching {url}",
            resp.status()
        )));
    }
    let total = resp.content_length().unwrap_or(0);

    let mut file = fs::File::create(dest)?;
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Download(e.to_string()))?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        // Throttle progress events to ~every 4 MB to avoid flooding the frontend.
        if downloaded - last_emit >= 4_000_000 || (total > 0 && downloaded >= total) {
            last_emit = downloaded;
            let percentage = if total > 0 {
                downloaded as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                events::EVT_MODEL_DOWNLOAD_PROGRESS,
                ModelDownloadProgressPayload {
                    model_id: model_id.to_string(),
                    downloaded,
                    total,
                    percentage,
                },
            );
        }
    }
    file.flush()?;
    Ok(())
}

fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got_hex = hex_lower(&hasher.finalize());
    if got_hex != expected_hex {
        // Delete the corrupt file so a retry re-downloads from scratch.
        let _ = fs::remove_file(path);
        return Err(AppError::Download(format!(
            "checksum mismatch (got {got_hex}, expected {expected_hex}) — file deleted, please retry"
        )));
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn extract_tar_gz(archive: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive)?;
    let gz = GzDecoder::new(file);
    let mut tar = Archive::new(gz);
    tar.unpack(dest_dir)
        .map_err(|e| AppError::Download(format!("extract: {e}")))?;
    Ok(())
}
