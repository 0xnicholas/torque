use session_agent::db::Database;
use sqlx::postgres::PgPoolOptions;

pub async fn setup_test_db() -> Database {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/session_agent_test".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::query("DROP TABLE IF EXISTS session_messages, sessions, tools CASCADE")
        .execute(&pool)
        .await
        .expect("Failed to clean test database");

    let migrator = sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
    )
    .await
    .expect("Failed to create migrator");
    migrator.run(&pool).await.expect("Failed to run migrations");

    Database::new(pool)
}

pub fn test_api_key() -> String {
    "test-api-key-12345".to_string()
}