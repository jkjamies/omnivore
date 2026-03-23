use axum::extract::State;
use axum::Json;
use omnivore_core::storage::Database;
use serde::Serialize;

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub fn init_uptime() {
    START_TIME.get_or_init(std::time::Instant::now);
}

pub fn start_time() -> Option<std::time::Instant> {
    START_TIME.get().copied()
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub project_count: i64,
    pub snapshot_count: i64,
    pub last_ingest: Option<String>,
    pub db_size: String,
}

pub async fn health(State(db): State<Database>) -> Json<HealthResponse> {
    let uptime = START_TIME
        .get()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    let stats = db.get_health_stats().await.ok();

    let (project_count, snapshot_count, last_ingest, db_size) = match stats {
        Some(s) => (
            s.project_count,
            s.snapshot_count,
            s.last_ingest,
            format_bytes(s.db_size_bytes),
        ),
        None => (0, 0, None, "unknown".into()),
    };

    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        project_count,
        snapshot_count,
        last_ingest,
        db_size,
    })
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
