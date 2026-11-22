//! Read-only "doctor" report for this machine.
//!
//! Every check is best-effort and local: the report is shown in the app and
//! never uploaded. Nothing is repaired or changed here; that is deliberate so
//! running the doctor cannot damage a user's setup.

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};

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
}
