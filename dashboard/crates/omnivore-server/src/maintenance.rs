//! Periodic housekeeping.
//!
//! Several tables grow without bound and had prune functions that nothing ever
//! called: `prune_permission_cache` was dead code, expired sessions were only
//! cleared when someone happened to log in, and `source_cache` — which holds the
//! largest rows in the database — was never cleaned at all. On a long-lived
//! self-hosted instance that is a slow disk leak whose first symptom is the
//! machine filling up.
//!
//! Housekeeping is best-effort by design: a failure here must never take the
//! server down, so every error is logged and swallowed.

use omnivore_core::storage::Database;
use std::time::Duration;

/// How often to sweep. Everything pruned here has an hour-scale lifetime, so
/// sweeping more often would be busywork.
const SWEEP_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Spawn the background maintenance loop.
pub fn spawn(db: Database) {
    tokio::spawn(async move {
        // Sweep once shortly after startup so a restart clears whatever
        // accumulated while the process was down, then settle into the
        // interval.
        tokio::time::sleep(Duration::from_secs(30)).await;

        loop {
            sweep(&db).await;
            tokio::time::sleep(SWEEP_INTERVAL).await;
        }
    });
}

async fn sweep(db: &Database) {
    match db.prune_expired_sessions().await {
        Ok(()) => tracing::debug!("Pruned expired sessions"),
        Err(e) => tracing::warn!(error = %e, "Failed to prune expired sessions"),
    }

    match db.prune_permission_cache().await {
        Ok(()) => tracing::debug!("Pruned permission cache"),
        Err(e) => tracing::warn!(error = %e, "Failed to prune permission cache"),
    }

    match db.prune_source_cache().await {
        Ok(0) => tracing::debug!("Source cache clean"),
        Ok(n) => tracing::info!(removed = n, "Pruned stale source cache entries"),
        Err(e) => tracing::warn!(error = %e, "Failed to prune source cache"),
    }
}
