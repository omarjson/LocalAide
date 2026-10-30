use crate::app::error::AppError;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Verify file SHA-256 digest streaming in 64 KB chunks.
/// Uses bounded constant RAM (<100 KB) regardless of file size (multi-GB models).
pub fn compute_sha256_streaming(path: &Path) -> Result<String, AppError> {
    if !path.exists() {
        return Err(AppError::storage(format!(
            "File does not exist: {}",
            path.display()
        )));
    }

    let mut file = File::open(path).map_err(|e| {
        AppError::storage(format!(
            "Failed to open file for hashing {}: {e}",
            path.display()
        ))
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536]; // 64 KB buffer

    loop {
        let n = file.read(&mut buffer).map_err(|e| {
            AppError::storage(format!("Failed reading file chunk {}: {e}", path.display()))
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Verify that a file's SHA-256 matches the expected digest.
/// Returns Ok(()) if matched, Err(AppError::checksum_mismatch) otherwise.
pub fn verify_file_checksum(path: &Path, expected_sha256: &str) -> Result<(), AppError> {
    let actual = compute_sha256_streaming(path)?;
    if !expected_sha256.trim().eq_ignore_ascii_case(&actual) {
        return Err(AppError::checksum_mismatch(expected_sha256, &actual));
    }
    Ok(())
}
