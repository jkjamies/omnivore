use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::model::coverage::DependencyGraph;
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;

#[derive(Template)]
#[template(path = "dependency_graph.html")]
struct DependencyGraphPage {
    project: Project,
    graph: DependencyGraph,
}

impl DependencyGraphPage {
    fn graph_json(&self) -> String {
        serde_json::to_string(&self.graph).unwrap_or_else(|_| r#"{"modules":[],"edges":[]}"#.to_string())
    }
    fn internal_count(&self) -> usize {
        use omnivore_core::model::coverage::ModuleType;
        self.graph.modules.iter().filter(|m| m.module_type == ModuleType::Internal).count()
    }
}

pub async fn dependency_graph_page(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let latest = db
        .get_latest_snapshot(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let graph: DependencyGraph = latest
        .dependencies_json
        .as_deref()
        .ok_or(StatusCode::NOT_FOUND)
        .and_then(|json| serde_json::from_str(json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR))?;

    let page = DependencyGraphPage { project, graph };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}
