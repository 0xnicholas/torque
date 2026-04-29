use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;

use torque_harness::app;
use torque_harness::config::AppConfig;
use torque_harness::db::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting torque-harness service...");

    let config = AppConfig::from_env();

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await?;

    info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    let database = Database::new(pool);

    info!("Initializing LLM provider...");
    let mut registry = llm::ProviderRegistry::new();
    let default_config = llm::ProviderConfig::from_env().with_defaults();
    let default_provider = llm::create_provider(default_config)?;
    registry.register("default", default_provider);

    let health = registry.health_check_all().await;
    for (name, status) in &health {
        if !status.reachable {
            tracing::warn!(
                "Provider '{}' health check failed: {:?}",
                name,
                status.error
            );
        } else {
            tracing::info!(
                "Provider '{}' OK ({}ms, {} models)",
                name,
                status.latency_ms,
                status.model_count.unwrap_or(0)
            );
        }
    }

    let llm: Arc<dyn llm::LlmClient> = registry.create_default_client().await?.into();

    let app = app::build_app(database, llm);

    info!("Listening on {}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
