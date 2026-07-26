//! Rate limiting for the ingest endpoint.
//!
//! Ingest auto-creates a project for any `project_id` it has not seen, with no
//! ceiling on how often. On an open instance that is unbounded row creation
//! from an unauthenticated endpoint; even with API keys, a leaked key can fill
//! the disk. Coverage uploads are a CI-shaped workload — a handful per minute
//! at most — so a limit costs legitimate users nothing.
//!
//! A fixed-window counter per client, held in memory. Deliberately not a
//! distributed or persistent limiter: this is one process with a SQLite file,
//! and the goal is to bound accidental or casual abuse, not to survive a
//! determined attacker with many source addresses.

use axum::http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Requests allowed per window, per client.
const DEFAULT_MAX_REQUESTS: u32 = 60;
const WINDOW: Duration = Duration::from_secs(60);

/// Cap on tracked clients, so the limiter cannot itself become the memory leak.
const MAX_TRACKED_CLIENTS: usize = 10_000;

static BUCKETS: std::sync::LazyLock<Mutex<HashMap<String, Bucket>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

struct Bucket {
    window_started: Instant,
    count: u32,
}

fn max_requests() -> u32 {
    std::env::var("OMNIVORE_INGEST_RATE_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_REQUESTS)
}

/// Check and record a request from `client`.
///
/// `Ok(())` to proceed; `Err` carries a ready-to-return 429. A limit of 0
/// disables the check entirely.
pub fn check(client: &str) -> Result<(), (StatusCode, String)> {
    let limit = max_requests();
    if limit == 0 {
        return Ok(());
    }

    let Ok(mut buckets) = BUCKETS.lock() else {
        // A poisoned lock must not take ingest down with it.
        return Ok(());
    };

    let now = Instant::now();
    buckets.retain(|_, b| now.duration_since(b.window_started) < WINDOW);

    if buckets.len() >= MAX_TRACKED_CLIENTS && !buckets.contains_key(client) {
        // Under a broad distributed flood, stop tracking new clients rather
        // than growing without bound. Existing limits still apply.
        tracing::warn!("Ingest rate limiter at capacity; not tracking new clients this window");
        return Ok(());
    }

    let bucket = buckets.entry(client.to_string()).or_insert(Bucket {
        window_started: now,
        count: 0,
    });

    if now.duration_since(bucket.window_started) >= WINDOW {
        bucket.window_started = now;
        bucket.count = 0;
    }

    bucket.count += 1;
    if bucket.count > limit {
        tracing::warn!(%client, count = bucket.count, "Ingest rate limit exceeded");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!("Rate limit exceeded: more than {limit} uploads per minute"),
        ));
    }

    Ok(())
}

/// Best-effort client identity for rate limiting.
///
/// Prefers `X-Forwarded-For` (self-hosted instances usually sit behind a
/// reverse proxy, where every peer address would otherwise be the proxy's).
/// That header is client-controlled and trivially spoofed, so this is a
/// throttle on accidents and casual abuse, not an authorization boundary —
/// which is what the API key is for.
pub fn client_key(headers: &HeaderMap, peer: Option<IpAddr>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| peer.map(|ip| ip.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests mutate the process-wide limit, so they must not overlap.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_limit<T>(limit: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: serialized by ENV_LOCK, and restored before returning.
        unsafe {
            match limit {
                Some(v) => std::env::set_var("OMNIVORE_INGEST_RATE_LIMIT", v),
                None => std::env::remove_var("OMNIVORE_INGEST_RATE_LIMIT"),
            }
        }
        let out = f();
        unsafe { std::env::remove_var("OMNIVORE_INGEST_RATE_LIMIT") };
        out
    }

    #[test]
    fn allows_traffic_under_the_limit() {
        with_limit(None, || {
            let client = "test-under-limit";
            for _ in 0..5 {
                assert!(check(client).is_ok());
            }
        });
    }

    #[test]
    fn blocks_traffic_over_the_limit() {
        with_limit(Some("3"), || {
            let client = "test-over-limit";
            assert!(check(client).is_ok());
            assert!(check(client).is_ok());
            assert!(check(client).is_ok());
            let blocked = check(client);
            assert!(blocked.is_err());
            assert_eq!(blocked.unwrap_err().0, StatusCode::TOO_MANY_REQUESTS);
        });
    }

    #[test]
    fn a_zero_limit_disables_the_check() {
        with_limit(Some("0"), || {
            let client = "test-disabled";
            for _ in 0..1000 {
                assert!(check(client).is_ok());
            }
        });
    }

    #[test]
    fn prefers_forwarded_for_over_peer_address() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        assert_eq!(
            client_key(&headers, Some("10.0.0.1".parse().unwrap())),
            "203.0.113.7"
        );
    }

    #[test]
    fn falls_back_to_peer_address() {
        let headers = HeaderMap::new();
        assert_eq!(
            client_key(&headers, Some("198.51.100.4".parse().unwrap())),
            "198.51.100.4"
        );
    }
}
