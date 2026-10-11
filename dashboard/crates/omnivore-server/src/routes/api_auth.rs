//! API-key authentication for the machine-facing write endpoints.
//!
//! These endpoints are called by CI, not by a browser, so they authenticate
//! with `X-API-Key` rather than a session cookie.
//!
//! ## Why this is its own module
//!
//! The rule used to live inline in the ingest handler and read "require a key
//! only if at least one key exists". That is convenient for a first run, but it
//! fails open in two ways worth naming:
//!
//! * A fresh dashboard accepts writes from anyone who can reach the port, and
//!   nothing ever tells the operator that.
//! * Deleting the last API key silently disables authentication for the whole
//!   instance, turning a routine bit of key rotation into a full exposure.
//!
//! The behaviour is kept for compatibility, but `OMNIVORE_REQUIRE_API_KEY=true`
//! makes it explicit and unconditional, and the open case now logs a warning on
//! every request instead of passing quietly.

use axum::http::{HeaderMap, StatusCode};
use omnivore_core::model::api_key::ApiKey;
use omnivore_core::storage::Database;

pub type ApiAuthError = (StatusCode, String);

/// Is unconditional API-key auth demanded by configuration?
pub fn api_key_required() -> bool {
    matches!(
        std::env::var("OMNIVORE_REQUIRE_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Authenticate a write request.
///
/// Returns `Ok(None)` when the instance is running in open mode (no keys
/// configured and no explicit requirement), and `Ok(Some(key))` when a valid
/// key was presented.
pub async fn authenticate_write(
    db: &Database,
    headers: &HeaderMap,
) -> Result<Option<ApiKey>, ApiAuthError> {
    let required = api_key_required()
        || db.any_api_keys_exist().await.map_err(|e| {
            tracing::error!(error = %e, "API key lookup failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?;

    if !required {
        tracing::warn!(
            "Unauthenticated write accepted: no API keys exist. \
             Create one under /settings, or set OMNIVORE_REQUIRE_API_KEY=true to fail closed."
        );
        return Ok(None);
    }

    let raw_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing X-API-Key header".to_string(),
            )
        })?;

    let key = db.validate_api_key(raw_key).await.map_err(|e| {
        tracing::error!(error = %e, "API key validation failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
    })?;

    key.map(Some).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Invalid API key".to_string(),
        )
    })
}

/// Reject a project-scoped key being used against a different project.
pub fn enforce_project_scope(
    key: Option<&ApiKey>,
    project_id: &str,
) -> Result<(), ApiAuthError> {
    let Some(scoped_to) = key.and_then(|k| k.project_id.as_deref()) else {
        return Ok(());
    };
    if scoped_to == project_id {
        return Ok(());
    }
    Err((
        StatusCode::FORBIDDEN,
        format!("API key is scoped to project '{scoped_to}', cannot write to '{project_id}'"),
    ))
}
