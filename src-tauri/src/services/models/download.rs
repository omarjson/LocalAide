use super::verify::verify_file_checksum;
use crate::app::error::AppError;
use futures::StreamExt;
use reqwest::header::RANGE;
use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub percent: f32,
    pub speed_bps: f64,
}

pub struct ModelDownloader {
    client: reqwest::Client,
}

impl Default for ModelDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelDownloader {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3600))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// Download a model file with streaming verification, atomic rename, and HTTP resume.
    /// Writes to `.part` file until download and SHA-256 integrity verification succeed.
    pub async fn download_atomic<F>(
        &self,
        url: &str,
        dest: &Path,
        expected_sha256: &str,
        cancel: Arc<AtomicBool>,
        mut on_progress: F,
    ) -> Result<PathBuf, AppError>
    where
        F: FnMut(DownloadProgress) + Send + 'static,
    {
        if let Some(parent) = dest.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::storage(format!(
                    "Failed to create model directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        // Part file path
        let part_file = dest.with_extension("part");

        // Check if existing part file can be resumed
        let mut offset = if part_file.exists() {
            fs::metadata(&part_file).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let mut request = self.client.get(url);
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Network request to {url} failed: {e}")))?;

        let status = response.status();
        let total_size = match response.content_length() {
            Some(len) if status == reqwest::StatusCode::PARTIAL_CONTENT => Some(len + offset),
            Some(len) => Some(len),
            None => None,
        };

        if status == reqwest::StatusCode::OK && offset > 0 {
            // Server did not support Range, restart from 0
            offset = 0;
        } else if !status.is_success() {
            if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                // Offset matches total file size or out of range, restart
                offset = 0;
            } else {
                return Err(AppError::internal(format!(
                    "Download returned HTTP status: {status}"
                )));
            }
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(offset == 0)
            .open(&part_file)
            .map_err(|e| {
                AppError::storage(format!(
                    "Failed to open part file {}: {e}",
                    part_file.display()
                ))
            })?;

        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .map_err(|e| AppError::storage(format!("Failed to seek part file: {e}")))?;
        }

        let mut downloaded = offset;
        let mut stream = response.bytes_stream();
        let mut last_progress_time = Instant::now();
        let mut bytes_since_last_progress = 0u64;

        while let Some(chunk_result) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                drop(file);
                return Err(AppError::cancelled());
            }

            let chunk = chunk_result.map_err(|e| {
                AppError::internal(format!("Network stream error during download: {e}"))
            })?;

            file.write_all(&chunk)
                .map_err(|e| AppError::storage(format!("Failed writing chunk to disk: {e}")))?;

            downloaded += chunk.len() as u64;
            bytes_since_last_progress += chunk.len() as u64;

            let elapsed = last_progress_time.elapsed();
            if elapsed >= Duration::from_millis(200) {
                let speed_bps = (bytes_since_last_progress as f64) / elapsed.as_secs_f64();
                let total = total_size.unwrap_or(downloaded);
                let percent = if total > 0 {
                    ((downloaded as f64 / total as f64) * 100.0) as f32
                } else {
                    0.0
                };

                on_progress(DownloadProgress {
                    bytes_downloaded: downloaded,
                    bytes_total: total,
                    percent,
                    speed_bps,
                });

                last_progress_time = Instant::now();
                bytes_since_last_progress = 0;
            }
        }

        file.flush()
            .map_err(|e| AppError::storage(format!("Failed to flush part file: {e}")))?;
        drop(file);

        // Verify SHA-256 before atomic rename
        if let Err(mismatch_err) = verify_file_checksum(&part_file, expected_sha256) {
            // Delete corrupt part file to guarantee no corrupted artifact remains
            let _ = fs::remove_file(&part_file);
            return Err(mismatch_err);
        }

        // Atomically rename .part to dest
        fs::rename(&part_file, dest).map_err(|e| {
            AppError::storage(format!(
                "Failed to rename verified part file to {}: {e}",
                dest.display()
            ))
        })?;

        Ok(dest.to_path_buf())
    }
}
