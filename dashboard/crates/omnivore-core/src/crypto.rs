//! Encryption for secrets held at rest in SQLite.
//!
//! The only secret stored today is a user's GitHub OAuth access token, kept for
//! the life of their session so the dashboard can make API calls on their
//! behalf. In plaintext that makes `omnivore.db` a credential store: a backup, a
//! stray volume mount, or a `docker cp` yields working GitHub tokens for every
//! logged-in user. Reducing the default OAuth scope limits what those tokens can
//! do, but does not make them safe to leave lying around.
//!
//! ## Design
//!
//! ChaCha20-Poly1305 with a random 96-bit nonce per record, keyed by
//! `OMNIVORE_SECRET_KEY`. AEAD rather than a bare stream cipher so a tampered
//! ciphertext fails loudly instead of decrypting to garbage that then gets sent
//! to GitHub.
//!
//! Encryption is **opt-in**: without `OMNIVORE_SECRET_KEY` values are stored as
//! before. A self-hosted instance must keep working after an upgrade without the
//! operator having to do anything first, and silently generating a key would be
//! worse than not encrypting — a key that lives only in memory invalidates every
//! session on restart, and one written to disk beside the database protects
//! against nothing.
//!
//! Stored values are tagged so both forms can coexist: ciphertext is
//! `omnivore:v1:<base64(nonce||ciphertext)>`, anything else is plaintext. That
//! makes enabling encryption a no-downtime change (old rows keep working, new
//! ones are encrypted) and disabling it survivable.

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};

const PREFIX: &str = "omnivore:v1:";
const NONCE_LEN: usize = 12;

/// Errors that mean a stored secret could not be recovered.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("secret is encrypted but OMNIVORE_SECRET_KEY is not set")]
    MissingKey,
    #[error("stored secret is malformed")]
    Malformed,
    #[error("stored secret failed authentication — wrong key, or the data was tampered with")]
    Undecryptable,
}

/// Is encryption configured?
pub fn is_enabled() -> bool {
    key_material().is_some()
}

fn key_material() -> Option<Key> {
    let raw = std::env::var("OMNIVORE_SECRET_KEY").ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    // Derive a 32-byte key by hashing, so an operator can supply a passphrase
    // of any length rather than having to produce exactly 32 bytes.
    let mut hasher = Sha256::new();
    hasher.update(b"omnivore-secret-key-v1");
    hasher.update(raw.as_bytes());
    Some(*Key::from_slice(&hasher.finalize()))
}

/// Encrypt a secret for storage. Returns the value unchanged when encryption is
/// not configured, so callers can always write the result directly.
pub fn encrypt(plaintext: &str) -> String {
    let Some(key) = key_material() else {
        return plaintext.to_string();
    };

    let cipher = ChaCha20Poly1305::new(&key);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).expect("system RNG unavailable");
    let nonce = Nonce::from_slice(&nonce_bytes);

    match cipher.encrypt(nonce, plaintext.as_bytes()) {
        Ok(ciphertext) => {
            let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
            payload.extend_from_slice(&nonce_bytes);
            payload.extend_from_slice(&ciphertext);
            format!(
                "{PREFIX}{}",
                base64::engine::general_purpose::STANDARD.encode(payload)
            )
        }
        Err(_) => {
            // Refusing to store the secret at all would lock the user out; a
            // failure here means a broken build, not a routine condition.
            tracing::error!("Failed to encrypt secret — storing unencrypted");
            plaintext.to_string()
        }
    }
}

/// Decrypt a stored secret. Untagged values are returned as-is, which is what
/// makes rows written before encryption was enabled keep working.
pub fn decrypt(stored: &str) -> Result<String, CryptoError> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        return Ok(stored.to_string());
    };

    let key = key_material().ok_or(CryptoError::MissingKey)?;
    let payload = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| CryptoError::Malformed)?;
    if payload.len() <= NONCE_LEN {
        return Err(CryptoError::Malformed);
    }

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new(&key);
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| CryptoError::Undecryptable)?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests mutate process-wide environment state, so they must not run
    /// concurrently with each other.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_key<T>(key: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            match key {
                Some(k) => std::env::set_var("OMNIVORE_SECRET_KEY", k),
                None => std::env::remove_var("OMNIVORE_SECRET_KEY"),
            }
        }
        let result = f();
        unsafe {
            std::env::remove_var("OMNIVORE_SECRET_KEY");
        }
        result
    }

    #[test]
    fn round_trips_with_a_key() {
        with_key(Some("correct horse battery staple"), || {
            let stored = encrypt("ghp_secret");
            assert!(stored.starts_with(PREFIX), "should be tagged: {stored}");
            assert!(!stored.contains("ghp_secret"), "plaintext must not survive");
            assert_eq!(decrypt(&stored).unwrap(), "ghp_secret");
        });
    }

    #[test]
    fn is_a_no_op_without_a_key() {
        with_key(None, || {
            assert_eq!(encrypt("ghp_secret"), "ghp_secret");
            assert_eq!(decrypt("ghp_secret").unwrap(), "ghp_secret");
        });
    }

    #[test]
    fn plaintext_rows_still_decrypt_after_enabling_encryption() {
        // The upgrade path: rows written before a key was configured must keep
        // working, or enabling encryption logs every existing user out.
        with_key(Some("k"), || {
            assert_eq!(decrypt("ghp_written_before_encryption").unwrap(), "ghp_written_before_encryption");
        });
    }

    #[test]
    fn encrypted_rows_fail_loudly_if_the_key_goes_away() {
        let stored = with_key(Some("k"), || encrypt("ghp_secret"));
        with_key(None, || {
            assert!(matches!(decrypt(&stored), Err(CryptoError::MissingKey)));
        });
    }

    #[test]
    fn wrong_key_is_rejected_rather_than_returning_garbage() {
        let stored = with_key(Some("right"), || encrypt("ghp_secret"));
        with_key(Some("wrong"), || {
            assert!(matches!(decrypt(&stored), Err(CryptoError::Undecryptable)));
        });
    }

    #[test]
    fn tampering_is_detected() {
        let stored = with_key(Some("k"), || encrypt("ghp_secret"));
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(stored.strip_prefix(PREFIX).unwrap())
            .unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let tampered = format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        with_key(Some("k"), || {
            assert!(matches!(decrypt(&tampered), Err(CryptoError::Undecryptable)));
        });
    }

    #[test]
    fn nonce_is_per_record() {
        with_key(Some("k"), || {
            assert_ne!(
                encrypt("same"),
                encrypt("same"),
                "identical plaintexts must not produce identical ciphertexts"
            );
        });
    }
}
