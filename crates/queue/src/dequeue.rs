use sqlx::{PgPool, Row};
use types::{QueueEntry, QueueStatus};
use crate::error::QueueError;

pub async fn dequeue(
    pool: &PgPool,
    tenant_id: uuid::Uuid,
    executor_id: &str,
) -> Result<Option<QueueEntry>, QueueError> {
    let mut tx = pool.begin().await?;
    
    let row = sqlx::query(
        r#"
        SELECT * FROM queue
        WHERE tenant_id = $1
          AND status = 'pending'
          AND available_at <= NOW()
        ORDER BY priority DESC, created_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    
    if let Some(row) = row {
        let id: uuid::Uuid = row.get("id");
        
        sqlx::query(
            r#"
            UPDATE queue
            SET status = $2, locked_at = NOW(), locked_by = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(QueueStatus::Locked.to_string())
        .bind(executor_id)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        Ok(Some(QueueEntry {
            id: row.get("id"),
            tenant_id: row.get("tenant_id"),
            run_id: row.get("run_id"),
            node_id: row.get("node_id"),
            priority: row.get("priority"),
            status: QueueStatus::Locked,
            available_at: row.get("available_at"),
            locked_at: Some(chrono::Utc::now()),
            locked_by: Some(executor_id.to_string()),
            created_at: row.get("created_at"),
        }))
    } else {
        tx.rollback().await?;
        Ok(None)
    }
}