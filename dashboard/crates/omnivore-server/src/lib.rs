pub mod maintenance;
pub mod routes;

pub use routes::auth::OAuthConfig;
pub use routes::health::init_uptime;

use axum::http::{header, HeaderValue, Method};
use axum::{routing, Router};
use omnivore_core::storage::Database;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Maximum accepted ingest body.
///
/// Axum's default extractor limit is 2 MiB, which real multi-module coverage
/// reports exceed — but the endpoint is reachable before authentication in open
/// mode, so it needs *a* ceiling rather than none. 32 MiB by default,
/// overridable with `OMNIVORE_MAX_UPLOAD_BYTES`.
const DEFAULT_MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024;

fn max_upload_bytes() -> usize {
    std::env::var("OMNIVORE_MAX_UPLOAD_BYTES")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_MAX_UPLOAD_BYTES)
}

/// Build the CORS layer.
///
/// The previous `CorsLayer::permissive()` allowed any origin, method and
/// header on every route including ingest. For a self-hosted dashboard the
/// right default is same-origin only; operators who genuinely embed badges or
/// call the API cross-origin list their origins in `OMNIVORE_CORS_ORIGINS`
/// (comma-separated, or `*` to restore the old wide-open behaviour).
fn cors_layer() -> CorsLayer {
    let configured = std::env::var("OMNIVORE_CORS_ORIGINS").unwrap_or_default();
    let configured = configured.trim();

    if configured.is_empty() {
        return CorsLayer::new();
    }

    if configured == "*" {
        tracing::warn!("OMNIVORE_CORS_ORIGINS=* — every origin may call this API");
        return CorsLayer::permissive();
    }

    let origins: Vec<HeaderValue> = configured
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| match HeaderValue::from_str(s) {
            Ok(v) => Some(v),
            Err(_) => {
                tracing::warn!(origin = %s, "Ignoring malformed CORS origin");
                None
            }
        })
        .collect();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PATCH])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
}

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

    // -- Project settings --
    //
    // These all mutate (or expose the API keys of) a specific project, so they
    // require write access to *that* project — not merely a logged-in
    // account, which is what `require_login_middleware` used to check here.
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
            routes::auth::require_project_write_middleware,
        ));

    // -- Read routes --
    //
    // Open by default. `OMNIVORE_REQUIRE_LOGIN_TO_VIEW=true` puts them behind
    // a session, for instances where the file paths and hotspot lists are
    // themselves sensitive.
    let view_routes = Router::new()
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
        .route(
            "/api/v1/projects",
            routing::get(routes::projects::list_projects),
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
            "/api/v1/coverage/{project_id}/series",
            routing::get(routes::coverage::list_series),
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
        .layer(axum::middleware::from_fn_with_state(
            db.clone(),
            routes::auth::require_login_to_view_middleware,
        ));

    // -- Always-reachable routes --
    //
    // Health checks must answer for a load balancer regardless of auth, the
    // auth endpoints are how a user gets a session in the first place, and the
    // write API authenticates with an API key rather than a session — putting
    // any of them behind the read gate would deadlock the instance.
    let mut app = Router::new()
        .merge(admin_routes)
        .merge(project_settings_routes)
        .merge(view_routes)
        .route("/auth/logout", routing::post(routes::auth::logout))
        .route("/auth/me", routing::get(routes::auth::me))
        .route("/api/v1/health", routing::get(routes::health::health))
        .route(
            "/api/v1/projects",
            routing::post(routes::projects::create_project),
        )
        .route(
            "/api/v1/projects/{project_id}",
            routing::patch(routes::projects::update_project),
        )
        .route(
            "/api/v1/ingest/coverage",
            routing::post(routes::coverage::ingest_coverage)
                .layer(axum::extract::DefaultBodyLimit::max(max_upload_bytes())),
        )
        // Static files
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(cors_layer())
        // A CSP is the backstop for the HTML views: templates escape their
        // inputs, but coverage reports are attacker-supplied on an open
        // instance, so a missed spot should not become script execution.
        // 'unsafe-inline' is still required — the pages carry inline
        // <script> blocks and style attributes — so this bounds *where*
        // script may come from rather than eliminating injection risk.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; \
                 script-src 'self' 'unsafe-inline' https://unpkg.com https://cdn.jsdelivr.net; \
                 style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data: https://avatars.githubusercontent.com; \
                 connect-src 'self'; \
                 frame-ancestors 'none'; \
                 base-uri 'none'; \
                 form-action 'self'; \
                 object-src 'none'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(db.clone());

    // Auth routes that need OAuthConfig (only registered if OAuth is configured)
    if let Some(oauth_config) = OAuthConfig::from_env() {
        let auth_routes = Router::new()
            .route("/auth/login", routing::get(routes::auth::login))
            .with_state(oauth_config.clone())
            // `state` is verified against a cookie set by /auth/login
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

/// Log the instance's effective security posture at startup.
///
/// The dashboard has two independent access controls — OAuth for reading, API
/// keys for writing — and both default to open. Neither is visible from the UI,
/// so an operator can easily believe an instance is protected when it is not.
/// Stating it plainly on every boot is the cheapest way to prevent that.
pub async fn log_security_posture(db: &Database) {
    let oauth = routes::auth::oauth_enabled();
    let key_count = db.any_api_keys_exist().await.unwrap_or(false);
    let require_key = routes::api_auth::api_key_required();

    if oauth {
        if routes::auth::require_login_to_view() {
            tracing::info!("Read access: login required (OAuth enabled)");
        } else {
            tracing::info!(
                "Read access: OPEN — anyone who can reach this port can browse all coverage. \
                 Set OMNIVORE_REQUIRE_LOGIN_TO_VIEW=true to require a login."
            );
        }
    } else {
        tracing::warn!(
            "Read access: OPEN — GitHub OAuth is not configured, so nobody can log in and \
             everything is world-readable. Set GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET to enable."
        );
    }

    match (require_key, key_count) {
        (true, _) => tracing::info!("Write access: API key required (OMNIVORE_REQUIRE_API_KEY=true)"),
        (false, true) => tracing::info!("Write access: API key required (keys exist)"),
        (false, false) => tracing::warn!(
            "Write access: OPEN — no API keys exist, so anyone can upload coverage and create \
             projects. Create a key under /settings, or set OMNIVORE_REQUIRE_API_KEY=true."
        ),
    }

    if oauth && !omnivore_core::crypto::is_enabled() {
        tracing::warn!(
            "Session GitHub tokens are stored unencrypted. Set OMNIVORE_SECRET_KEY to encrypt \
             them at rest."
        );
    }
}
