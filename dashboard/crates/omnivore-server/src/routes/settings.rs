use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, Redirect};
use axum::Form;
use omnivore_core::model::settings::GlobalSettings;
use omnivore_core::storage::Database;
use serde::Deserialize;

// -- Global settings page --

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsPage {
    settings: GlobalSettings,
}

impl SettingsPage {
    fn line_pct(&self) -> String {
        format!("{:.0}", self.settings.default_line_threshold * 100.0)
    }
    fn branch_pct(&self) -> String {
        format!("{:.0}", self.settings.default_branch_threshold * 100.0)
    }
    fn line_warn_pct(&self) -> String {
        format!("{:.0}", self.settings.default_line_warn_threshold * 100.0)
    }
    fn branch_warn_pct(&self) -> String {
        format!("{:.0}", self.settings.default_branch_warn_threshold * 100.0)
    }
}

pub async fn settings_page(
    State(db): State<Database>,
) -> Result<Html<String>, StatusCode> {
    let settings = db
        .get_global_settings()
        .await
        .unwrap_or_default();

    let page = SettingsPage { settings };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

#[derive(Deserialize)]
pub struct SaveSettingsForm {
    default_line_threshold: Option<String>,
    default_branch_threshold: Option<String>,
    default_line_warn_threshold: Option<String>,
    default_branch_warn_threshold: Option<String>,
}

pub async fn save_settings(
    State(db): State<Database>,
    Form(form): Form<SaveSettingsForm>,
) -> Result<Redirect, StatusCode> {
    let line = form
        .default_line_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
        .unwrap_or(0.8);

    let branch = form
        .default_branch_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
        .unwrap_or(0.8);

    let line_warn = form
        .default_line_warn_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);

    let branch_warn = form
        .default_branch_warn_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
        .unwrap_or(0.5);

    let settings = GlobalSettings {
        default_line_threshold: line,
        default_branch_threshold: branch,
        default_line_warn_threshold: line_warn,
        default_branch_warn_threshold: branch_warn,
    };

    db.update_global_settings(&settings)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/settings"))
}

// -- Project threshold update --

#[derive(Deserialize)]
pub struct ProjectThresholdForm {
    line_threshold: Option<String>,
    branch_threshold: Option<String>,
    line_warn_threshold: Option<String>,
    branch_warn_threshold: Option<String>,
}

pub async fn save_project_thresholds(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Form(form): Form<ProjectThresholdForm>,
) -> Result<Redirect, StatusCode> {
    // Empty string = inherit global default (NULL in DB)
    let line = form
        .line_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    let branch = form
        .branch_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    let line_warn = form
        .line_warn_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    let branch_warn = form
        .branch_warn_threshold
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    db.update_project_thresholds(&project_id, line, branch, line_warn, branch_warn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to(&format!("/projects/{}", project_id)))
}
