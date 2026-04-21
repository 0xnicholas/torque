use sqlx::PgPool;
use types::QueueStatus;
use crate::error::QueueError;

pub async fn complete(pool: &PgPool, queue_id: uuid::Uuid) -> Result<(), QueueError> {
    sqlx::query(
        r#"
        UPDATE queue SET status = $2 WHERE id = $1
        "#,
    )
    .bind(queue_id)
    .bind(QueueStatus::Done.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_to_pending(pool: &PgPool, queue_id: uuid::Uuid) -> Result<(), QueueError> {
    sqlx::query(
        r#"
        UPDATE queue
        SET status = 'pending', locked_at = NULL, locked_by = NULL, available_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(queue_id)
    .execute(pool)
    .await?;
    Ok(())
}