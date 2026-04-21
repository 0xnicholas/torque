use types::{Node, NodeStatus, QueueEntry, QueueStatus};
use db::PgPool;
use uuid::Uuid;
use chrono::Utc;

pub struct Worker {
    id: usize,
    pool: PgPool,
}

impl Worker {
    pub fn new(id: usize, pool: PgPool) -> Self {
        Self { id, pool }
    }
    
    pub async fn check_upstream_deps_done(pool: &PgPool, node_id: Uuid) -> Result<bool, String> {
        let upstream_ids = db::edges::get_upstream_deps(pool, node_id)
            .await
            .map_err(|e| e.to_string())?;
        
        for upstream_id in upstream_ids {
            let node = db::nodes::get(pool, upstream_id)
                .await
                .map_err(|e| e.to_string())?;
            
            match node {
                Some(n) if n.status == NodeStatus::Done || n.status == NodeStatus::Skipped => continue,
                _ => return Ok(false),
            }
        }
        
        Ok(true)
    }
    
    pub async fn enqueue_downstream_nodes(pool: &PgPool, node: &Node) -> Result<(), String> {
        let downstream_ids = db::edges::get_downstream_nodes(pool, node.id)
            .await
            .map_err(|e| e.to_string())?;
        
        for downstream_id in downstream_ids {
            if !Self::check_upstream_deps_done(pool, downstream_id).await? {
                continue;
            }
            
            let downstream = db::nodes::get(pool, downstream_id)
                .await
                .map_err(|e| e.to_string())?;
            
            if let Some(downstream_node) = downstream {
                if downstream_node.status != NodeStatus::Pending {
                    continue;
                }
                
                let entry = QueueEntry {
                    id: Uuid::new_v4(),
                    tenant_id: node.tenant_id,
                    run_id: node.run_id,
                    node_id: downstream_id,
                    priority: 0,
                    status: QueueStatus::Pending,
                    available_at: Utc::now(),
                    locked_at: None,
                    locked_by: None,
                    created_at: Utc::now(),
                };
                
                let _ = queue::enqueue(pool, &entry).await;
                tracing::info!(node_id = %downstream_id, "Enqueued downstream node");
            }
        }
        
        Ok(())
    }
    
    pub async fn run_node(&self, node: Node) -> Result<(), String> {
        db::nodes::update_status(&self.pool, node.id, NodeStatus::Running)
            .await
            .map_err(|e| e.to_string())?;
        
        let output = "simplified execution".to_string();
        
        db::nodes::update_status(&self.pool, node.id, NodeStatus::Done)
            .await
            .map_err(|e| e.to_string())?;
        
        Self::enqueue_downstream_nodes(&self.pool, &node).await?;
        
        tracing::info!(node_id = %node.id, "Node completed with output: {}", output);
        Ok(())
    }
}