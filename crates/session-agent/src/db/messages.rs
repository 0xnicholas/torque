use crate::models::Message;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create(
    pool: &PgPool,
    message: &Message,
) -> anyhow::Result<Message> {
    let msg = sqlx::query_as::<_, Message>(
        r#"
        INSERT INTO session_messages (id, session_id, role, content, tool_calls, artifacts, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#
    )
    .bind(message.id)
    .bind(message.session_id)
    .bind(&message.role)
    .bind(&message.content)
    .bind(&message.tool_calls)
    .bind(&message.artifacts)
    .bind(message.created_at)
    .fetch_one(pool)
    .await?;

    Ok(msg)
}

pub async fn list_by_session(
    pool: &PgPool,
    session_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<Message>> {
    let messages = sqlx::query_as::<_, Message>(
        r#"
        SELECT * FROM session_messages
        WHERE session_id = $1
        ORDER BY created_at ASC
        LIMIT $2
        "#
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(messages)
}

pub async fn get_recent_by_session(
    pool: &PgPool,
    session_id: Uuid,
    count: i64,
) -> anyhow::Result<Vec<Message>> {
    let messages = sqlx::query_as::<_, Message>(
        r#"
        SELECT * FROM session_messages
        WHERE session_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#
    )
    .bind(session_id)
    .bind(count)
    .fetch_all(pool)
    .await?;

    let mut messages = messages;
    messages.reverse();
    Ok(messages)
}