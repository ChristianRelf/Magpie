use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Generate a new random API token with the given prefix (e.g. `mgp`).
pub fn generate_token(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let body = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    format!("{prefix}_{body}")
}

/// SHA-256 hash of a token, hex encoded. Tokens are stored only as hashes.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Constant-time comparison of a presented token against a stored hash.
pub fn verify_token(token: &str, stored_hash: &str) -> bool {
    let computed = hash_token(token);
    let a = computed.as_bytes();
    let b = stored_hash.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Short non-secret prefix shown in the UI to identify a key.
pub fn token_prefix(token: &str) -> String {
    token.chars().take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let t = generate_token("mgp");
        assert!(t.starts_with("mgp_"));
        assert!(t.len() > 40);
        let h = hash_token(&t);
        assert!(verify_token(&t, &h));
        assert!(!verify_token("mgp_wrong", &h));
        assert_ne!(generate_token("mgp"), t);
    }
}
