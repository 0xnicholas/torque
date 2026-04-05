use sqlx::{PgPool, Row};
use types::QueueStatus;
use uuid::Uuid;

pub async fn find_stale_locked(pool: &PgPool, max_age_seconds: i64) -> Result<Vec<StaleEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, tenant_id, run_id, node_id, status, available_at, locked_at, locked_by, created_at
        FROM queue
        WHERE status = 'locked'
          AND locked_at < NOW() - INTERVAL '1 second' * $1
        "#
    )
    .bind(max_age_seconds)
    .fetch_all(pool)
    .await?;
    
    Ok(rows.into_iter().map(|row| {
        let status_str: String = row.get("status");
        StaleEntry {
            id: row.get("id"),
            tenant_id: row.get("tenant_id"),
            run_id: row.get("run_id"),
            node_id: row.get("node_id"),
            status: match status_str.as_str() {
                "pending" => QueueStatus::Pending,
                "locked" => QueueStatus::Locked,
                "done" => QueueStatus::Done,
                _ => QueueStatus::Pending,
            },
            available_at: row.get("available_at"),
            locked_at: row.get("locked_at"),
            locked_by: row.get("locked_by"),
            created_at: row.get("created_at"),
        }
    }).collect())
}

#[derive(Debug, Clone)]
pub struct StaleEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub status: QueueStatus,
    pub available_at: chrono::DateTime<chrono::Utc>,
    pub locked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub locked_by: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}