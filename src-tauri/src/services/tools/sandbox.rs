use crate::app::error::AppError;
use crate::domain::tool::FileInfo;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const MAX_READ_BYTES: u64 = 1024 * 1024; // 1 MB
pub const MAX_WRITE_BYTES: usize = 4 * 1024 * 1024; // 4 MB

#[derive(Debug, Clone)]
pub struct WorkspaceJail {
    canonical_root: PathBuf,
}

impl WorkspaceJail {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, AppError> {
        let root_path = root.as_ref();
        if !root_path.exists() {
            fs::create_dir_all(root_path).map_err(|e| {
                AppError::storage(format!(
                    "Failed to create workspace jail root at {}: {e}",
                    root_path.display()
                ))
            })?;
        }
        let canonical_root = fs::canonicalize(root_path).map_err(|e| {
            AppError::storage(format!(
                "Failed to canonicalize workspace root at {}: {e}",
                root_path.display()
            ))
        })?;
        Ok(Self { canonical_root })
    }

    pub fn root_path(&self) -> &Path {
        &self.canonical_root
    }

    /// Resolve and verify that the target path remains strictly within the workspace jail.
    /// Traversal attempts, external absolute paths, and escaping symlinks are rejected.
    pub fn resolve_path(&self, input: &str) -> Result<PathBuf, AppError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(AppError::sandbox_denied(input, "Empty path provided"));
        }

        let p = Path::new(trimmed);

        // Pre-check for lexical traversal escapes
        let mut depth: isize = 0;
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 && !p.is_absolute() {
                        return Err(AppError::sandbox_denied(
                            input,
                            "Path traversal attempt (..) escapes workspace jail",
                        ));
                    }
                }
                Component::Normal(_) => {
                    depth += 1;
                }
                _ => {}
            }
        }

        let target = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.canonical_root.join(p)
        };

        // If the path exists on disk, canonicalize it to resolve symlinks and '..'
        if target.exists() || fs::symlink_metadata(&target).is_ok() {
            // First check if it is a symlink
            if let Ok(meta) = fs::symlink_metadata(&target)
                && meta.file_type().is_symlink()
            {
                // Try to resolve symlink target
                match fs::canonicalize(&target) {
                    Ok(canonical_target) => {
                        if !canonical_target.starts_with(&self.canonical_root) {
                            return Err(AppError::sandbox_denied(
                                input,
                                "Symlink target points outside workspace jail",
                            ));
                        }
                        return Ok(canonical_target);
                    }
                    Err(e) => {
                        // Detect symlink loop
                        let is_loop = e.raw_os_error() == Some(62) // ELOOP macOS
                            || e.raw_os_error() == Some(40) // ELOOP Linux
                            || e.to_string().to_lowercase().contains("too many")
                            || e.to_string().to_lowercase().contains("loop");
                        if is_loop {
                            return Err(AppError::sandbox_denied(
                                input,
                                "Symlink loop detected",
                            ));
                        }
                        let err_msg = format!("Symlink resolution failed: {e}");
                        return Err(AppError::sandbox_denied(input, &err_msg));
                    }
                }
            }

            match fs::canonicalize(&target) {
                Ok(canonical) => {
                    if !canonical.starts_with(&self.canonical_root) {
                        return Err(AppError::sandbox_denied(
                            input,
                            "Path resolves outside workspace jail",
                        ));
                    }
                    Ok(canonical)
                }
                Err(e) => {
                    let is_loop = e.raw_os_error() == Some(62)
                        || e.raw_os_error() == Some(40)
                        || e.to_string().to_lowercase().contains("too many")
                        || e.to_string().to_lowercase().contains("loop");
                    if is_loop {
                        return Err(AppError::sandbox_denied(input, "Symlink loop detected"));
                    }
                    let err_msg = format!("Canonicalization failed: {e}");
                    Err(AppError::sandbox_denied(input, &err_msg))
                }
            }
        } else {
            // Path does not exist yet (e.g. writing a new file).
            // Find the closest existing ancestor directory and verify its canonical location.
            let mut ancestor = target.as_path();
            let mut uncreated_components = Vec::new();

            while !ancestor.exists() {
                if let Some(file_name) = ancestor.file_name() {
                    uncreated_components.push(file_name);
                }
                match ancestor.parent() {
                    Some(parent) => ancestor = parent,
                    None => break,
                }
            }

            let canonical_ancestor = fs::canonicalize(ancestor).map_err(|e| {
                let err_msg = format!("Ancestor directory cannot be canonicalized: {e}");
                AppError::sandbox_denied(input, &err_msg)
            })?;

            if !canonical_ancestor.starts_with(&self.canonical_root) {
                return Err(AppError::sandbox_denied(
                    input,
                    "Target ancestor directory resolves outside workspace jail",
                ));
            }

            // Reconstruct the verified path from canonical ancestor
            let mut final_path = canonical_ancestor;
            uncreated_components.reverse();
            for comp in uncreated_components {
                final_path.push(comp);
            }

            Ok(final_path)
        }
    }

    pub fn list_dir(&self, rel_path: Option<&str>) -> Result<Vec<FileInfo>, AppError> {
        let target_dir = match rel_path {
            Some(p) if !p.trim().is_empty() => self.resolve_path(p)?,
            _ => self.canonical_root.clone(),
        };

        if !target_dir.is_dir() {
            return Err(AppError::sandbox_denied(
                &target_dir.to_string_lossy(),
                "Target is not a directory",
            ));
        }

        let entries = fs::read_dir(&target_dir).map_err(|e| {
            AppError::storage(format!(
                "Failed to read directory {}: {e}",
                target_dir.display()
            ))
        })?;

        let mut results = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir = meta.is_dir();
            let size_bytes = if is_dir { 0 } else { meta.len() };
            let modified_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let rel_str = path
                .strip_prefix(&self.canonical_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());

            results.push(FileInfo {
                name,
                path: rel_str,
                is_dir,
                size_bytes,
                modified_at,
            });

            if results.len() >= 1000 {
                break;
            }
        }

        results.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(results)
    }

    pub fn read_file(&self, rel_path: &str) -> Result<(String, bool), AppError> {
        let path = self.resolve_path(rel_path)?;
        if !path.is_file() {
            return Err(AppError::sandbox_denied(
                rel_path,
                "Target path is not a regular file",
            ));
        }

        let meta = fs::metadata(&path).map_err(|e| {
            AppError::storage(format!("Failed to read file metadata for {rel_path}: {e}"))
        })?;

        let is_truncated = meta.len() > MAX_READ_BYTES;
        let read_len = if is_truncated {
            MAX_READ_BYTES as usize
        } else {
            meta.len() as usize
        };

        let mut file = fs::File::open(&path)
            .map_err(|e| AppError::storage(format!("Failed to open file {rel_path}: {e}")))?;

        let mut buffer = vec![0u8; read_len];
        file.read_exact(&mut buffer)
            .map_err(|e| AppError::storage(format!("Failed to read file {rel_path}: {e}")))?;

        // Guard against binary files: inspect initial 1024 bytes for null bytes
        let sample_len = buffer.len().min(1024);
        if buffer[..sample_len].contains(&0) {
            return Err(AppError::invalid_input(format!(
                "Cannot read binary file '{rel_path}' as text"
            )));
        }

        let content = String::from_utf8_lossy(&buffer).to_string();
        Ok((content, is_truncated))
    }

    pub fn write_file(&self, rel_path: &str, content: &str) -> Result<usize, AppError> {
        if content.len() > MAX_WRITE_BYTES {
            return Err(AppError::invalid_input(format!(
                "Payload size ({} bytes) exceeds maximum allowed write size ({} bytes)",
                content.len(),
                MAX_WRITE_BYTES
            )));
        }

        let target_path = self.resolve_path(rel_path)?;

        if let Some(parent) = target_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::storage(format!(
                    "Failed to create parent directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        // Atomic write: write to temp file then rename
        let temp_filename = format!(
            ".tmp_write_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let temp_path = target_path
            .parent()
            .unwrap_or(&self.canonical_root)
            .join(temp_filename);

        {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .map_err(|e| AppError::storage(format!("Failed to create temporary file: {e}")))?;

            file.write_all(content.as_bytes()).map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                AppError::storage(format!("Failed to write content: {e}"))
            })?;
            file.flush().map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                AppError::storage(format!("Failed to flush content: {e}"))
            })?;
        }

        fs::rename(&temp_path, &target_path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            AppError::storage(format!(
                "Failed to rename temporary file to {}: {e}",
                target_path.display()
            ))
        })?;

        Ok(content.len())
    }

    pub fn search_files(
        &self,
        query: &str,
        rel_path: Option<&str>,
    ) -> Result<Vec<String>, AppError> {
        let base_dir = match rel_path {
            Some(p) if !p.trim().is_empty() => self.resolve_path(p)?,
            _ => self.canonical_root.clone(),
        };

        let mut matches = Vec::new();
        let query_lower = query.to_lowercase();
        self.walk_and_search(&base_dir, &query_lower, &mut matches)?;
        Ok(matches)
    }

    fn walk_and_search(
        &self,
        dir: &Path,
        query: &str,
        matches: &mut Vec<String>,
    ) -> Result<(), AppError> {
        if matches.len() >= 200 {
            return Ok(());
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    self.walk_and_search(&path, query, matches)?;
                } else if meta.is_file() {
                    let rel = path
                        .strip_prefix(&self.canonical_root)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| name.clone());

                    if name.to_lowercase().contains(query) {
                        matches.push(rel);
                    } else if meta.len() <= 256 * 1024 {
                        // For small files, search content
                        if let Ok(content) = fs::read_to_string(&path)
                            && content.to_lowercase().contains(query)
                        {
                            matches.push(rel);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
