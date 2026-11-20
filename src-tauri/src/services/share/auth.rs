//! Tokens for shared links. Made from the operating system's random source,
//! kept only as SHA-256 hashes after the links are shown, and compared in
//! constant time.

use sha2::{Digest, Sha256};

use crate::app::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Reads conversations.
    Viewer,
    /// Also chats, answers approvals for its own requests, runs the self-test.
    Remote,
}

pub struct Tokens {
    viewer: [u8; 32],
    remote: [u8; 32],
    /// Only until the links are shown.
    pub viewer_plain: String,
    pub remote_plain: String,
}

fn hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_token() -> Result<String, AppError> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes)
        .map_err(|_| AppError::internal("No random source for the share token."))?;
    Ok(hex::encode(bytes))
}

impl Tokens {
    pub fn generate() -> Result<Self, AppError> {
        let viewer_plain = random_token()?;
        let remote_plain = random_token()?;
        Ok(Self {
            viewer: hash(&viewer_plain),
            remote: hash(&remote_plain),
            viewer_plain,
            remote_plain,
        })
    }

    /// Drop the plain tokens; only hashes stay in memory.
    pub fn forget_plain(mut self) -> Self {
        self.viewer_plain.clear();
        self.remote_plain.clear();
        self
    }

    /// The role a presented token grants, if any.
    pub fn check(&self, presented: &str) -> Option<Role> {
        let t = presented.trim();
        if t.len() != 48 || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let h = hash(t);
        // Check both, so the time taken does not say which one matched.
        let is_remote = same(&h, &self.remote);
        let is_viewer = same(&h, &self.viewer);
        if is_remote {
            Some(Role::Remote)
        } else if is_viewer {
            Some(Role::Viewer)
        } else {
            None
        }
    }
}

/// The token from an `Authorization: Bearer …` header.
pub fn bearer(header: Option<&str>) -> Option<&str> {
    let h = header?.trim();
    h.strip_prefix("Bearer ")
        .or_else(|| h.strip_prefix("bearer "))
        .map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_grant_their_role_and_nothing_else() {
        let t = Tokens::generate().unwrap();
        let (v, r) = (t.viewer_plain.clone(), t.remote_plain.clone());
        assert_ne!(v, r);
        let t = t.forget_plain();
        assert!(t.viewer_plain.is_empty());
        assert_eq!(t.check(&v), Some(Role::Viewer));
        assert_eq!(t.check(&r), Some(Role::Remote));
        assert_eq!(t.check(""), None);
        assert_eq!(t.check(&v.to_uppercase()), None, "a changed token fails");
        // Swap the last digit for a different one.
        let last = if v.ends_with('0') { '1' } else { '0' };
        assert_eq!(t.check(&format!("{}{last}", &v[..47])), None);
        assert_eq!(bearer(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer(Some("Basic abc")), None);
    }
}
