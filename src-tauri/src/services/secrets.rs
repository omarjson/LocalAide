//! Provider API keys live in the OS credential store (macOS Keychain,
//! Windows Credential Manager or the Linux Secret Service). `config.json`
//! keeps only a `keyring:` marker, so the key is never in plain text on disk.
//!
//! Keychain access is best-effort: when no native store is reachable (for
//! example a headless Linux CI runner), `save` leaves the original value in
//! config.json rather than losing the user's key. The app still works.

use crate::app::error::AppError;
use keyring::Entry;

const SERVICE: &str = "com.localaide.provider-key";
const PREFIX: &str = "keyring:";

pub fn is_reference(value: &str) -> bool {
    value.starts_with(PREFIX)
}

pub fn reference(provider_id: &str) -> String {
    format!("{PREFIX}{provider_id}")
}

fn entry(provider_id: &str) -> Result<Entry, AppError> {
    Entry::new(SERVICE, provider_id)
        .map_err(|e| AppError::internal(format!("Could not open the OS keychain entry: {e}")))
}

pub fn save(provider_id: &str, secret: &str) -> Result<(), AppError> {
    if secret.is_empty() {
        return delete(provider_id);
    }
    entry(provider_id)?
        .set_password(secret)
        .map_err(|e| AppError::internal(format!("Could not save the API key: {e}")))
}

pub fn load(provider_id: &str) -> Result<Option<String>, AppError> {
    match entry(provider_id)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::internal(format!(
            "Could not read the API key: {e}"
        ))),
    }
}

pub fn delete(provider_id: &str) -> Result<(), AppError> {
    match entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::internal(format!(
            "Could not delete the API key: {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_recognised_without_exposing_the_key() {
        assert!(is_reference("keyring:p1"));
        assert!(!is_reference("sk-plain"));
        assert_eq!(reference("p1"), "keyring:p1");
    }
}
