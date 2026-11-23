//! Local "doctor" report and its opt-in repair path.
//!
//! `run()` is read-only and best-effort: the report is shown in the app and
//! never uploaded. Repair is a separate, explicit action (`fix` / `doctor_fix`)
//! that only performs safe changes — create a missing log directory, rebuild
//! a missing catalog cache from bundled data, and reopen the library DB. It
//! never deletes or overwrites user data.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::models::catalog::CatalogManifest;
use crate::services::models::refresh::CachedCatalog;

/// Name of the keychain probe check. It is best-effort: an unreachable OS
/// keychain does not make the overall report fail (headless Linux often has
/// no Secret Service, and the app still works with keys kept in config).
pub const KEYCHAIN_CHECK: &str = "keychain";

/// A single check in a [`DoctorReport`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorCheck {
    /// Stable machine-readable name, e.g. `config` or `library_db`.
    pub name: String,
    pub ok: bool,
    /// Plain-language outcome, safe to show in the UI.
    pub detail: String,
}

impl DoctorCheck {
    pub fn new(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ok,
            detail: detail.into(),
        }
    }
}

/// Serializable result of a doctor run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorReport {
    /// True when every required check passed. The best-effort keychain
    /// probe is intentionally excluded.
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    /// Build a report and compute `ok` from its checks.
    pub fn from_checks(checks: Vec<DoctorCheck>) -> Self {
        let ok = checks
            .iter()
            .filter(|check| check.name.as_str() != KEYCHAIN_CHECK)
            .all(|check| check.ok);
        Self { ok, checks }
    }
}

/// Read a JSON file and report whether it parses. A missing optional file is
/// not a failure; only an unreadable or malformed one is.
fn json_check(name: &'static str, path: &Path, missing_ok: bool) -> DoctorCheck {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(_) => DoctorCheck::new(name, true, format!("{} parses", path_label(path))),
            Err(e) => DoctorCheck::new(
                name,
                false,
                format!("{} is not valid JSON: {e}", path_label(path)),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DoctorCheck::new(
            name,
            missing_ok,
            if missing_ok {
                format!("{} absent (optional)", path_label(path))
            } else {
                format!("{} missing", path_label(path))
            },
        ),
        Err(e) => DoctorCheck::new(
            name,
            false,
            format!("Could not read {}: {e}", path_label(path)),
        ),
    }
}

fn path_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn config_check() -> DoctorCheck {
    json_check("config", &crate::app::paths::config_path(), true)
}

fn catalog_check() -> DoctorCheck {
    json_check(
        "catalog_cache",
        &crate::app::paths::catalog_cache_path(),
        true,
    )
}

fn library_check() -> DoctorCheck {
    let path = crate::app::paths::index_dir().join("library.db");
    if !path.is_file() {
        return DoctorCheck::new("library_db", false, "library.db is missing");
    }
    let opened = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY);
    match opened.and_then(|conn| {
        conn.query_row("SELECT count(*) FROM sqlite_master", params![], |row| {
            row.get::<_, i64>(0)
        })
    }) {
        Ok(_) => DoctorCheck::new("library_db", true, "library.db opens and reads"),
        Err(e) => DoctorCheck::new(
            "library_db",
            false,
            format!("library.db is unreadable: {e}"),
        ),
    }
}

fn log_dir_check() -> DoctorCheck {
    let dir = crate::app::paths::app_dir().join("logs");
    if dir.is_dir() {
        DoctorCheck::new("log_dir", true, "log directory is present")
    } else {
        DoctorCheck::new("log_dir", false, "log directory is missing")
    }
}

fn sidecar_check() -> DoctorCheck {
    let info = crate::services::engine::info();
    if info.installed {
        DoctorCheck::new("sidecar", true, "model engine binary found")
    } else {
        DoctorCheck::new("sidecar", false, "model engine binary is not installed")
    }
}

fn keychain_check() -> DoctorCheck {
    let probe = (|| -> Result<(), String> {
        let entry =
            keyring::Entry::new("com.localaide.doctor", "probe").map_err(|e| e.to_string())?;
        // NoEntry means the store answered, it simply has no probe secret.
        match entry.get_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    })();
    match probe {
        Ok(()) => DoctorCheck::new(KEYCHAIN_CHECK, true, "OS keychain reachable (best effort)"),
        Err(e) => DoctorCheck::new(
            KEYCHAIN_CHECK,
            false,
            format!("OS keychain unavailable (best effort): {e}"),
        ),
    }
}

/// Bundled catalog used to rebuild a missing cache. Same file the model
/// service compiles in, so a rebuilt cache always validates at startup.
const BUNDLED_CATALOG: &str = include_str!("../../../catalog/models.json");

/// Paths the repair touches. Kept explicit so tests can use a temp directory
/// instead of the user's real app folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairPaths {
    pub log_dir: PathBuf,
    pub catalog_cache: PathBuf,
    pub library_db: PathBuf,
}

impl RepairPaths {
    /// The real locations used by the app.
    pub fn defaults() -> Self {
        Self {
            log_dir: crate::app::paths::app_dir().join("logs"),
            catalog_cache: crate::app::paths::catalog_cache_path(),
            library_db: crate::app::paths::index_dir().join("library.db"),
        }
    }
}

/// What a repair run did. `fixed` lists changes actually made; `remaining`
/// lists safe issues that could not be repaired. Neither ever includes a
/// deletion — the repair path has no delete operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorFixReport {
    pub fixed: Vec<String>,
    pub remaining: Vec<String>,
}

/// Write a valid cache entry from bundled catalog data. A `CachedCatalog` is
/// the on-disk shape the model service reads; `fetched_at = 0` marks it as
/// the bundled baseline rather than a live fetch. Existing files are never
/// touched — the caller only invokes this when the cache is missing.
fn rebuild_catalog_cache(path: &Path, bundled: &str) -> Result<(), String> {
    let manifest: CatalogManifest =
        serde_json::from_str(bundled).map_err(|e| format!("bundled catalog is invalid: {e}"))?;
    manifest
        .verify()
        .map_err(|e| format!("bundled catalog failed verification: {e}"))?;
    let payload = CachedCatalog {
        manifest,
        fetched_at: 0,
    };
    let data = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Atomic write, matching the model service's own cache writer.
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data.as_bytes()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Open the library DB read-write (creating an empty one if it was missing)
/// and run a quick integrity check. No file is ever removed or rewritten.
fn reopen_library_db(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| e.to_string())?;
    let state: String = conn
        .query_row("PRAGMA quick_check", params![], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if state.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(format!("integrity check reported {state}"))
    }
}

/// Repair only the safe issues against explicit paths. Pure with respect to
/// the filesystem state it is given: no network, no deletion, no overwrite of
/// an existing cache or database.
pub fn repair_at(paths: &RepairPaths, bundled_catalog: &str) -> DoctorFixReport {
    let mut fixed = Vec::new();
    let mut remaining = Vec::new();

    if !paths.log_dir.is_dir() {
        match fs::create_dir_all(&paths.log_dir) {
            Ok(()) => fixed.push(format!(
                "created log directory {}",
                paths.log_dir.display()
            )),
            Err(e) => remaining.push(format!("could not create the log directory: {e}")),
        }
    }

    if !paths.catalog_cache.is_file() {
        match rebuild_catalog_cache(&paths.catalog_cache, bundled_catalog) {
            Ok(()) => fixed.push("recreated the catalog cache from bundled data".to_string()),
            Err(e) => remaining.push(format!("could not recreate the catalog cache: {e}")),
        }
    }

    // Reopening an already-healthy DB is harmless; it is the only way to
    // clear a transient lock or finish creating a missing one. Remember
    // whether it existed before the open so the report is accurate.
    let library_was_missing = !paths.library_db.is_file();
    match reopen_library_db(&paths.library_db) {
        Ok(()) => {
            if library_was_missing {
                fixed.push("created and opened the library database".to_string());
            } else {
                fixed.push("reopened and verified the library database".to_string());
            }
        }
        Err(e) => remaining.push(format!("could not reopen the library database: {e}")),
    }

    DoctorFixReport { fixed, remaining }
}

/// Repair the safe issues at the app's real paths, using the bundled catalog.
pub fn fix() -> DoctorFixReport {
    repair_at(&RepairPaths::defaults(), BUNDLED_CATALOG)
}

/// Explicit, opt-in repair command. The read-only `doctor_run` report is
/// unchanged; this command is the only one that changes local files, and it
/// never deletes user data.
#[tauri::command]
pub async fn doctor_fix() -> Result<DoctorFixReport, AppError> {
    run_blocking(|| Ok(fix())).await
}

/// Run all checks. Never returns an error: a failure is data in the report,
/// not a failed command.
pub fn run() -> DoctorReport {
    DoctorReport::from_checks(vec![
        config_check(),
        catalog_check(),
        library_check(),
        log_dir_check(),
        sidecar_check(),
        keychain_check(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_ok_ignores_best_effort_keychain() {
        let report = DoctorReport::from_checks(vec![
            DoctorCheck::new("config", true, "ok"),
            DoctorCheck::new(KEYCHAIN_CHECK, false, "no secret service"),
        ]);
        assert!(report.ok);
    }

    #[test]
    fn report_fails_on_required_check() {
        let report = DoctorReport::from_checks(vec![
            DoctorCheck::new("config", true, "ok"),
            DoctorCheck::new("library_db", false, "missing"),
        ]);
        assert!(!report.ok);
    }

    #[test]
    fn repair_creates_only_missing_safe_files() {
        let dir = std::env::temp_dir().join(format!("la-doctor-fix-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = RepairPaths {
            log_dir: dir.join("logs"),
            catalog_cache: dir.join("catalog-cache.json"),
            library_db: dir.join("index/library.db"),
        };
        let report = repair_at(&paths, BUNDLED_CATALOG);
        assert!(report.remaining.is_empty(), "{:?}", report.remaining);
        assert!(paths.log_dir.is_dir());
        assert!(paths.catalog_cache.is_file());
        assert!(paths.library_db.is_file());
        assert!(report.fixed.iter().any(|m| m.contains("log directory")));
        assert!(report.fixed.iter().any(|m| m.contains("catalog cache")));
        // The rebuilt file is a CachedCatalog the model service can load.
        let text = fs::read_to_string(&paths.catalog_cache).unwrap();
        let cached: CachedCatalog = serde_json::from_str(&text).unwrap();
        cached.manifest.verify().unwrap();
        assert_eq!(cached.fetched_at, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repair_never_overwrites_existing_user_data() {
        let dir = std::env::temp_dir().join(format!("la-doctor-fix-keep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let paths = RepairPaths {
            log_dir: dir.join("logs"),
            catalog_cache: dir.join("catalog-cache.json"),
            library_db: dir.join("index/library.db"),
        };
        fs::write(&paths.catalog_cache, "{\"user\":\"data\"}").unwrap();
        let report = repair_at(&paths, BUNDLED_CATALOG);
        assert_eq!(
            fs::read_to_string(&paths.catalog_cache).unwrap(),
            "{\"user\":\"data\"}"
        );
        assert!(
            !report.fixed.iter().any(|m| m.contains("catalog cache")),
            "{:?}",
            report.fixed
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
