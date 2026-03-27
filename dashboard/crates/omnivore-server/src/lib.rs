pub mod routes;

pub use routes::auth::OAuthConfig;
pub use routes::health::init_uptime;

use axum::{routing, Router};
use omnivore_core::storage::Database;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

/// Build the application router. Extracted so integration tests can reuse it.
pub fn build_router(db: Database) -> Router {
    let static_dir = std::env::var("OMNIVORE_STATIC_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static"));

    // -- Global settings (admin-only when OAuth is enabled) --
    let admin_routes = Router::new()
        .route("/settings", routing::get(routes::settings::settings_page))
        .route("/settings", routing::post(routes::settings::save_settings))
        .route(
            "/settings/api-keys",
            routing::post(routes::settings::create_global_api_key),
        )
        .route(
            "/settings/api-keys/{key_id}/delete",
            routing::post(routes::settings::delete_global_api_key),
        )
        .layer(axum::middleware::from_fn_with_state(
            db.clone(),
            routes::auth::require_admin_middleware,
        ));

    // -- Project settings (login-required when OAuth is enabled) --
    let project_settings_routes = Router::new()
        .route(
            "/projects/{project_id}/settings",
            routing::get(routes::settings::project_settings_page),
        )
        .route(
            "/projects/{project_id}/thresholds",
            routing::post(routes::settings::save_project_thresholds),
        )
        .route(
            "/projects/{project_id}/tags",
            routing::post(routes::settings::save_project_tags),
        )
        .route(
            "/projects/{project_id}/ratchet",
            routing::post(routes::settings::save_project_ratchet),
        )
        .route(
            "/projects/{project_id}/delete",
            routing::post(routes::settings::delete_project),
        )
        .route(
            "/projects/{project_id}/api-keys",
            routing::post(routes::settings::create_project_api_key),
        )
        .route(
            "/projects/{project_id}/api-keys/{key_id}/delete",
            routing::post(routes::settings::delete_project_api_key),
        )
        .layer(axum::middleware::from_fn_with_state(
            db.clone(),
            routes::auth::require_login_middleware,
        ));

    // -- All other routes (open — no auth required) --
    let mut app = Router::new()
        .merge(admin_routes)
        .merge(project_settings_routes)
        // Pages (open for viewing)
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
            "/projects/{project_id}/files/{*file_path}",
            routing::get(routes::pages::file_coverage_page),
        )
        .route("/health", routing::get(routes::pages::health_page))
        // Source fragment (HTMX, uses user token if logged in)
        .route(
            "/api/v1/source/{project_id}/files/{*file_path}",
            routing::get(routes::pages::file_source_fragment),
        )
        // Auth: logout and me (always accessible)
        .route("/auth/logout", routing::post(routes::auth::logout))
        .route("/auth/me", routing::get(routes::auth::me))
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
        // Embeds
        .route(
            "/embed/{project_id}/trend",
            routing::get(routes::embed::trend_embed),
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
        .with_state(db.clone());

    // Auth routes that need OAuthConfig (only registered if OAuth is configured)
    if let Some(oauth_config) = OAuthConfig::from_env() {
        let auth_routes = Router::new()
            .route("/auth/login", routing::get(routes::auth::login))
            .with_state(oauth_config.clone())
            .route(
                "/auth/callback",
                routing::get(routes::auth::callback),
            )
            .with_state((db, oauth_config));

        app = app.merge(auth_routes);

        tracing::info!("GitHub OAuth enabled");
    } else {
        tracing::info!("GitHub OAuth not configured (GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET not set)");
    }

    app
}
