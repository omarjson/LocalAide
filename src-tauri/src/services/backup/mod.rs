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
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use zeroize::Zeroizing;

/// Version marker at the head of every archive file.
/// Version 1 archives: magic, salt, nonce, ciphertext. The key came from
/// argon2's defaults (19 MiB, 2 passes). They can still be restored.
const MAGIC: &[u8] = b"LOCALAIDE-BACKUP-v1";
/// Version 2 archives: magic, the Argon2 settings (memory in KiB, passes,
/// lanes; little-endian u32 each), salt, nonce, ciphertext. The whole header
/// is authenticated by the cipher, so the settings cannot be changed unseen.
const MAGIC_V2: &[u8] = b"LOCALAIDE-BACKUP-v2";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Argon2id settings for one archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KdfSettings {
    memory_kib: u32,
    passes: u32,
    lanes: u32,
}

impl KdfSettings {
    /// What version 1 archives used (argon2's defaults at the time).
    const V1: Self = Self {
        memory_kib: 19 * 1024,
        passes: 2,
        lanes: 1,
    };
    /// New archives: 64 MiB and 3 passes.
    const CURRENT: Self = Self {
        memory_kib: 64 * 1024,
        passes: 3,
        lanes: 1,
    };

    /// A header may not ask for more than this, so a crafted file cannot make
    /// opening it use gigabytes of memory or minutes of work.
    fn within_limits(self) -> bool {
        self.memory_kib <= 1024 * 1024 && self.passes <= 10 && self.lanes <= 8
    }
}
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
        // Web search keys get the same treatment.
        if !include_api_keys
            && let Some(web) = config.get_mut("web_search").and_then(|v| v.as_object_mut())
        {
            for slot in ["exa_api_key", "firecrawl_api_key"] {
                web.insert(slot.to_string(), serde_json::Value::String(String::new()));
            }
        }
        if !include_api_keys
            && let Some(opencode) = config.get_mut("opencode").and_then(|v| v.as_object_mut())
        {
            opencode.insert("password".to_string(), serde_json::Value::Null);
        }
        // MCP header and environment values often hold tokens: keep the
        // names, empty the values.
        if !include_api_keys
            && let Some(servers) = config.get_mut("mcp_servers").and_then(|v| v.as_array_mut())
        {
            for server in servers {
                let Some(transport) = server.get_mut("transport").and_then(|t| t.as_object_mut())
                else {
                    continue;
                };
                for slot in ["env", "headers"] {
                    if let Some(map) = transport.get_mut(slot).and_then(|m| m.as_object_mut()) {
                        for value in map.values_mut() {
                            *value = serde_json::Value::String(String::new());
                        }
                    }
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
        // The plain text and the key are wiped from memory when dropped.
        let plaintext = Zeroizing::new(
            serde_json::to_vec(payload)
                .map_err(|e| AppError::storage(format!("Could not serialize backup: {e}")))?,
        );

        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::fill(&mut salt)
            .map_err(|_| AppError::internal("OS random source unavailable"))?;
        getrandom::fill(&mut nonce_bytes)
            .map_err(|_| AppError::internal("OS random source unavailable"))?;

        let kdf = KdfSettings::CURRENT;
        let mut header = Vec::with_capacity(MAGIC_V2.len() + 12 + SALT_LEN + NONCE_LEN);
        header.extend_from_slice(MAGIC_V2);
        for value in [kdf.memory_kib, kdf.passes, kdf.lanes] {
            header.extend_from_slice(&value.to_le_bytes());
        }
        header.extend_from_slice(&salt);
        header.extend_from_slice(&nonce_bytes);

        let key = derive_key(passphrase, &salt, kdf)?;
        let cipher = Aes256Gcm::new((&*key).into());
        let nonce = Nonce::try_from(&nonce_bytes[..])
            .map_err(|_| AppError::internal("backup nonce construction failed"))?;
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.as_slice(),
                    aad: &header,
                },
            )
            .map_err(|_| AppError::internal("Backup encryption failed"))?;

        let mut out = header;
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypt archive bytes back into a payload.
    pub fn decrypt_payload(
        &self,
        bytes: &[u8],
        passphrase: &str,
    ) -> Result<BackupPayload, AppError> {
        let not_a_backup = || AppError::invalid_input("This file is not a LocalAide backup");
        let version_2 = bytes.starts_with(MAGIC_V2);
        let (kdf, salt_at) = if version_2 {
            let at = MAGIC_V2.len();
            let read = |offset: usize| -> Option<u32> {
                let field: [u8; 4] = bytes.get(at + offset..at + offset + 4)?.try_into().ok()?;
                Some(u32::from_le_bytes(field))
            };
            let (Some(memory_kib), Some(passes), Some(lanes)) = (read(0), read(4), read(8)) else {
                return Err(not_a_backup());
            };
            let kdf = KdfSettings {
                memory_kib,
                passes,
                lanes,
            };
            if !kdf.within_limits() {
                return Err(AppError::invalid_input(
                    "This backup asks for more memory than LocalAide allows, so it was not opened.",
                ));
            }
            (kdf, at + 12)
        } else if bytes.starts_with(MAGIC) {
            (KdfSettings::V1, MAGIC.len())
        } else {
            return Err(not_a_backup());
        };
        let head = salt_at + SALT_LEN + NONCE_LEN;
        if bytes.len() <= head {
            return Err(not_a_backup());
        }
        let salt = &bytes[salt_at..salt_at + SALT_LEN];
        let nonce_bytes = &bytes[salt_at + SALT_LEN..head];
        let ciphertext = &bytes[head..];
        // Version 2 authenticates its whole header; version 1 only the magic.
        let aad = if version_2 { &bytes[..head] } else { MAGIC };

        let key = derive_key(passphrase, salt, kdf)?;
        let cipher = Aes256Gcm::new((&*key).into());
        let nonce = Nonce::try_from(nonce_bytes)
            .map_err(|_| AppError::invalid_input("Backup header is corrupted"))?;
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    &nonce,
                    Payload {
                        msg: ciphertext,
                        aad,
                    },
                )
                .map_err(|_| {
                    AppError::invalid_input(
                        "Backup decryption failed — wrong passphrase or a corrupted file",
                    )
                })?,
        );
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

        // Check every destination before writing anything, so a bad archive
        // restores nothing at all.
        let memory_dests = payload
            .memories
            .iter()
            .map(|f| restore_destination(&self.app_dir, &f.rel_path))
            .collect::<Result<Vec<_>, _>>()?;
        let conversation_dests = payload
            .conversations
            .iter()
            .map(|f| restore_destination(&self.app_dir, &f.rel_path))
            .collect::<Result<Vec<_>, _>>()?;

        let mut memories_restored = 0;
        for (file, dest) in payload.memories.iter().zip(&memory_dests) {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::storage(format!("Could not create restore folder: {e}"))
                })?;
            }
            std::fs::write(dest, &file.content).map_err(|e| {
                AppError::storage(format!("Could not restore {}: {e}", file.rel_path))
            })?;
            memories_restored += 1;
        }

        let mut conversations_restored = 0;
        for (file, dest) in payload.conversations.iter().zip(&conversation_dests) {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::storage(format!("Could not create restore folder: {e}"))
                })?;
            }
            std::fs::write(dest, &file.content).map_err(|e| {
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
            let current = std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
            let current_time = current
                .as_ref()
                .and_then(|v| v.get("backup_taken_at").and_then(|t| t.as_u64()))
                .unwrap_or(0);
            if backup_time >= current_time {
                let merged = merge_restored_config(current.as_ref(), &payload.config);
                let text = serde_json::to_string_pretty(&merged).map_err(AppError::from)?;
                // Same as ConfigStore::save: temp file, owner-only, then rename.
                let tmp = config_path.with_extension("json.tmp");
                std::fs::write(&tmp, text)
                    .and_then(|()| {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(
                                &tmp,
                                std::fs::Permissions::from_mode(0o600),
                            );
                        }
                        std::fs::rename(&tmp, &config_path)
                    })
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

/// Where one archived file may be written. The entry must be exactly
/// `memory/<name>` or `conversations/<name>`, with a `.md` or `.json` name, as
/// `collect_dir` writes them. `..`, absolute or drive paths, other folders and
/// other extensions are refused.
fn restore_destination(app_dir: &Path, rel_path: &str) -> Result<PathBuf, AppError> {
    let mut parts = Path::new(rel_path).components();
    let allowed = match (parts.next(), parts.next(), parts.next()) {
        (Some(Component::Normal(dir)), Some(Component::Normal(name)), None) => {
            let folder_ok = dir == OsStr::new("memory") || dir == OsStr::new("conversations");
            let ext = Path::new(name).extension();
            folder_ok && (ext == Some(OsStr::new("md")) || ext == Some(OsStr::new("json")))
        }
        _ => false,
    };
    if !allowed {
        return Err(AppError::invalid_input(format!(
            "The backup contains an unexpected file path: {rel_path}"
        )));
    }
    Ok(app_dir.join(rel_path))
}

/// Prepares a restored config for writing.
///
/// - A key the backup left out (an empty string) keeps the current key, when
///   the current provider has the same id and the same address.
/// - A provider whose address fails the base URL check is dropped.
/// - Online chat is switched off when its provider is gone.
fn merge_restored_config(
    current: Option<&serde_json::Value>,
    restored: &serde_json::Value,
) -> serde_json::Value {
    use serde_json::Value;

    let text =
        |value: &Value, field: &str| value.get(field).and_then(Value::as_str).map(str::to_string);
    let mut merged = restored.clone();
    if let Some(obj) = merged.as_object_mut() {
        let current_providers: Vec<Value> = current
            .and_then(|c| c.get("cloud_providers"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if let Some(providers) = obj.get_mut("cloud_providers").and_then(Value::as_array_mut) {
            providers.retain(|p| {
                text(p, "base_url")
                    .is_some_and(|url| crate::config::validate_base_url(&url).is_ok())
            });
            for provider in providers.iter_mut() {
                if text(&*provider, "api_key").is_some_and(|key| !key.is_empty()) {
                    continue;
                }
                let restored_id = text(&*provider, "id");
                let restored_url = text(&*provider, "base_url");
                let kept = current_providers
                    .iter()
                    .find(|&c| {
                        restored_id.is_some()
                            && text(c, "id") == restored_id
                            && text(c, "base_url") == restored_url
                    })
                    .and_then(|c| text(c, "api_key"))
                    .filter(|key| !key.is_empty());
                if let (Some(key), Some(slot)) = (kept, provider.as_object_mut()) {
                    slot.insert("api_key".to_string(), Value::String(key));
                }
            }
        }

        let active = obj
            .get("active_cloud_provider")
            .and_then(Value::as_str)
            .map(str::to_string);
        let active_exists = active.is_some_and(|id| {
            obj.get("cloud_providers")
                .and_then(Value::as_array)
                .is_some_and(|ps| {
                    ps.iter()
                        .any(|p| text(p, "id").as_deref() == Some(id.as_str()))
                })
        });
        if !active_exists {
            obj.insert("active_cloud_provider".to_string(), Value::Null);
            obj.insert("cloud_llm_enabled".to_string(), Value::Bool(false));
        }

        let current_web = current.and_then(|c| c.get("web_search")).cloned();
        if let Some(web) = obj.get_mut("web_search").and_then(Value::as_object_mut) {
            for slot in ["exa_api_key", "firecrawl_api_key"] {
                let empty = web
                    .get(slot)
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty);
                let kept = current_web
                    .as_ref()
                    .and_then(|w| text(w, slot))
                    .filter(|key| !key.is_empty());
                if empty && let Some(key) = kept {
                    web.insert(slot.to_string(), Value::String(key));
                }
            }
        }
    }
    merged
}

/// Derives the archive key with Argon2id. The key is wiped from memory when
/// it is dropped.
fn derive_key(
    passphrase: &str,
    salt: &[u8],
    kdf: KdfSettings,
) -> Result<Zeroizing<[u8; 32]>, AppError> {
    let params = Params::new(kdf.memory_kib, kdf.passes, kdf.lanes, Some(32))
        .map_err(|_| AppError::invalid_input("Backup header is corrupted"))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, &mut key[..])
        .map_err(|_| AppError::internal("The backup key could not be made"))?;
    Ok(key)
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

    fn temp_app_dir(tag: &str) -> PathBuf {
        // Tagged per test: cargo test runs tests in parallel threads, and a
        // shared dir would let one test's remove_dir_all delete another's files.
        let dir =
            std::env::temp_dir().join(format!("la-backup-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("memory")).unwrap();
        std::fs::create_dir_all(dir.join("conversations")).unwrap();
        dir
    }

    #[test]
    fn archive_roundtrip() {
        let dir = temp_app_dir("roundtrip");
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
        assert!(bytes.starts_with(MAGIC_V2));

        let back = engine.decrypt_payload(&bytes, "long-enough-pass").unwrap();
        assert_eq!(back, payload);
        assert!(engine.decrypt_payload(&bytes, "wrong-passphrase").is_err());

        // The Argon2 settings sit in the authenticated header: a changed
        // setting makes the archive fail to open.
        let mut tampered = bytes.clone();
        tampered[MAGIC_V2.len() + 4] ^= 1;
        assert!(
            engine
                .decrypt_payload(&tampered, "long-enough-pass")
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_1_archives_still_open() {
        let dir = temp_app_dir("v1");
        let engine = BackupEngine::new(dir.clone());
        let payload = engine
            .build_payload(serde_json::json!({"selected_models": []}), false)
            .unwrap();

        // Build a version 1 archive the way older releases did.
        let salt = [7u8; SALT_LEN];
        let nonce_bytes = [9u8; NONCE_LEN];
        let key = derive_key("long-enough-pass", &salt, KdfSettings::V1).unwrap();
        let cipher = Aes256Gcm::new((&*key).into());
        let nonce = Nonce::try_from(&nonce_bytes[..]).unwrap();
        let plaintext = serde_json::to_vec(&payload).unwrap();
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &plaintext,
                    aad: MAGIC,
                },
            )
            .unwrap();
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&salt);
        bytes.extend_from_slice(&nonce_bytes);
        bytes.extend_from_slice(&ciphertext);

        assert_eq!(
            engine.decrypt_payload(&bytes, "long-enough-pass").unwrap(),
            payload
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_header_asking_for_huge_memory_is_refused() {
        let engine = BackupEngine::new(temp_app_dir("huge"));
        let mut bytes = MAGIC_V2.to_vec();
        for value in [u32::MAX, 3, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[0u8; SALT_LEN + NONCE_LEN + 32]);
        let err = engine
            .decrypt_payload(&bytes, "long-enough-pass")
            .unwrap_err();
        assert!(err.message.contains("more memory"));
    }

    #[test]
    fn api_keys_stripped_unless_included() {
        let dir = temp_app_dir("api-keys");
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
        let dir = temp_app_dir("restore");
        std::fs::write(dir.join("memory").join("n.md"), "note body").unwrap();
        let engine = BackupEngine::new(dir.clone());
        let payload = engine.build_payload(serde_json::json!({}), false).unwrap();
        let target = dir.parent().unwrap().join("la-backup-target");
        // Clear leftovers so the single-archive assertion holds on reruns.
        let _ = std::fs::remove_dir_all(&target);
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

#[cfg(test)]
mod restore_path_tests {
    use super::restore_destination;
    use std::path::Path;

    #[test]
    fn restored_config_keeps_current_keys_and_drops_bad_addresses() {
        let current = serde_json::json!({
            "cloud_providers": [
                {"id": "p1", "base_url": "https://api.example.com/v1", "api_key": "sk-current"},
                {"id": "p2", "base_url": "https://old.example.com/v1", "api_key": "sk-old"}
            ],
            "web_search": {"exa_api_key": "exa-current", "firecrawl_api_key": null}
        });
        let restored = serde_json::json!({
            "cloud_llm_enabled": true,
            "active_cloud_provider": "p3",
            "cloud_providers": [
                {"id": "p1", "base_url": "https://api.example.com/v1", "api_key": ""},
                {"id": "p2", "base_url": "https://moved.example.com/v1", "api_key": ""},
                {"id": "p3", "base_url": "http://192.168.1.20/v1", "api_key": "sk-lan"}
            ],
            "web_search": {"exa_api_key": "", "firecrawl_api_key": ""}
        });

        let merged = super::merge_restored_config(Some(&current), &restored);
        let providers = merged["cloud_providers"].as_array().unwrap();
        // Same id and address: the current key stays.
        assert_eq!(providers[0]["api_key"], "sk-current");
        // The address moved: the old key does not follow it.
        assert_eq!(providers[1]["api_key"], "");
        // Plain http to another machine fails the base URL check.
        assert_eq!(providers.len(), 2);
        // Its provider is gone, so online chat is off.
        assert_eq!(merged["cloud_llm_enabled"], false);
        assert!(merged["active_cloud_provider"].is_null());
        assert_eq!(merged["web_search"]["exa_api_key"], "exa-current");
        assert_eq!(merged["web_search"]["firecrawl_api_key"], "");
    }

    #[test]
    fn restore_accepts_only_memory_and_conversation_files() {
        let app = Path::new("/app");
        assert!(restore_destination(app, "memory/notes.md").is_ok());
        assert!(restore_destination(app, "conversations/t_1.json").is_ok());
        for bad in [
            "memory/../../escape.md",
            "../escape.md",
            "/etc/passwd",
            "C:x.md",
            "memory/notes.txt",
            "other/a.md",
            "memory/a/b.md",
            "memory",
        ] {
            assert!(
                restore_destination(app, bad).is_err(),
                "{bad} must be refused"
            );
        }
    }
}
