use db::PgPool;
use queue::QueueError;

pub async fn find_stale_locked_entries(
    pool: &PgPool,
    max_age_seconds: i64,
) -> Result<Vec<db::queue::StaleEntry>, QueueError> {
    db::queue::find_stale_locked(pool, max_age_seconds)
        .await
        .map_err(QueueError::from)
}

pub async fn reset_stale_entry(pool: &PgPool, entry: &db::queue::StaleEntry) -> Result<(), QueueError> {
    queue::reset_to_pending(pool, entry.id).await
}

pub async fn recover_node_if_needed(
    pool: &PgPool,
    node_id: uuid::Uuid,
    queue_id: uuid::Uuid,
) -> Result<bool, String> {
    let artifacts = db::artifacts::get_by_node(pool, node_id)
        .await
        .map_err(|e| e.to_string())?;
    
    if artifacts.is_empty() {
        return Ok(false);
    }
    
    db::nodes::update_status(pool, node_id, types::NodeStatus::Done)
        .await
        .map_err(|e| e.to_string())?;
    
    queue::complete(pool, queue_id).await.map_err(|e| e.to_string())?;
    
    Ok(true)
}