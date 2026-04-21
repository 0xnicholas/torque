use sqlx::{PgPool, Row};
use crate::error::QueueError;

pub async fn get_waiting_count(pool: &PgPool, tenant_id: uuid::Uuid) -> Result<u64, QueueError> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) FROM queue
        WHERE tenant_id = $1 AND status = 'pending' AND available_at <= NOW()
        "#,
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    
    let count: i64 = row.get("count");
    Ok(count as u64)
}