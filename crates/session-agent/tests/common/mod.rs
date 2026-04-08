use session_agent::db::Database;
use sqlx::postgres::PgPoolOptions;

#[allow(dead_code)]
pub mod fake_llm;

pub async fn setup_test_db_or_skip() -> Option<Database> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/session_agent_test".to_string());

    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
    {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("skipping DB test: unable to connect test database: {err}");
            return None;
        }
    };

    if let Err(err) = sqlx::query("DROP TABLE IF EXISTS session_messages, sessions, tools CASCADE")
        .execute(&pool)
        .await
    {
        eprintln!("skipping DB test: unable to clean test database: {err}");
        return None;
    }

    let migrator = match sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
    )
    .await
    {
        Ok(migrator) => migrator,
        Err(err) => {
            eprintln!("skipping DB test: unable to prepare migrator: {err}");
            return None;
        }
    };

    if let Err(err) = migrator.run(&pool).await {
        eprintln!("skipping DB test: unable to run migrations: {err}");
        return None;
    }

    Some(Database::new(pool))
}

#[allow(dead_code)]
pub fn test_api_key() -> String {
    "test-api-key-12345".to_string()
}
