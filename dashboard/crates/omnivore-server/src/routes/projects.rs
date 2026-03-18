use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use omnivore_core::model::project::{CreateProject, Project};
use omnivore_core::storage::Database;
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
    Json(input): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), StatusCode> {
    db.create_project(&input)
        .await
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(|_| StatusCode::CONFLICT)
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
    Json(input): Json<UpdateProject>,
) -> Result<Json<Project>, StatusCode> {
    db.update_project_settings(
        &project_id,
        input.github_repo.as_deref(),
        input.source_root.as_deref(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map(Json)
    .ok_or(StatusCode::NOT_FOUND)
}
