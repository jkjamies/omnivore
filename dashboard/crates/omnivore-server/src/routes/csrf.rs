//! CSRF protection for the HTML form endpoints.
//!
//! The settings pages submit ordinary `POST` forms that change global
//! thresholds, delete projects, and mint API keys. `SameSite=Lax` blocks the
//! obvious cross-site form post, and that is why this was not a critical gap —
//! but it is the *only* thing that was stopping it. Lax is a browser-side
//! default that a same-site subdomain, an older browser, or a future relaxation
//! of the routes' cookie settings would quietly remove, and the actions behind
//! these forms are destructive.
//!
//! ## Design: signed double-submit
//!
//! A token is a random value plus an HMAC over it, handed out in a cookie and
//! echoed in a hidden form field. A request is accepted only when both are
//! present, equal, and carry a valid signature. An attacker on another origin
//! can neither read the cookie (to copy it into their form) nor forge the
//! signature, so they cannot produce a matching pair.
//!
//! Signing means the server does not have to remember issued tokens, which
//! keeps this stateless across restarts and avoids another table.
//!
//! ## Applicability
//!
//! Only enforced when OAuth is enabled. With no authentication there is no
//! session for an attacker to ride, so demanding a token would add friction
//! (and break `curl`-driven setup) while protecting nothing.

use axum::http::StatusCode;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sha2::{Digest, Sha256};

pub const COOKIE_NAME: &str = "omnivore_csrf";
pub const FIELD_NAME: &str = "csrf_token";

/// Issue a token, returning it alongside the cookie that must be set with it.
///
/// The cookie is deliberately **not** `HttpOnly`: the double-submit pattern
/// requires the page to read it. It carries no authority on its own — it
/// authorises nothing without the session cookie, which stays `HttpOnly`.
pub fn issue(jar: CookieJar) -> (CookieJar, String) {
    // Reuse an existing valid token so multiple forms on a page, and multiple
    // tabs, all agree.
    if let Some(existing) = jar.get(COOKIE_NAME).map(|c| c.value().to_string()) {
        if verify_signature(&existing) {
            return (jar, existing);
        }
    }

    let token = mint();
    let cookie = Cookie::build((COOKIE_NAME, token.clone()))
        .path("/")
        .http_only(false)
        .secure(super::auth::cookie_secure())
        .same_site(SameSite::Lax)
        .build();

    (jar.add(cookie), token)
}

/// Validate the token submitted with a form against the cookie.
pub fn verify(jar: &CookieJar, submitted: Option<&str>) -> Result<(), (StatusCode, String)> {
    if !super::auth::oauth_enabled() {
        return Ok(());
    }

    let cookie = jar.get(COOKIE_NAME).map(|c| c.value().to_string());
    let (Some(cookie), Some(submitted)) = (cookie, submitted) else {
        return Err(csrf_error());
    };

    if !verify_signature(&cookie) || !constant_time_eq(&cookie, submitted) {
        return Err(csrf_error());
    }

    Ok(())
}

fn csrf_error() -> (StatusCode, String) {
    (
        StatusCode::FORBIDDEN,
        "CSRF check failed — reload the page and try again".to_string(),
    )
}

fn mint() -> String {
    let random = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let signature = sign(&random);
    format!("{random}.{signature}")
}

fn verify_signature(token: &str) -> bool {
    let Some((random, signature)) = token.split_once('.') else {
        return false;
    };
    constant_time_eq(&sign(random), signature)
}

/// HMAC-style tag over the random half.
///
/// Keyed by `OMNIVORE_SECRET_KEY` when set, so tokens survive a restart. With
/// no key configured we fall back to a per-process key: tokens are then
/// invalidated by a restart, which costs a page reload and is preferable to a
/// predictable constant.
fn sign(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"omnivore-csrf-v1");
    hasher.update(signing_key().as_bytes());
    hasher.update(value.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

fn signing_key() -> String {
    static PROCESS_KEY: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| uuid::Uuid::new_v4().to_string());

    std::env::var("OMNIVORE_SECRET_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| PROCESS_KEY.clone())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_token_verifies() {
        let token = mint();
        assert!(verify_signature(&token));
    }

    #[test]
    fn a_forged_token_does_not_verify() {
        // An attacker who can guess the random half still cannot sign it.
        assert!(!verify_signature("deadbeef.0000000000000000000000000000000"));
        assert!(!verify_signature("no-separator"));
        assert!(!verify_signature(""));
    }

    #[test]
    fn a_tampered_token_does_not_verify() {
        let token = mint();
        let (random, signature) = token.split_once('.').unwrap();
        let tampered = format!("{random}x.{signature}");
        assert!(!verify_signature(&tampered));
    }
}
