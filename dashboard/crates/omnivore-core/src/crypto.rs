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
    derive_key(&std::env::var("OMNIVORE_SECRET_KEY").ok()?)
}

/// Turn the configured passphrase into a key; `None` when it is blank.
fn derive_key(raw: &str) -> Option<Key> {
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
    encrypt_with(key_material().as_ref(), plaintext)
}

/// [`encrypt`] with an explicit key, so tests need not touch process env.
fn encrypt_with(key: Option<&Key>, plaintext: &str) -> String {
    let Some(key) = key else {
        return plaintext.to_string();
    };

    let cipher = ChaCha20Poly1305::new(key);
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
    decrypt_with(key_material().as_ref(), stored)
}

/// [`decrypt`] with an explicit key, so tests need not touch process env.
fn decrypt_with(key: Option<&Key>, stored: &str) -> Result<String, CryptoError> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        return Ok(stored.to_string());
    };

    let key = key.ok_or(CryptoError::MissingKey)?;
    let payload = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| CryptoError::Malformed)?;
    if payload.len() <= NONCE_LEN {
        return Err(CryptoError::Malformed);
    }

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new(key);
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| CryptoError::Undecryptable)?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Keys are passed in rather than set in the environment: process env is
    // shared by every test in the binary, and a lock only orders the tests
    // that remember to take it.
    fn key(passphrase: &str) -> Option<Key> {
        derive_key(passphrase)
    }

    #[test]
    fn round_trips_with_a_key() {
        let k = key("correct horse battery staple");
        let stored = encrypt_with(k.as_ref(), "ghp_secret");
        assert!(stored.starts_with(PREFIX), "should be tagged: {stored}");
        assert!(!stored.contains("ghp_secret"), "plaintext must not survive");
        assert_eq!(decrypt_with(k.as_ref(), &stored).unwrap(), "ghp_secret");
    }

    #[test]
    fn is_a_no_op_without_a_key() {
        assert_eq!(encrypt_with(None, "ghp_secret"), "ghp_secret");
        assert_eq!(decrypt_with(None, "ghp_secret").unwrap(), "ghp_secret");
        assert!(key("   ").is_none(), "a blank passphrase means no encryption");
    }

    #[test]
    fn plaintext_rows_still_decrypt_after_enabling_encryption() {
        // The upgrade path: rows written before a key was configured must keep
        // working, or enabling encryption logs every existing user out.
        assert_eq!(
            decrypt_with(key("k").as_ref(), "ghp_written_before_encryption").unwrap(),
            "ghp_written_before_encryption"
        );
    }

    #[test]
    fn encrypted_rows_fail_loudly_if_the_key_goes_away() {
        let stored = encrypt_with(key("k").as_ref(), "ghp_secret");
        assert!(matches!(decrypt_with(None, &stored), Err(CryptoError::MissingKey)));
    }

    #[test]
    fn wrong_key_is_rejected_rather_than_returning_garbage() {
        let stored = encrypt_with(key("right").as_ref(), "ghp_secret");
        assert!(matches!(
            decrypt_with(key("wrong").as_ref(), &stored),
            Err(CryptoError::Undecryptable)
        ));
    }

    #[test]
    fn tampering_is_detected() {
        let stored = encrypt_with(key("k").as_ref(), "ghp_secret");
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(stored.strip_prefix(PREFIX).unwrap())
            .unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let tampered = format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        assert!(matches!(
            decrypt_with(key("k").as_ref(), &tampered),
            Err(CryptoError::Undecryptable)
        ));
    }

    #[test]
    fn nonce_is_per_record() {
        let k = key("k");
        assert_ne!(
            encrypt_with(k.as_ref(), "same"),
            encrypt_with(k.as_ref(), "same"),
            "identical plaintexts must not produce identical ciphertexts"
        );
    }
}
