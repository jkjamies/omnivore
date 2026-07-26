use crate::routes::api_auth::{authenticate_write, enforce_project_scope};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use omnivore_core::model::project::{CreateProject, Project};
use omnivore_core::storage::Database;
use omnivore_core::validation::is_valid_repo_slug;
use serde::Deserialize;

pub async fn list_projects(
    State(db): State<Database>,
) -> Result<Json<Vec<Project>>, StatusCode> {
    db.list_projects()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn create_project(
    State(db): State<Database>,
    headers: HeaderMap,
    Json(input): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), (StatusCode, String)> {
    let key = authenticate_write(&db, &headers).await?;
    enforce_project_scope(key.as_ref(), &input.id)?;
    validate_repo_field(input.github_repo.as_deref())?;

    db.create_project(&input)
        .await
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(|e| {
            tracing::warn!(error = %e, project_id = %input.id, "Project creation failed");
            (
                StatusCode::CONFLICT,
                "Project already exists or could not be created".to_string(),
            )
        })
}

#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    #[serde(default)]
    pub github_repo: Option<String>,
    #[serde(default)]
    pub source_root: Option<String>,
}

pub async fn update_project(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<UpdateProject>,
) -> Result<Json<Project>, (StatusCode, String)> {
    // `github_repo` decides which repository the dashboard will fetch source
    // from and comment on, so changing it is a privileged operation — it used
    // to be writable by any anonymous caller.
    let key = authenticate_write(&db, &headers).await?;
    enforce_project_scope(key.as_ref(), &project_id)?;
    validate_repo_field(input.github_repo.as_deref())?;

    db.update_project_settings(
        &project_id,
        input.github_repo.as_deref(),
        input.source_root.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, %project_id, "Project update failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update project".to_string(),
        )
    })?
    .map(Json)
    .ok_or((StatusCode::NOT_FOUND, "Project not found".to_string()))
}

/// Reject a `github_repo` that isn't a plain `owner/name` slug, so an invalid
/// value can never be persisted and later interpolated into a GitHub URL.
fn validate_repo_field(repo: Option<&str>) -> Result<(), (StatusCode, String)> {
    match repo {
        None => Ok(()),
        Some(r) if r.trim().is_empty() => Ok(()),
        Some(r) if is_valid_repo_slug(r) => Ok(()),
        Some(r) => Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid github_repo '{r}' — expected owner/name"),
        )),
    }
}
