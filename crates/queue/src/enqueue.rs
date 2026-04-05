use sqlx::{PgPool, Row};
use types::{QueueEntry, QueueStatus};
use crate::error::QueueError;

pub async fn enqueue(
    pool: &PgPool,
    entry: &QueueEntry,
) -> Result<uuid::Uuid, QueueError> {
    let result = sqlx::query(
        r#"
        INSERT INTO queue (id, tenant_id, run_id, node_id, priority, status, available_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (node_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(entry.id)
    .bind(entry.tenant_id)
    .bind(entry.run_id)
    .bind(entry.node_id)
    .bind(entry.priority)
    .bind(QueueStatus::Pending.to_string())
    .bind(entry.available_at)
    .bind(entry.created_at)
    .fetch_optional(pool)
    .await?;
    
    Ok(result.map(|row| row.get("id")).unwrap_or(entry.id))
}