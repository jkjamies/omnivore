mod routes;

use axum::{routing, Router};
use omnivore_core::storage::Database;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

/// Build the application router. Extracted so integration tests can reuse it.
pub fn build_router(db: Database) -> Router {
    let static_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static");

    Router::new()
        // Pages
        .route("/", routing::get(routes::pages::projects_page))
        .route(
            "/projects/{project_id}",
            routing::get(routes::pages::project_detail_page),
        )
        .route(
            "/projects/{project_id}/dependencies",
            routing::get(routes::pages::dependency_graph_page),
        )
        .route(
            "/projects/{project_id}/export/report",
            routing::get(routes::export::export_report),
        )
        .route(
            "/projects/{project_id}/settings",
            routing::get(routes::settings::project_settings_page),
        )
        .route(
            "/projects/{project_id}/thresholds",
            routing::post(routes::settings::save_project_thresholds),
        )
        .route(
            "/projects/{project_id}/delete",
            routing::post(routes::settings::delete_project),
        )
        // Settings
        .route("/settings", routing::get(routes::settings::settings_page))
        .route("/settings", routing::post(routes::settings::save_settings))
        .route(
            "/projects/{project_id}/files/{*file_path}",
            routing::get(routes::pages::file_coverage_page),
        )
        .route(
            "/api/v1/source/{project_id}/files/{*file_path}",
            routing::get(routes::pages::file_source_fragment),
        )
        // API: Health
        .route("/api/v1/health", routing::get(routes::health::health))
        // API: Projects
        .route(
            "/api/v1/projects",
            routing::get(routes::projects::list_projects),
        )
        .route(
            "/api/v1/projects",
            routing::post(routes::projects::create_project),
        )
        .route(
            "/api/v1/projects/{project_id}",
            routing::patch(routes::projects::update_project),
        )
        // API: Coverage ingestion
        .route(
            "/api/v1/ingest/coverage",
            routing::post(routes::coverage::ingest_coverage),
        )
        // API: Coverage queries
        .route(
            "/api/v1/coverage/{project_id}/latest",
            routing::get(routes::coverage::get_latest),
        )
        .route(
            "/api/v1/coverage/{project_id}/trend",
            routing::get(routes::coverage::get_trend),
        )
        .route(
            "/api/v1/coverage/{project_id}/dependencies",
            routing::get(routes::coverage::get_dependencies),
        )
        // Badge
        .route(
            "/badge/{project_id}",
            routing::get(routes::badge::badge),
        )
        // Static files
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(db)
}
