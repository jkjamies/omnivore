use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::model::coverage::CoverageSnapshot;
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;

use super::{fmt_delta_html, fmt_pct_val, rate_color_with_threshold, TargetSnapshot};

pub struct ProjectWithLatest {
    pub project: Project,
    pub latest: Option<CoverageSnapshot>,
    pub targets: Vec<TargetSnapshot>,
    pub effective_line_threshold: f64,
    pub effective_line_warn_threshold: f64,
}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsPage {
    projects: Vec<ProjectWithLatest>,
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
        for tname in &target_names {
            let snaps = db
                .get_snapshots_for_project_by_target(&project.id, tname, 2)
                .await
                .unwrap_or_default();
            if let Some(snap) = snaps.first() {
                let prev = snaps.get(1);
                targets.push(TargetSnapshot::from_snapshot(snap, prev, vec![]));
            }
        }

        let effective_line_threshold = project
            .line_threshold
            .unwrap_or(global_settings.default_line_threshold);
        let effective_line_warn_threshold = project
            .line_warn_threshold
            .unwrap_or(global_settings.default_line_warn_threshold);
        items.push(ProjectWithLatest { project, latest, targets, effective_line_threshold, effective_line_warn_threshold });
    }

    let page = ProjectsPage { projects: items };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}
