use db::PgPool;
use queue::QueueError;
use checkpointer::{Checkpointer, CheckpointMeta};

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
    checkpointer: Option<&dyn Checkpointer>,
) -> Result<bool, String> {
    let artifacts = db::artifacts::get_by_node(pool, node_id)
        .await
        .map_err(|e| e.to_string())?;
    
    if !artifacts.is_empty() {
        db::nodes::update_status(pool, node_id, types::NodeStatus::Done)
            .await
            .map_err(|e| e.to_string())?;
        
        queue::complete(pool, queue_id).await.map_err(|e| e.to_string())?;
        
        return Ok(true);
    }
    
    if let Some(cp) = checkpointer {
        let checkpoints = cp.list_node_checkpoints(node_id)
            .await
            .map_err(|e| e.to_string())?;
        
        if !checkpoints.is_empty() {
            let latest = checkpoints.first().ok_or("empty checkpoint list")?;
            let state = cp.load(latest.id).await.map_err(|e| e.to_string())?;
            
            if !state.messages.is_empty() || !state.intermediate_results.is_empty() {
                db::nodes::update_status(pool, node_id, types::NodeStatus::Done)
                    .await
                    .map_err(|e| e.to_string())?;
                
                queue::complete(pool, queue_id).await.map_err(|e| e.to_string())?;
                
                return Ok(true);
            }
        }
    }
    
    Ok(false)
}

pub async fn get_recovery_checkpoint(
    checkpointer: &dyn Checkpointer,
    node_id: uuid::Uuid,
) -> Result<Option<CheckpointMeta>, String> {
    let checkpoints = checkpointer.list_node_checkpoints(node_id)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(checkpoints.into_iter().next())
}