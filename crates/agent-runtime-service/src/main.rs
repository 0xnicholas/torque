use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::env;
use tracing::info;

use agent_runtime_service::app;
use agent_runtime_service::db::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting agent-runtime-service service...");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/agent_runtime_service".to_string());
    
    let bind_addr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;

    info!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    let database = Database::new(pool);

    info!("Initializing LLM client...");
    let llm = Arc::new(llm::OpenAiClient::from_env()?);

    let app = app::build_app(database, llm);

    info!("Listening on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
