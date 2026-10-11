use omnivore_core::storage::Database;
use omnivore_server::{build_router, init_uptime};
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

    // With OAuth on, admin access must be granted explicitly. Without either
    // setting nobody can reach global settings or API keys, which otherwise
    // only shows up as a 403 the first time someone tries.
    let configured = |name: &str| std::env::var(name).is_ok_and(|v| !v.trim().is_empty());
    if omnivore_server::routes::auth::oauth_enabled()
        && !configured("OMNIVORE_ADMIN_USERS")
        && !configured("OMNIVORE_GITHUB_ORG")
    {
        tracing::warn!(
            "GitHub OAuth is enabled but no dashboard admin is configured: set \
             OMNIVORE_ADMIN_USERS (comma-separated GitHub usernames) or \
             OMNIVORE_GITHUB_ORG, or nobody can manage global settings or API keys"
        );
    }

    // Router
    let app = build_router(db);

    // Serve
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Omnivore Dashboard listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
