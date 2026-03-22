use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use omnivore_core::storage::Database;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct BadgeQuery {
    /// Which metric to display: "line" (default) or "branch"
    metric: Option<String>,
    /// Optional target filter (e.g., "JvmUnit", "AndroidInstrumented")
    target: Option<String>,
}

pub async fn badge(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Query(params): Query<BadgeQuery>,
) -> Response {
    let metric = params.metric.as_deref().unwrap_or("line");

    let snapshot = if let Some(target) = &params.target {
        db.get_latest_snapshot_by_target(&project_id, target).await.ok().flatten()
    } else {
        db.get_latest_snapshot(&project_id).await.ok().flatten()
    };

    // Resolve effective thresholds for badge colors
    let project = db.get_project(&project_id).await.ok().flatten();
    let global = db.get_global_settings().await.unwrap_or_default();
    let line_thresh = project.as_ref().and_then(|p| p.line_threshold).unwrap_or(global.default_line_threshold);
    let branch_thresh = project.as_ref().and_then(|p| p.branch_threshold).unwrap_or(global.default_branch_threshold);
    let line_warn = project.as_ref().and_then(|p| p.line_warn_threshold).unwrap_or(global.default_line_warn_threshold);
    let branch_warn = project.as_ref().and_then(|p| p.branch_warn_threshold).unwrap_or(global.default_branch_warn_threshold);

    let (label, pct, color) = match snapshot {
        Some(snap) => {
            let (label, rate, threshold, warn) = match metric {
                "branch" => ("branch coverage", snap.branch_rate, branch_thresh, branch_warn),
                _ => ("coverage", snap.line_rate, line_thresh, line_warn),
            };
            let pct = format!("{:.1}%", rate * 100.0);
            let color = if rate >= threshold {
                "#2ecc71"
            } else if rate >= warn {
                "#f39c12"
            } else {
                "#e74c3c"
            };
            (label, pct, color)
        }
        None => ("coverage", "unknown".to_string(), "#9e9e9e"),
    };

    let svg = render_badge(label, &pct, color);

    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
        ],
        svg,
    )
        .into_response()
}

fn render_badge(label: &str, value: &str, color: &str) -> String {
    // Approximate character widths for the font
    let label_width = label.len() as f32 * 6.5 + 12.0;
    let value_width = value.len() as f32 * 7.0 + 12.0;
    let total_width = label_width + value_width;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="20" role="img" aria-label="{label}: {value}">
  <title>{label}: {value}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="{total_width}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_width}" height="20" fill="#555"/>
    <rect x="{label_width}" width="{value_width}" height="20" fill="{color}"/>
    <rect width="{total_width}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">
    <text aria-hidden="true" x="{label_x}0" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)">{label}</text>
    <text x="{label_x}0" y="140" transform="scale(.1)">{label}</text>
    <text aria-hidden="true" x="{value_x}0" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)">{value}</text>
    <text x="{value_x}0" y="140" transform="scale(.1)">{value}</text>
  </g>
</svg>"##,
        total_width = total_width,
        label_width = label_width,
        value_width = value_width,
        color = color,
        label = label,
        value = value,
        label_x = (label_width / 2.0) as u32,
        value_x = (label_width + value_width / 2.0) as u32,
    )
}
