use crate::models::{Session, SessionStatus};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create(
    pool: &PgPool,
    api_key: &str,
) -> anyhow::Result<Session> {
    let session = sqlx::query_as::<_, Session>(
        r#"
        INSERT INTO sessions (api_key, status)
        VALUES ($1, 'idle')
        RETURNING *
        "#
    )
    .bind(api_key)
    .fetch_one(pool)
    .await?;

    Ok(session)
}

pub async fn get_by_id(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<Option<Session>> {
    let session = sqlx::query_as::<_, Session>(
        r#"SELECT * FROM sessions WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(session)
}

pub async fn list_by_api_key(
    pool: &PgPool,
    api_key: &str,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<Session>> {
    let sessions = sqlx::query_as::<_, Session>(
        r#"
        SELECT * FROM sessions
        WHERE api_key = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(api_key)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(sessions)
}

pub async fn update_status(
    pool: &PgPool,
    id: Uuid,
    status: SessionStatus,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE sessions
        SET status = $1, error_message = $2, updated_at = NOW()
        WHERE id = $3
        "#
    )
    .bind(status)
    .bind(error_message)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn try_mark_running(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<bool> {
    let updated_rows = sqlx::query(
        r#"
        UPDATE sessions
        SET status = 'running', error_message = NULL, updated_at = NOW()
        WHERE id = $1
          AND status IN ('idle', 'completed')
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(updated_rows == 1)
}
