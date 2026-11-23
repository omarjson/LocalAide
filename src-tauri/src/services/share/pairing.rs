//! The one-time pairing password for the local share server.
//!
//! A fresh random token is made when sharing starts. Only its SHA-256 hash
//! is kept in memory; the plain value goes back to the desktop exactly once
//! in the `pairing_url` / `pairing_token` fields of `ShareStarted`. Every
//! request to the server (except the small `/health` check) must present the
//! token as `?token=` or in an `X-LocalAide-Token` header. Nothing here talks
//! to the network.

use sha2::{Digest, Sha256};

use crate::app::error::AppError;

/// 24 random bytes, written as 48 lowercase hex characters.
pub const TOKEN_BYTES: usize = 24;
pub const TOKEN_HEX_LEN: usize = TOKEN_BYTES * 2;

/// The header the pairing token is accepted in.
pub const HEADER: &str = "X-LocalAide-Token";

/// The query parameter the pairing token is accepted in.
pub const QUERY: &str = "token";

/// Make a new pairing token from the operating system's random source.
pub fn generate_token() -> Result<String, AppError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|_| AppError::internal("No random source for the pairing token."))?;
    Ok(hex::encode(bytes))
}

/// Hash a token for in-memory storage. The plain value is never kept.
pub fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

/// Constant-time equality for two 32-byte hashes.
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Compare a presented token with a stored hash. Wrong shapes are refused
/// before hashing; matching hashes are compared in constant time.
pub fn token_matches(stored: &[u8; 32], presented: &str) -> bool {
    let t = presented.trim();
    if t.len() != TOKEN_HEX_LEN || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    constant_time_eq(stored, &hash_token(t))
}

/// Take `token` from a URL's query string. Hex tokens need no decoding.
pub fn token_from_query(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == QUERY).then(|| value.to_string())
    })
}

/// Build the one-time URL shown to the person pairing a device. The token
/// is a single query parameter; the base may be a loopback or tunnel URL.
pub fn pairing_url(base: &str, token: &str) -> String {
    format!("{}/?{}={}", base.trim_end_matches('/'), QUERY, token)
}
