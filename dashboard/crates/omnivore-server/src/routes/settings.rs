use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, Redirect};
use axum::Form;
use omnivore_core::model::api_key::{ApiKey, ApiKeyCreated};
use omnivore_core::model::project::Project;
use omnivore_core::model::settings::GlobalSettings;
use omnivore_core::storage::Database;
use serde::Deserialize;

// -- Global settings page --

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsPage {
    settings: GlobalSettings,
    api_keys: Vec<ApiKey>,
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
    fn retention_full(&self) -> String {
        self.settings.retention_full.to_string()
    }
    fn retention_summary(&self) -> String {
        self.settings.retention_summary.to_string()
    }
    fn fmt_key_date(&self, key: &ApiKey) -> String {
        key.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
    fn fmt_key_last_used(&self, key: &ApiKey) -> String {
        key.last_used_at
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Never".to_string())
    }
}

pub async fn settings_page(
    State(db): State<Database>,
) -> Result<Html<String>, StatusCode> {
    let settings = db
        .get_global_settings()
        .await
        .unwrap_or_default();

    let api_keys = db
        .list_api_keys(None)
        .await
        .unwrap_or_default();

    let page = SettingsPage { settings, api_keys };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

#[derive(Deserialize)]
pub struct SaveSettingsForm {
    default_line_threshold: Option<String>,
    default_branch_threshold: Option<String>,
    default_line_warn_threshold: Option<String>,
    default_branch_warn_threshold: Option<String>,
    retention_full: Option<String>,
    retention_summary: Option<String>,
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

    let retention_full = form
        .retention_full
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i64>().ok())
        .map(|v| v.max(1))
        .unwrap_or(30);

    let retention_summary = form
        .retention_summary
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i64>().ok())
        .map(|v| v.max(0))
        .unwrap_or(60);

    let settings = GlobalSettings {
        default_line_threshold: line,
        default_branch_threshold: branch,
        default_line_warn_threshold: line_warn,
        default_branch_warn_threshold: branch_warn,
        retention_full,
        retention_summary,
    };

    db.update_global_settings(&settings)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/settings"))
}

// -- Project settings page --

#[derive(Template)]
#[template(path = "project_settings.html")]
struct ProjectSettingsPage {
    project: Project,
    global_settings: GlobalSettings,
    api_keys: Vec<ApiKey>,
}

impl ProjectSettingsPage {
    fn ratchet_enabled(&self) -> bool {
        self.project.ratchet_enabled
    }
    fn ratchet_line_floor_pct(&self) -> String {
        self.project.ratchet_line_floor
            .map(|v| format!("{:.1}", v * 100.0))
            .unwrap_or_default()
    }
    fn ratchet_branch_floor_pct(&self) -> String {
        self.project.ratchet_branch_floor
            .map(|v| format!("{:.1}", v * 100.0))
            .unwrap_or_default()
    }
    fn project_line_pct(&self) -> String {
        self.project.line_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn project_branch_pct(&self) -> String {
        self.project.branch_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn project_line_warn_pct(&self) -> String {
        self.project.line_warn_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn project_branch_warn_pct(&self) -> String {
        self.project.branch_warn_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn global_line_pct(&self) -> String {
        format!("{:.0}", self.global_settings.default_line_threshold * 100.0)
    }
    fn global_branch_pct(&self) -> String {
        format!("{:.0}", self.global_settings.default_branch_threshold * 100.0)
    }
    fn global_line_warn_pct(&self) -> String {
        format!("{:.0}", self.global_settings.default_line_warn_threshold * 100.0)
    }
    fn global_branch_warn_pct(&self) -> String {
        format!("{:.0}", self.global_settings.default_branch_warn_threshold * 100.0)
    }
    fn project_tags(&self) -> String {
        self.project.tags.clone().unwrap_or_default()
    }
    fn fmt_key_date(&self, key: &ApiKey) -> String {
        key.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
    fn fmt_key_last_used(&self, key: &ApiKey) -> String {
        key.last_used_at
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Never".to_string())
    }
    fn badge_url(&self) -> String {
        let base = std::env::var("OMNIVORE_DASHBOARD_URL")
            .unwrap_or_else(|_| String::new());
        format!("{}/badge/{}", base, self.project.id)
    }
}

pub async fn project_settings_page(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let global_settings = db.get_global_settings().await.unwrap_or_default();

    let api_keys = db
        .list_api_keys(Some(&project_id))
        .await
        .unwrap_or_default();

    let page = ProjectSettingsPage { project, global_settings, api_keys };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
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

    Ok(Redirect::to(&format!("/projects/{}/settings", project_id)))
}

// -- Project tags --

#[derive(Deserialize)]
pub struct ProjectTagsForm {
    tags: Option<String>,
}

pub async fn save_project_tags(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Form(form): Form<ProjectTagsForm>,
) -> Result<Redirect, StatusCode> {
    let tags = form.tags.as_deref().filter(|s| !s.trim().is_empty());
    // Normalize: trim each tag, remove empties
    let normalized = tags.map(|s| {
        s.split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    });

    db.update_project_tags(&project_id, normalized.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to(&format!("/projects/{}/settings", project_id)))
}

// -- Project ratchet --

#[derive(Deserialize)]
pub struct ProjectRatchetForm {
    ratchet_enabled: Option<String>,
    ratchet_line_floor: Option<String>,
    ratchet_branch_floor: Option<String>,
}

pub async fn save_project_ratchet(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Form(form): Form<ProjectRatchetForm>,
) -> Result<Redirect, StatusCode> {
    let enabled = form.ratchet_enabled.as_deref() == Some("on");

    let line_floor = form.ratchet_line_floor
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    let branch_floor = form.ratchet_branch_floor
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    db.update_project_ratchet(&project_id, enabled, line_floor, branch_floor)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to(&format!("/projects/{}/settings", project_id)))
}

// -- Project delete --

pub async fn delete_project(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Redirect, StatusCode> {
    db.delete_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/"))
}

// -- API key management --

#[derive(Deserialize)]
pub struct CreateApiKeyForm {
    name: String,
}

#[derive(Template)]
#[template(path = "api_key_created.html")]
struct ApiKeyCreatedPage {
    key: ApiKeyCreated,
}

/// Create a global API key (not scoped to a project).
pub async fn create_global_api_key(
    State(db): State<Database>,
    Form(form): Form<CreateApiKeyForm>,
) -> Result<Html<String>, StatusCode> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let key = db
        .create_api_key(name, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let page = ApiKeyCreatedPage { key };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

/// Delete a global API key.
pub async fn delete_global_api_key(
    State(db): State<Database>,
    Path(key_id): Path<String>,
) -> Result<Redirect, StatusCode> {
    db.delete_api_key(&key_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/settings"))
}

/// Create a project-scoped API key.
pub async fn create_project_api_key(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Form(form): Form<CreateApiKeyForm>,
) -> Result<Html<String>, StatusCode> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let key = db
        .create_api_key(name, Some(&project_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let page = ApiKeyCreatedPage { key };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

/// Delete a project-scoped API key.
pub async fn delete_project_api_key(
    State(db): State<Database>,
    Path((project_id, key_id)): Path<(String, String)>,
) -> Result<Redirect, StatusCode> {
    db.delete_api_key(&key_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to(&format!("/projects/{}/settings", project_id)))
}
