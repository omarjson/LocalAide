//! Resolve `@path[:start-end]` references against a bounded set of roots.
//!
//! This is what lets a prompt point at a file in the user's workspace
//! without granting the window arbitrary filesystem access. Absolute paths,
//! `..` escapes, symlinks out of a root and sensitive files are all refused.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

pub const DEFAULT_MAX_BYTES: usize = 64 * 1024;
pub const HARD_MAX_BYTES: usize = 256 * 1024;

/// A parsed reference: a relative path and an optional 1-based line range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRef {
    pub path: String,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

/// Parse `@path` or `@path:start-end`.
///
/// The range is optional and 1-based and inclusive. The path is kept as
/// text here; validation of escapes happens in [`resolve`].
pub fn parse_reference(input: &str) -> Result<PathRef, AppError> {
    let rest = input
        .strip_prefix('@')
        .ok_or_else(|| AppError::invalid_input("A reference must start with '@'."))?
        .trim();
    if rest.is_empty() {
        return Err(AppError::invalid_input("The reference is empty."));
    }

    let mut path_part = rest;
    let mut range: Option<(usize, usize)> = None;
    if let Some(colon) = rest.rfind(':') {
        let tail = &rest[colon + 1..];
        if let Some((a, b)) = tail.split_once('-') {
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
            if digits(a) && digits(b) {
                let start: usize = a
                    .parse()
                    .map_err(|_| AppError::invalid_input("Invalid start line."))?;
                let end: usize = b
                    .parse()
                    .map_err(|_| AppError::invalid_input("Invalid end line."))?;
                if start == 0 || end == 0 || start > end {
                    return Err(AppError::invalid_input("Invalid line range."));
                }
                range = Some((start, end));
                path_part = &rest[..colon];
            }
        }
    }
    if path_part.is_empty() {
        return Err(AppError::invalid_input("The reference has no path."));
    }
    Ok(PathRef {
        path: path_part.to_string(),
        start: range.map(|r| r.0),
        end: range.map(|r| r.1),
    })
}

/// Validate that a reference path is a relative, non-escaping path.
fn validate_relative(path_text: &str) -> Result<&Path, AppError> {
    if path_text.is_empty() || path_text.as_bytes().contains(&0) {
        return Err(AppError::invalid_input("Invalid reference path."));
    }
    let path = Path::new(path_text);
    if path.is_absolute() {
        return Err(AppError::invalid_input(
            "Absolute references are not allowed.",
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                return Err(AppError::invalid_input(
                    "Parent directory ('..') is not allowed in a reference.",
                ));
            }
            _ => return Err(AppError::invalid_input("Invalid reference path.")),
        }
    }
    Ok(path)
}

/// Names/extensions that must never be read through a reference. Config and
/// catalog files hold provider keys; the others are private credentials.
fn is_sensitive(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.starts_with(".env") {
        return true;
    }
    const DENY_EXT: [&str; 5] = ["pem", "key", "p12", "pfx", "p8"];
    if let Some(ext) = path.extension().and_then(|e| e.to_str())
        && DENY_EXT.contains(&ext.to_ascii_lowercase().as_str())
    {
        return true;
    }
    const DENY_NAMES: [&str; 7] = [
        "id_rsa",
        "id_ed25519",
        "id_dsa",
        "config.json",
        "catalog-cache.json",
        "custom-models.json",
        "credentials",
    ];
    if DENY_NAMES.contains(&name.as_str()) {
        return true;
    }
    const DENY_COMPONENTS: [&str; 2] = [".git", "node_modules"];
    path.components().any(|component| match component {
        Component::Normal(part) => {
            let part = part.to_string_lossy().to_ascii_lowercase();
            DENY_COMPONENTS.contains(&part.as_str())
        }
        _ => false,
    })
}

/// The resolved text slice plus enough metadata for the UI to show it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefResult {
    /// Canonical path actually read, after symlink resolution.
    pub path: String,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    /// True when the byte cap cut the slice short.
    pub truncated: bool,
}

fn slice_lines(text: &str, start: usize, end: usize, max_bytes: usize) -> (String, bool) {
    let mut out = String::new();
    let mut truncated = false;
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        if line_no < start || line_no > end {
            continue;
        }
        if out.len() + line.len() + 1 > max_bytes {
            truncated = true;
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, truncated)
}

/// Resolve a reference against `roots`, returning only the requested lines.
/// The first root that contains the file wins; roots are otherwise unordered.
pub fn resolve(input: &str, roots: &[PathBuf], max_bytes: usize) -> Result<RefResult, AppError> {
    let parsed = parse_reference(input)?;
    let relative = validate_relative(&parsed.path)?;
    if is_sensitive(relative) {
        return Err(AppError::invalid_input(
            "That file is sensitive and cannot be referenced.",
        ));
    }

    let mut chosen: Option<PathBuf> = None;
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(base) = root.canonicalize() else {
            continue;
        };
        let Ok(canonical) = base.join(relative).canonicalize() else {
            continue;
        };
        // Canonicalizing first also defeats symlinks that point outside.
        if !canonical.starts_with(&base) || !canonical.is_file() {
            continue;
        }
        if is_sensitive(&canonical) {
            return Err(AppError::invalid_input(
                "That file is sensitive and cannot be referenced.",
            ));
        }
        chosen = Some(canonical);
        break;
    }
    let path = chosen.ok_or_else(|| {
        AppError::invalid_input("The reference did not resolve inside the allowed roots.")
    })?;

    let bytes = fs::read(&path)
        .map_err(|e| AppError::storage(format!("Could not read the referenced file: {e}")))?;
    let text = String::from_utf8_lossy(&bytes);
    let total_lines = text.lines().count();
    let last = total_lines.max(1);
    let start = parsed.start.unwrap_or(1).max(1);
    let end = parsed.end.unwrap_or(last).min(last);
    let (slice, truncated) = slice_lines(&text, start, end, max_bytes);

    Ok(RefResult {
        path: path.to_string_lossy().into_owned(),
        text: slice,
        start_line: start,
        end_line: end,
        total_lines,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_path_and_range() {
        let r = parse_reference("@src/main.rs:10-20").unwrap();
        assert_eq!(r.path, "src/main.rs");
        assert_eq!(r.start, Some(10));
        assert_eq!(r.end, Some(20));
    }

    #[test]
    fn parses_path_without_range() {
        let r = parse_reference("@docs/readme.md").unwrap();
        assert_eq!(r.path, "docs/readme.md");
        assert_eq!(r.start, None);
        assert_eq!(r.end, None);
    }

    #[test]
    fn rejects_bad_references() {
        assert!(parse_reference("src/main.rs").is_err());
        assert!(parse_reference("@").is_err());
        assert!(parse_reference("@a.txt:5-2").is_err());
        assert!(parse_reference("@a.txt:0-3").is_err());
        assert!(parse_reference("@a.txt:x-y").is_err());
    }

    #[test]
    fn rejects_escapes_and_sensitive_names() {
        assert!(validate_relative("/etc/passwd").is_err());
        assert!(validate_relative("../secret").is_err());
        assert!(validate_relative("a/../../b").is_err());
        assert!(is_sensitive(Path::new(".env")));
        assert!(is_sensitive(Path::new("config.json")));
        assert!(is_sensitive(Path::new("keys/private.pem")));
        assert!(!is_sensitive(Path::new("src/main.rs")));
    }

    #[test]
    fn resolves_a_line_slice() {
        let dir = std::env::temp_dir().join(format!("la-context-refs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("note.txt"), "one\ntwo\nthree\nfour\n").unwrap();
        let r = resolve("@note.txt:2-3", std::slice::from_ref(&dir), 1024).unwrap();
        assert_eq!(r.text, "two\nthree\n");
        assert_eq!(r.total_lines, 4);
        assert!(resolve("@/etc/passwd", std::slice::from_ref(&dir), 1024).is_err());
        assert!(resolve("@.env", std::slice::from_ref(&dir), 1024).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
