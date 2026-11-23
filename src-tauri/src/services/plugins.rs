//! Local plugin registry. A plugin is a plain `.html` file a person drops
//! into `~/.aiassistant/plugins`. Rust only lists the files; it never reads
//! them as code and never executes anything in them. Rendering (if any) is
//! the UI's job, inside its own sandbox.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::app::error::AppError;
use crate::app::paths::app_dir;

/// At most this many plugins are returned in one listing.
pub const MAX_PLUGINS: usize = 64;
/// Files larger than this are skipped rather than read.
pub const MAX_PLUGIN_BYTES: u64 = 2 * 1024 * 1024;

/// One plugin, as sent to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plugin {
    /// Stable identifier derived from the file name.
    pub id: String,
    /// Human-readable name derived from the file name.
    pub label: String,
    /// Absolute path of the html file, for the UI to open in a sandbox.
    pub path: String,
}

/// The folder scanned for plugins: `~/.aiassistant/plugins`.
pub fn plugins_dir() -> PathBuf {
    app_dir().join("plugins")
}

/// Stable id from a file stem: lowercase ASCII letters/digits, with runs of
/// anything else collapsed to one hyphen. Non-ASCII names get no id here;
/// the caller skips those files so ids stay predictable across machines.
pub fn id_from_stem(stem: &str) -> String {
    if !stem.is_ascii() {
        return String::new();
    }
    let mut out = String::with_capacity(stem.len());
    let mut pending_dash = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out
}

/// "my-plugin" → "My plugin". Only cosmetic; the id is the stable key.
pub fn label_from_stem(stem: &str) -> String {
    let spaced = stem.replace(['-', '_'], " ");
    let collapsed = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Pure path check: is `target` a child of `base`? Both must already be
/// absolute, canonical paths. Used to reject `..` and escaping symlinks.
pub fn contained(base: &Path, target: &Path) -> bool {
    target.starts_with(base) && target != base
}

/// Resolve one candidate, rejecting anything outside the plugin folder or
/// too large to be a plugin. Symlinks are followed only after their target
/// is confirmed to live inside the folder.
fn safe_file(dir: &Path, entry: &Path) -> Option<PathBuf> {
    let base = fs::canonicalize(dir).ok()?;
    let target = fs::canonicalize(entry).ok()?;
    if !contained(&base, &target) || target == base {
        return None;
    }
    let meta = fs::metadata(&target).ok()?;
    if !meta.is_file() || meta.len() > MAX_PLUGIN_BYTES {
        return None;
    }
    Some(target)
}

/// List the plugins in the user's plugin folder. A missing folder is an
/// empty list, not an error.
pub fn list() -> Result<Vec<Plugin>, AppError> {
    list_from(&plugins_dir())
}

/// Scan one folder. Split out so tests can point at a scratch directory.
pub fn list_from(dir: &Path) -> Result<Vec<Plugin>, AppError> {
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(AppError::storage(format!(
                "The plugin folder could not be read: {e}"
            )));
        }
    };
    let mut entries: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in entries {
        if out.len() >= MAX_PLUGINS {
            break;
        }
        let is_html = entry
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("html"));
        if !is_html {
            continue;
        }
        let Some(safe) = safe_file(dir, &entry) else {
            continue;
        };
        let stem = entry.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let id = id_from_stem(stem);
        if id.is_empty() || !seen.insert(id.clone()) {
            continue;
        }
        out.push(Plugin {
            label: label_from_stem(stem),
            id,
            path: safe.to_string_lossy().into_owned(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_labels_are_readable() {
        assert_eq!(id_from_stem("My Plugin"), "my-plugin");
        assert_eq!(label_from_stem("my-plugin"), "My plugin");
        assert_eq!(id_from_stem("notes_v2.html"), "notes-v2");
        assert_eq!(label_from_stem("notes_v2"), "Notes v2");
        assert_eq!(id_from_stem("  Odd  Name  "), "odd-name");
        assert_eq!(label_from_stem("  Odd  Name  "), "Odd Name");
        // Ids never leak path characters or case.
        assert_eq!(id_from_stem("A/../B"), "a-b");
        assert_eq!(id_from_stem(".."), "");
        assert_eq!(id_from_stem("café"), "");
    }

    #[test]
    fn path_validation_keeps_children_out_rejects_escape_and_self() {
        let base = Path::new("/home/me/.aiassistant/plugins");
        assert!(contained(base, &base.join("a.html")));
        assert!(contained(base, &base.join("nested").join("b.html")));
        assert!(!contained(base, base), "the folder itself is not a plugin");
        assert!(!contained(
            base,
            Path::new("/home/me/.aiassistant/other/a.html")
        ));
        assert!(!contained(base, Path::new("/etc/passwd")));
        // A sibling prefix is not a child either.
        assert!(!contained(
            base,
            Path::new("/home/me/.aiassistant/plugins-evil/a.html")
        ));
    }

    /// A symlink that resolves outside the folder must never be listed.
    #[cfg(unix)]
    #[test]
    fn escaping_symlinks_are_not_listed() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "localaide-plugins-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let dir = root.join("plugins");
        let outside = root.join("outside.html");
        let _ = fs::create_dir_all(&dir);
        fs::write(&outside, "<html>secret</html>").unwrap();
        symlink(&outside, dir.join("escape.html")).unwrap();
        fs::write(dir.join("good.html"), "<html>ok</html>").unwrap();

        let plugins = list_from(&dir).unwrap();
        assert_eq!(plugins.len(), 1, "only the real file is listed");
        assert_eq!(plugins[0].id, "good");
        assert!(plugins[0].path.ends_with("good.html"));

        let _ = fs::remove_dir_all(&root);
    }
}
