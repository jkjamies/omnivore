use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::model::coverage::CoverageSnapshot;
use omnivore_core::model::project::Project;
use omnivore_core::storage::{ActivityEntry, Database};

use super::{fmt_delta_html, fmt_pct_val, rate_color_with_threshold, TargetSnapshot};

pub struct ProjectWithLatest {
    pub project: Project,
    pub latest: Option<CoverageSnapshot>,
    pub targets: Vec<TargetSnapshot>,
    pub effective_line_threshold: f64,
    pub effective_line_warn_threshold: f64,
    pub sparkline: String,
}

pub struct HomeSummary {
    pub total_projects: usize,
    pub avg_line_rate: f64,
    pub passing_count: usize,
    pub warning_count: usize,
    pub failing_count: usize,
}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsPage {
    projects: Vec<ProjectWithLatest>,
    summary: Option<HomeSummary>,
    activity: Vec<ActivityEntry>,
}

impl ProjectsPage {
    fn fmt_pct(&self, rate: &f64) -> String {
        fmt_pct_val(*rate)
    }
    fn rate_color_t(&self, rate: &f64, threshold: &f64, warn_threshold: &f64) -> &'static str {
        rate_color_with_threshold(*rate, *threshold, *warn_threshold)
    }
    fn fmt_delta(&self, delta: &Option<f64>) -> String {
        fmt_delta_html(*delta)
    }
    fn fmt_activity_time(&self, dt: &chrono::DateTime<chrono::Utc>) -> String {
        dt.format("%b %d, %Y %H:%M").to_string()
    }
    fn short_sha(&self, sha: &Option<String>) -> String {
        sha.as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| if s.len() > 7 { &s[..7] } else { s })
            .unwrap_or("—")
            .to_string()
    }
}

pub async fn projects_page(
    State(db): State<Database>,
) -> Result<Html<String>, StatusCode> {
    let projects = db
        .list_projects()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let global_settings = db.get_global_settings().await.unwrap_or_default();

    let mut items = Vec::with_capacity(projects.len());
    for project in projects {
        let latest = db
            .get_latest_snapshot(&project.id)
            .await
            .unwrap_or(None);

        let target_names = db
            .get_targets_for_project(&project.id)
            .await
            .unwrap_or_default();
        let mut targets = Vec::new();
        let mut sparkline_points: Vec<f64> = Vec::new();
        for tname in &target_names {
            let snaps = db
                .get_snapshots_for_project_by_target(&project.id, tname, 15)
                .await
                .unwrap_or_default();
            if let Some(snap) = snaps.first() {
                let prev = snaps.get(1);
                targets.push(TargetSnapshot::from_snapshot(snap, prev, vec![]));
            }
            // Use the first target's trend for the sparkline
            if sparkline_points.is_empty() {
                sparkline_points = snaps.iter().rev().map(|s| s.line_rate).collect();
            }
        }
        let sparkline = sparkline_points
            .iter()
            .map(|r| format!("{:.3}", r))
            .collect::<Vec<_>>()
            .join(",");

        let effective_line_threshold = project
            .line_threshold
            .unwrap_or(global_settings.default_line_threshold);
        let effective_line_warn_threshold = project
            .line_warn_threshold
            .unwrap_or(global_settings.default_line_warn_threshold);
        items.push(ProjectWithLatest { project, latest, targets, effective_line_threshold, effective_line_warn_threshold, sparkline });
    }

    let summary = if items.is_empty() {
        None
    } else {
        let total_projects = items.len();
        let mut total_lines_covered: i64 = 0;
        let mut total_lines: i64 = 0;
        let mut passing = 0usize;
        let mut warning = 0usize;
        let mut failing = 0usize;

        for item in &items {
            if let Some(snap) = &item.latest {
                total_lines_covered += snap.lines_covered;
                total_lines += snap.lines_total;
                if snap.line_rate >= item.effective_line_threshold {
                    passing += 1;
                } else if snap.line_rate >= item.effective_line_warn_threshold {
                    warning += 1;
                } else {
                    failing += 1;
                }
            }
        }

        let avg_line_rate = if total_lines > 0 {
            total_lines_covered as f64 / total_lines as f64
        } else {
            0.0
        };

        Some(HomeSummary {
            total_projects,
            avg_line_rate,
            passing_count: passing,
            warning_count: warning,
            failing_count: failing,
        })
    };

    let activity = db.get_recent_activity(15).await.unwrap_or_default();

    let page = ProjectsPage { projects: items, summary, activity };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}
