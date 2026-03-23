use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::storage::Database;

use crate::routes::health::start_time;

#[derive(Template)]
#[template(path = "health.html")]
struct HealthPage {
    status: String,
    version: String,
    uptime: String,
    db_size: String,
    project_count: i64,
    snapshot_count: i64,
    last_ingest: String,
}

pub async fn health_page(
    State(db): State<Database>,
) -> Result<Html<String>, StatusCode> {
    let uptime_secs = start_time().map(|t| t.elapsed().as_secs()).unwrap_or(0);

    let stats = db.get_health_stats().await.ok();

    let (project_count, snapshot_count, last_ingest_raw, db_size) = match stats {
        Some(s) => (
            s.project_count,
            s.snapshot_count,
            s.last_ingest,
            format_bytes(s.db_size_bytes),
        ),
        None => (0, 0, None, "unknown".into()),
    };

    let last_ingest = last_ingest_raw
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or(s)
        })
        .unwrap_or_else(|| "Never".into());

    let page = HealthPage {
        status: "Healthy".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        uptime: format_uptime(uptime_secs),
        db_size,
        project_count,
        snapshot_count,
        last_ingest,
    };

    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
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
