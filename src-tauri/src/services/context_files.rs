//! Discover the small "agent instructions" files in the user's workspace
//! and indexed library roots, and concatenate them within a byte budget.
//!
//! These files (AGENTS.md, CLAUDE.md, .hermes.md, .cursorrules) describe how
//! an assistant should behave in a project. Reading them is local-only.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

pub const CONTEXT_FILE_NAMES: [&str; 4] = ["AGENTS.md", "CLAUDE.md", ".hermes.md", ".cursorrules"];
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024;
pub const HARD_MAX_BYTES: usize = 256 * 1024;

const MAX_DEPTH: usize = 4;
const MAX_FILES: usize = 256;
const SKIP_DIRS: [&str; 5] = [".git", "node_modules", "target", ".venv", "dist"];

/// One included file, with the number of bytes actually added.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextFileHit {
    pub path: String,
    pub bytes: usize,
}

/// The concatenated instructions and the files that contributed to them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextFilesResult {
    pub text: String,
    pub files: Vec<ContextFileHit>,
    /// True when files or bytes were left out because of the cap.
    pub truncated: bool,
}

/// Walk one root up to a bounded depth and collect matching instruction
/// files. Hidden build directories are skipped to keep the walk cheap.
fn discover(root: &Path, out: &mut BTreeSet<PathBuf>) {
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut seen_dirs = BTreeSet::new();
    while let Some((dir, depth)) = stack.pop() {
        if out.len() >= MAX_FILES {
            break;
        }
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !seen_dirs.insert(canonical) {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if depth < MAX_DEPTH && !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push((path, depth + 1));
                }
            } else if CONTEXT_FILE_NAMES.contains(&name.as_str()) {
                out.insert(path);
            }
        }
    }
}

fn read_text(path: &Path) -> Option<String> {
    fs::read(path)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Cut a string at a UTF-8 character boundary, never in the middle of a
/// character. Returns the piece and whether anything was left out.
fn capped_piece(content: &str, max_bytes: usize) -> (String, bool) {
    if content.len() <= max_bytes {
        return (content.to_string(), false);
    }
    let mut cut = max_bytes;
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    (content[..cut].to_string(), true)
}

/// Read the roots the library is indexing, straight from its DB. A missing
/// or unreadable DB simply means there are no indexed roots to add.
pub fn indexed_roots(library_db: &Path) -> Vec<PathBuf> {
    let Ok(conn) = Connection::open_with_flags(library_db, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare("SELECT path FROM library_folders") else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(params![], |row| row.get::<_, String>(0)) else {
        return Vec::new();
    };
    rows.filter_map(|row| row.ok())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Collect and concatenate instruction files under `roots`, stopping at
/// `max_bytes`. Results are sorted by path so the output is deterministic.
pub fn collect(roots: &[PathBuf], max_bytes: usize) -> Result<ContextFilesResult, AppError> {
    let mut found = BTreeSet::new();
    for root in roots {
        if root.is_dir() {
            discover(root, &mut found);
        }
    }

    let mut text = String::new();
    let mut files = Vec::new();
    let mut truncated = false;
    for path in found {
        let Some(content) = read_text(&path) else {
            continue;
        };
        let header = format!("\n<!-- {} -->\n", path.display());
        if !files.is_empty() && text.len() + header.len() >= max_bytes {
            truncated = true;
            continue;
        }
        let room = max_bytes.saturating_sub(text.len() + header.len());
        let (piece, cut) = capped_piece(&content, room);
        if cut {
            truncated = true;
        }
        text.push_str(&header);
        text.push_str(&piece);
        files.push(ContextFileHit {
            path: path.to_string_lossy().into_owned(),
            bytes: header.len() + piece.len(),
        });
        if text.len() >= max_bytes {
            truncated = true;
            break;
        }
    }
    Ok(ContextFilesResult {
        text,
        files,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capping_never_splits_a_character() {
        let (piece, cut) = capped_piece("aé中", 3);
        assert!(cut);
        assert!(piece.is_char_boundary(piece.len()));
        assert_eq!(piece, "aé");
    }

    #[test]
    fn collect_finds_and_bounds_instruction_files() {
        let dir = std::env::temp_dir().join(format!("la-context-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("AGENTS.md"), "be kind").unwrap();
        fs::write(dir.join("sub").join(".cursorrules"), "use tabs").unwrap();
        let result = collect(std::slice::from_ref(&dir), 1024).unwrap();
        assert_eq!(result.files.len(), 2);
        assert!(result.text.contains("be kind"));
        assert!(!result.truncated);
        let small = collect(std::slice::from_ref(&dir), 12).unwrap();
        assert!(small.truncated);
        let _ = fs::remove_dir_all(&dir);
    }
}
