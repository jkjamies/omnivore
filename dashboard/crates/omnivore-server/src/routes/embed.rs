use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use omnivore_core::storage::Database;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct EmbedQuery {
    /// Number of data points (default: 30)
    limit: Option<i64>,
    /// Metric: "line" (default) or "branch"
    metric: Option<String>,
    /// Target filter (e.g., "JvmUnit")
    target: Option<String>,
    /// Width in pixels (default: 400)
    width: Option<f64>,
    /// Height in pixels (default: 120)
    height: Option<f64>,
    /// Theme: "light" (default) or "dark"
    theme: Option<String>,
}

pub async fn trend_embed(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Query(params): Query<EmbedQuery>,
) -> Response {
    let limit = params.limit.unwrap_or(30);
    let metric = params.metric.as_deref().unwrap_or("line");
    let width = params.width.unwrap_or(400.0);
    let height = params.height.unwrap_or(120.0);
    let dark = params.theme.as_deref() == Some("dark");

    let snapshots = if let Some(target) = &params.target {
        db.get_snapshots_for_project_by_target(&project_id, target, limit)
            .await
            .unwrap_or_default()
    } else {
        db.get_snapshots_for_project(&project_id, limit)
            .await
            .unwrap_or_default()
    };

    let rates: Vec<f64> = snapshots
        .iter()
        .rev()
        .map(|s| match metric {
            "branch" => s.branch_rate,
            _ => s.line_rate,
        })
        .collect();

    // Resolve thresholds for color zones
    let project = db.get_project(&project_id).await.ok().flatten();
    let global = db.get_global_settings().await.unwrap_or_default();
    let (threshold, warn_threshold) = match metric {
        "branch" => (
            project.as_ref().and_then(|p| p.branch_threshold).unwrap_or(global.default_branch_threshold),
            project.as_ref().and_then(|p| p.branch_warn_threshold).unwrap_or(global.default_branch_warn_threshold),
        ),
        _ => (
            project.as_ref().and_then(|p| p.line_threshold).unwrap_or(global.default_line_threshold),
            project.as_ref().and_then(|p| p.line_warn_threshold).unwrap_or(global.default_line_warn_threshold),
        ),
    };

    let project_name = project
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or(&project_id);

    let svg = render_trend_svg(
        &rates,
        project_name,
        metric,
        threshold,
        warn_threshold,
        width,
        height,
        dark,
    );

    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
        ],
        svg,
    )
        .into_response()
}

fn render_trend_svg(
    rates: &[f64],
    project_name: &str,
    metric: &str,
    threshold: f64,
    warn_threshold: f64,
    width: f64,
    height: f64,
    dark: bool,
) -> String {
    let padding_top = 28.0;
    let padding_bottom = 22.0;
    let padding_left = 38.0;
    let padding_right = 12.0;
    let chart_w = width - padding_left - padding_right;
    let chart_h = height - padding_top - padding_bottom;

    // Colors
    let (bg, text_color, grid_color, line_color, fill_color) = if dark {
        ("#1a1d27", "#e4e6eb", "#2d3041", "#4361ee", "rgba(67,97,238,0.15)")
    } else {
        ("#ffffff", "#1a1a2e", "#dee2e6", "#4361ee", "rgba(67,97,238,0.1)")
    };
    let green = "#2ecc71";
    let yellow = "#f39c12";
    let red = "#e74c3c";

    if rates.is_empty() {
        return format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <rect width="{width}" height="{height}" fill="{bg}" rx="6"/>
  <text x="{cx}" y="{cy}" fill="{text_color}" font-family="system-ui,sans-serif" font-size="12" text-anchor="middle">No data</text>
</svg>"#,
            cx = width / 2.0,
            cy = height / 2.0,
        );
    }

    // Y-axis: 0% to 100%
    let y_min = 0.0_f64;
    let y_max = 1.0_f64;

    let to_x = |i: usize| -> f64 {
        if rates.len() == 1 {
            padding_left + chart_w / 2.0
        } else {
            padding_left + (i as f64 / (rates.len() - 1) as f64) * chart_w
        }
    };
    let to_y = |v: f64| -> f64 {
        padding_top + (1.0 - (v - y_min) / (y_max - y_min)) * chart_h
    };

    // Build polyline points
    let line_points: String = rates
        .iter()
        .enumerate()
        .map(|(i, &r)| format!("{:.1},{:.1}", to_x(i), to_y(r)))
        .collect::<Vec<_>>()
        .join(" ");

    // Build fill polygon (area under curve)
    let bottom_y = to_y(y_min);
    let first_x = to_x(0);
    let last_x = to_x(rates.len().saturating_sub(1));
    let fill_points = format!(
        "{:.1},{:.1} {} {:.1},{:.1}",
        first_x, bottom_y, line_points, last_x, bottom_y
    );

    // Current value
    let current = rates.last().copied().unwrap_or(0.0);
    let current_pct = format!("{:.1}%", current * 100.0);
    let current_color = if current >= threshold {
        green
    } else if current >= warn_threshold {
        yellow
    } else {
        red
    };

    // Threshold zone lines
    let thresh_y = to_y(threshold);
    let warn_y = to_y(warn_threshold);

    // Y-axis labels
    let y_labels = [0.0, 0.25, 0.5, 0.75, 1.0];

    let metric_label = match metric {
        "branch" => "Branch",
        _ => "Line",
    };

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <rect width="{width}" height="{height}" fill="{bg}" rx="6"/>
  <text x="{title_x}" y="16" fill="{text_color}" font-family="system-ui,sans-serif" font-size="11" font-weight="600">{project_name} — {metric_label} Coverage</text>
  <text x="{value_x}" y="16" fill="{current_color}" font-family="system-ui,sans-serif" font-size="11" font-weight="700" text-anchor="end">{current_pct}</text>
"#,
        title_x = padding_left,
        value_x = width - padding_right,
    );

    // Grid lines + Y labels
    for &pct in &y_labels {
        let y = to_y(pct);
        svg.push_str(&format!(
            r#"  <line x1="{pl}" y1="{y:.1}" x2="{pr:.1}" y2="{y:.1}" stroke="{grid_color}" stroke-width="0.5"/>
  <text x="{lx}" y="{ly:.1}" fill="{text_color}" font-family="system-ui,sans-serif" font-size="9" text-anchor="end" opacity="0.6">{label}%</text>
"#,
            pl = padding_left,
            pr = width - padding_right,
            lx = padding_left - 4.0,
            ly = y + 3.0,
            label = (pct * 100.0) as u32,
        ));
    }

    // Threshold lines (dashed)
    svg.push_str(&format!(
        r#"  <line x1="{pl}" y1="{thresh_y:.1}" x2="{pr:.1}" y2="{thresh_y:.1}" stroke="{green}" stroke-width="0.8" stroke-dasharray="4,3" opacity="0.5"/>
  <line x1="{pl}" y1="{warn_y:.1}" x2="{pr:.1}" y2="{warn_y:.1}" stroke="{yellow}" stroke-width="0.8" stroke-dasharray="4,3" opacity="0.5"/>
"#,
        pl = padding_left,
        pr = width - padding_right,
    ));

    // Area fill
    svg.push_str(&format!(
        r#"  <polygon points="{fill_points}" fill="{fill_color}"/>
"#,
    ));

    // Trend line
    svg.push_str(&format!(
        r#"  <polyline points="{line_points}" fill="none" stroke="{line_color}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
"#,
    ));

    // Data point dots (last point highlighted)
    if let Some(&last) = rates.last() {
        let lx = to_x(rates.len() - 1);
        let ly = to_y(last);
        svg.push_str(&format!(
            r#"  <circle cx="{lx:.1}" cy="{ly:.1}" r="3.5" fill="{current_color}" stroke="{bg}" stroke-width="1.5"/>
"#,
        ));
    }

    // X-axis label
    let points_label = format!("{} points", rates.len());
    svg.push_str(&format!(
        r#"  <text x="{cx:.1}" y="{by:.1}" fill="{text_color}" font-family="system-ui,sans-serif" font-size="9" text-anchor="middle" opacity="0.5">{points_label}</text>
"#,
        cx = padding_left + chart_w / 2.0,
        by = height - 4.0,
    ));

    svg.push_str("</svg>");
    svg
}
