use omnivore_core::storage::Database;
use omnivore_server::build_router;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // Database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:omnivore.db?mode=rwc".to_string());
    let db = Database::new(&db_url).await?;
    tracing::info!("Database initialized at {db_url}");

    // Router
    let app = build_router(db);

    // Serve
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Omnivore Dashboard listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
