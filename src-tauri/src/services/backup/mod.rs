//! Encrypted local backups (ADR-0009). One archive file holds the app's
//! user state — config, memory notes, conversations — encrypted with
//! AES-256-GCM under a key derived from a passphrase with argon2id. The
//! archive is a plain local file the user can copy anywhere (including a
//! OneDrive or Drive desktop-synced folder) without the app ever holding
//! cloud credentials. Cloud-provider targets slot in behind the same
//! engine once the operator registers an OAuth client (the credentials
//! are theirs to create); nothing here talks to the network.

use crate::app::error::AppError;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Version marker at the head of every archive file.
const MAGIC: &[u8] = b"LOCALAIDE-BACKUP-v1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const ARCHIVE_EXT: &str = "labak";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub file_name: String,
    pub created_at: u64,
    pub size_bytes: u64,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupPayload {
    pub created_at: u64,
    pub app_version: String,
    /// config.json contents. API keys are stripped unless explicitly
    /// included; a restored config never silently overwrites a newer one.
    pub config: serde_json::Value,
    pub memories: Vec<BackupFile>,
    pub conversations: Vec<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupFile {
    /// Path relative to the app folder (`memory/notes.md`,
    /// `conversations/{id}.json`).
    pub rel_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub memories_restored: usize,
    pub conversations_restored: usize,
    pub config_restored: bool,
    pub skipped_newer_config: bool,
}

pub struct BackupEngine {
    app_dir: PathBuf,
}

impl BackupEngine {
    pub fn new(app_dir: PathBuf) -> Self {
        Self { app_dir }
    }

    /// Collect the current user state into a payload.
    pub fn build_payload(
        &self,
        config: serde_json::Value,
        include_api_keys: bool,
    ) -> Result<BackupPayload, AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut config = config;
        if !include_api_keys
            && let Some(providers) = config
                .get_mut("cloud_providers")
                .and_then(|v| v.as_array_mut())
        {
            for provider in providers {
                if let Some(obj) = provider.as_object_mut() {
                    obj.insert(
                        "api_key".to_string(),
                        serde_json::Value::String(String::new()),
                    );
                }
            }
        }
        // Stamp the backup time inside the config so a restore can refuse
        // to clobber a config newer than the archive.
        if let Some(obj) = config.as_object_mut() {
            obj.insert("backup_taken_at".to_string(), serde_json::json!(now));
        }

        let memories = collect_dir(&self.app_dir.join("memory"), "memory")?;
        let conversations = collect_dir(&self.app_dir.join("conversations"), "conversations")?;

        Ok(BackupPayload {
            created_at: now,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            config,
            memories,
            conversations,
        })
    }

    /// Encrypt a payload into archive bytes.
    pub fn encrypt_payload(
        &self,
        payload: &BackupPayload,
        passphrase: &str,
    ) -> Result<Vec<u8>, AppError> {
        if passphrase.len() < 8 {
            return Err(AppError::invalid_input(
                "A backup passphrase needs at least 8 characters — there is no recovery without it",
            ));
        }
        let plaintext = serde_json::to_vec(payload)
            .map_err(|e| AppError::storage(format!("Could not serialize backup: {e}")))?;

        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::fill(&mut salt)
            .map_err(|_| AppError::internal("OS random source unavailable"))?;
        getrandom::fill(&mut nonce_bytes)
            .map_err(|_| AppError::internal("OS random source unavailable"))?;

        let key = derive_key(passphrase, &salt);
        let cipher = Aes256Gcm::new((&key).into());
        let nonce = Nonce::try_from(&nonce_bytes[..])
            .map_err(|_| AppError::internal("backup nonce construction failed"))?;
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &plaintext,
                    aad: MAGIC,
                },
            )
            .map_err(|_| AppError::internal("Backup encryption failed"))?;

        let mut out = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&salt);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypt archive bytes back into a payload.
    pub fn decrypt_payload(
        &self,
        bytes: &[u8],
        passphrase: &str,
    ) -> Result<BackupPayload, AppError> {
        let head = MAGIC.len() + SALT_LEN + NONCE_LEN;
        if bytes.len() <= head || !bytes.starts_with(MAGIC) {
            return Err(AppError::invalid_input(
                "This file is not a LocalAide backup",
            ));
        }
        let salt = &bytes[MAGIC.len()..MAGIC.len() + SALT_LEN];
        let nonce_bytes = &bytes[MAGIC.len() + SALT_LEN..head];
        let ciphertext = &bytes[head..];

        let key = derive_key(passphrase, salt);
        let cipher = Aes256Gcm::new((&key).into());
        let nonce = Nonce::try_from(nonce_bytes)
            .map_err(|_| AppError::invalid_input("Backup header is corrupted"))?;
        let plaintext = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: MAGIC,
                },
            )
            .map_err(|_| {
                AppError::invalid_input(
                    "Backup decryption failed — wrong passphrase or a corrupted file",
                )
            })?;
        serde_json::from_slice(&plaintext)
            .map_err(|e| AppError::invalid_input(format!("Backup content is invalid: {e}")))
    }

    /// Write a backup archive into `target_dir` and return its path.
    pub fn write_backup(
        &self,
        target_dir: &Path,
        payload: &BackupPayload,
        passphrase: &str,
    ) -> Result<PathBuf, AppError> {
        std::fs::create_dir_all(target_dir)
            .map_err(|e| AppError::storage(format!("Could not create the backup folder: {e}")))?;
        let bytes = self.encrypt_payload(payload, passphrase)?;
        let file_name = format!(
            "localaide-backup-{}.{}",
            timestamp_name(payload.created_at),
            ARCHIVE_EXT
        );
        let path = target_dir.join(file_name);
        std::fs::write(&path, &bytes)
            .map_err(|e| AppError::storage(format!("Could not write the backup file: {e}")))?;
        Ok(path)
    }

    /// List archive files in a folder, newest first.
    pub fn list_backups(&self, target_dir: &Path) -> Result<Vec<BackupEntry>, AppError> {
        let entries = match std::fs::read_dir(target_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(AppError::storage(format!(
                    "Could not read backup folder: {e}"
                )));
            }
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(ARCHIVE_EXT) {
                continue;
            }
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let created_at = parse_timestamp(&file_name).unwrap_or(0);
            out.push(BackupEntry {
                file_name,
                created_at,
                size_bytes,
                app_version: String::new(),
            });
        }
        out.sort_by_key(|item| std::cmp::Reverse(item.created_at));
        Ok(out)
    }

    /// Restore memories and conversations (always) and, when requested, the
    /// config — but never over a config that is newer than the backup's.
    pub fn restore(
        &self,
        payload: &BackupPayload,
        passphrase: &str,
        archive_bytes: &[u8],
        restore_config: bool,
    ) -> Result<RestoreReport, AppError> {
        // Verify the passphrase against this exact archive before touching
        // any file, so a wrong passphrase restores nothing at all.
        let verified = self.decrypt_payload(archive_bytes, passphrase)?;
        if verified.created_at != payload.created_at {
            return Err(AppError::invalid_input(
                "Backup file changed between listing and restore; retry",
            ));
        }

        let mut memories_restored = 0;
        for file in &payload.memories {
            let dest = self.app_dir.join(&file.rel_path);
            match dest.parent() {
                Some(parent) if parent.starts_with(&self.app_dir) => {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        AppError::storage(format!("Could not create restore folder: {e}"))
                    })?;
                }
                _ => continue,
            }
            std::fs::write(&dest, &file.content).map_err(|e| {
                AppError::storage(format!("Could not restore {}: {e}", file.rel_path))
            })?;
            memories_restored += 1;
        }

        let mut conversations_restored = 0;
        for file in &payload.conversations {
            let dest = self.app_dir.join(&file.rel_path);
            std::fs::create_dir_all(dest.parent().unwrap_or(&self.app_dir))
                .map_err(|e| AppError::storage(format!("Could not create restore folder: {e}")))?;
            std::fs::write(&dest, &file.content).map_err(|e| {
                AppError::storage(format!("Could not restore {}: {e}", file.rel_path))
            })?;
            conversations_restored += 1;
        }

        // Config restore is opt-in and refuses to clobber a newer file.
        let mut config_restored = false;
        let mut skipped_newer_config = false;
        if restore_config
            && let Some(config) = payload.config.as_object()
            && let Some(backup_time) = config.get("backup_taken_at").and_then(|v| v.as_u64())
        {
            let config_path = self.app_dir.join("config.json");
            let current_time = std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                .and_then(|v| v.get("backup_taken_at").and_then(|t| t.as_u64()))
                .unwrap_or(0);
            if backup_time >= current_time {
                std::fs::write(
                    &config_path,
                    serde_json::to_string_pretty(config).unwrap_or_default(),
                )
                .map_err(|e| AppError::storage(format!("Could not restore config: {e}")))?;
                config_restored = true;
            } else {
                skipped_newer_config = true;
            }
        }

        Ok(RestoreReport {
            memories_restored,
            conversations_restored,
            config_restored,
            skipped_newer_config,
        })
    }
}

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .expect("argon2 hash_password_into cannot fail with valid parameters");
    key
}

fn timestamp_name(secs: u64) -> String {
    // Compact UTC stamp, no timezone math: YYYYMMDD-HHMMSS from the epoch.
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days (Howard Hinnant's algorithm), stable and tiny.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!("{y:04}{mth:02}{d:02}-{h:02}{m:02}{s:02}")
}

fn parse_timestamp(file_name: &str) -> Option<u64> {
    // inverse of timestamp_name for "localaide-backup-YYYYMMDD-HHMMSS"
    let stamp = file_name
        .strip_prefix("localaide-backup-")?
        .strip_suffix(&format!(".{ARCHIVE_EXT}"))?;
    let (date, time) = stamp.split_once('-')?;
    if date.len() != 8 || time.len() != 6 {
        return None;
    }
    let y: i64 = date[..4].parse().ok()?;
    let mth: u32 = date[4..6].parse().ok()?;
    let d: u32 = date[6..8].parse().ok()?;
    let h: u64 = time[..2].parse().ok()?;
    let mi: u64 = time[2..4].parse().ok()?;
    let s: u64 = time[4..6].parse().ok()?;
    if !(1..=12).contains(&mth) || !(1..=31).contains(&d) {
        return None;
    }
    // Days-from-civil (Hinnant), inverse of the above.
    let y_adj = if mth <= 2 { y - 1 } else { y };
    let era = y_adj.div_euclid(400);
    let yoe = y_adj - era * 400;
    let mp = if mth > 2 { mth - 3 } else { mth + 9 };
    let doy = (153 * (mp as i64) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days as u64) * 86_400 + h * 3600 + mi * 60 + s)
}

fn collect_dir(dir: &Path, prefix: &str) -> Result<Vec<BackupFile>, AppError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::storage(format!("Could not read {prefix}: {e}"))),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // Only small text files are backed up; model and database files are
        // reproducible and far too large to embed here.
        if path.extension().and_then(|e| e.to_str()) != Some("json")
            && path.extension().and_then(|e| e.to_str()) != Some("md")
        {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|e| AppError::storage(e.to_string()))?;
        if meta.len() > 8 * 1024 * 1024 {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        out.push(BackupFile {
            rel_path: format!(
                "{prefix}/{}",
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ),
            content,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("la-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("memory")).unwrap();
        std::fs::create_dir_all(dir.join("conversations")).unwrap();
        dir
    }

    #[test]
    fn archive_roundtrip() {
        let dir = temp_app_dir();
        std::fs::write(dir.join("memory").join("note.md"), "# hi\n\ncontent").unwrap();
        std::fs::write(dir.join("conversations").join("t.json"), "{\"id\":1}").unwrap();
        let engine = BackupEngine::new(dir.clone());
        let payload = engine
            .build_payload(serde_json::json!({"selected_models": []}), false)
            .unwrap();
        assert_eq!(payload.memories.len(), 1);
        assert_eq!(payload.conversations.len(), 1);

        let bytes = engine
            .encrypt_payload(&payload, "long-enough-pass")
            .unwrap();
        assert!(bytes.starts_with(MAGIC));

        let back = engine.decrypt_payload(&bytes, "long-enough-pass").unwrap();
        assert_eq!(back, payload);
        assert!(engine.decrypt_payload(&bytes, "wrong-passphrase").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_keys_stripped_unless_included() {
        let dir = temp_app_dir();
        let engine = BackupEngine::new(dir.clone());
        let config = serde_json::json!({
            "cloud_providers": [{"id": "p1", "api_key": "sk-secret"}]
        });
        let stripped = engine.build_payload(config.clone(), false).unwrap();
        assert_eq!(
            stripped.config["cloud_providers"][0]["api_key"]
                .as_str()
                .unwrap(),
            ""
        );
        let kept = engine.build_payload(config, true).unwrap();
        assert_eq!(kept.config["cloud_providers"][0]["api_key"], "sk-secret");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_list_and_restore() {
        let dir = temp_app_dir();
        std::fs::write(dir.join("memory").join("n.md"), "note body").unwrap();
        let engine = BackupEngine::new(dir.clone());
        let payload = engine.build_payload(serde_json::json!({}), false).unwrap();
        let target = dir.parent().unwrap().join("la-backup-target");
        let path = engine
            .write_backup(&target, &payload, "long-enough-pass")
            .unwrap();
        assert!(path.is_file());

        let listed = engine.list_backups(&target).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].created_at > 0);

        // Restore into a fresh app dir.
        let fresh = dir.parent().unwrap().join("la-backup-fresh");
        std::fs::create_dir_all(&fresh).unwrap();
        let fresh_engine = BackupEngine::new(fresh.clone());
        let bytes = std::fs::read(&path).unwrap();
        let decoded = fresh_engine
            .decrypt_payload(&bytes, "long-enough-pass")
            .unwrap();
        let report = fresh_engine
            .restore(&decoded, "long-enough-pass", &bytes, false)
            .unwrap();
        assert_eq!(report.memories_restored, 1);
        assert!(fresh.join("memory").join("n.md").is_file());
        assert!(!report.config_restored);

        let _ = std::fs::remove_dir_all(&target);
        let _ = std::fs::remove_dir_all(&fresh);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamp_roundtrip() {
        for secs in [0u64, 86_399, 1_000_000_000, 1_794_620_687] {
            let stamp = timestamp_name(secs);
            assert_eq!(
                parse_timestamp(&format!("localaide-backup-{stamp}.labak")),
                Some(secs)
            );
        }
    }
}
