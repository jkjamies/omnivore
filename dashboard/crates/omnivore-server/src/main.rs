use omnivore_core::storage::Database;
use omnivore_server::{build_router, init_uptime, log_security_posture, maintenance};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (not required)
    let _ = dotenvy::dotenv();

    // Track server start time
    init_uptime();

    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // Database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:omnivore.db?mode=rwc".to_string());
    let db = Database::new(&db_url).await?;
    tracing::info!("Database initialized at {db_url}");

    // State plainly what this instance allows; both access controls default
    // to open and neither is visible from the UI.
    log_security_posture(&db).await;

    // Background housekeeping: expired sessions, permission cache, stale
    // source blobs. Without it these tables only ever grow.
    maintenance::spawn(db.clone());

    // Router
    let app = build_router(db);

    // Serve
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Omnivore Dashboard listening on {addr}");
    // Connect info is what the ingest rate limiter uses to identify clients
    // when there is no reverse proxy in front supplying X-Forwarded-For.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
