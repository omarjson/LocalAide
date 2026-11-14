use super::verify::verify_file_checksum;
use crate::app::blocking::run_blocking;
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
        // No total timeout: a multi-gigabyte model can take hours on a slow link.
        // A stalled connection is what matters, so each read must keep arriving.
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(60))
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
        expected_size: u64,
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

        // Refuse early when the disk cannot hold the rest of the file.
        let needed = expected_size
            .saturating_sub(offset)
            .saturating_add(DISK_MARGIN_BYTES);
        if let Some(free) = free_space_bytes(dest)
            && free < needed
        {
            return Err(AppError::storage(format!(
                "Not enough free disk space. This model needs about {} GB, and {} GB are free.",
                gigabytes(needed),
                gigabytes(free)
            )));
        }

        let mut request = self.client.get(url);
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Network request to {url} failed: {e}")))?;

        let status = response.status();
        // 416 to a Range request: the part file already holds every byte. Check it
        // and move it into place, instead of writing the server's error page.
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && offset > 0 {
            return finish_partial_file(&part_file, dest, expected_sha256).await;
        }
        if !status.is_success() {
            return Err(AppError::internal(format!(
                "Download returned HTTP status: {status}"
            )));
        }
        let total_size = match response.content_length() {
            Some(len) if status == reqwest::StatusCode::PARTIAL_CONTENT => Some(len + offset),
            Some(len) => Some(len),
            None => None,
        };
        if status == reqwest::StatusCode::OK {
            // The server ignored the Range header and sent the whole file.
            offset = 0;
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

        finish_partial_file(&part_file, dest, expected_sha256).await
    }
}

/// Extra space kept free after a download, so the disk never fills mid-write.
const DISK_MARGIN_BYTES: u64 = 512 * 1024 * 1024;

/// Verifies a finished `.part` file on a worker thread, then moves it into place.
/// A file that fails the check is deleted, so it can never be used.
async fn finish_partial_file(
    part_file: &Path,
    dest: &Path,
    expected_sha256: &str,
) -> Result<PathBuf, AppError> {
    let checked = part_file.to_path_buf();
    let expected = expected_sha256.to_string();
    if let Err(e) = run_blocking(move || verify_file_checksum(&checked, &expected)).await {
        let _ = fs::remove_file(part_file);
        return Err(AppError::internal(format!(
            "The download did not pass its checksum, so it was removed. Start the download again. ({e})"
        )));
    }
    fs::rename(part_file, dest).map_err(|e| {
        AppError::storage(format!(
            "Failed to move the verified file to {}: {e}",
            dest.display()
        ))
    })?;
    Ok(dest.to_path_buf())
}

/// Free bytes on the disk that holds `path`, or None when the disk is unknown.
fn free_space_bytes(path: &Path) -> Option<u64> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| disk.available_space())
}

fn gigabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::finish_partial_file;
    use std::path::PathBuf;

    /// SHA-256 of the three bytes "abc".
    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn temp_pair(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("la-download-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (dir.join("model.part"), dir.join("model.gguf"))
    }

    #[test]
    fn a_verified_part_file_becomes_the_model() {
        let (part, dest) = temp_pair("ok");
        std::fs::write(&part, b"abc").unwrap();
        let moved = futures::executor::block_on(finish_partial_file(&part, &dest, ABC_SHA256));
        assert_eq!(moved.unwrap(), dest);
        assert!(dest.exists() && !part.exists());
        let _ = std::fs::remove_dir_all(dest.parent().unwrap());
    }

    #[test]
    fn a_damaged_part_file_is_removed() {
        let (part, dest) = temp_pair("bad");
        std::fs::write(&part, b"not the model").unwrap();
        let result = futures::executor::block_on(finish_partial_file(&part, &dest, ABC_SHA256));
        assert!(result.is_err());
        assert!(!part.exists() && !dest.exists());
        let _ = std::fs::remove_dir_all(dest.parent().unwrap());
    }
}
