use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    pub messages: Vec<Message>,
    pub tool_call_count: u32,
    pub intermediate_results: Vec<ArtifactPointer>,
    pub custom_state: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPointer {
    pub task_id: String,
    pub storage: String,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMeta {
    pub id: CheckpointId,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub state_hash: String,
}

#[async_trait]
pub trait Checkpointer: Send + Sync {
    async fn save(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        state: CheckpointState,
    ) -> Result<CheckpointId>;
    
    async fn load(&self, checkpoint_id: CheckpointId) -> Result<CheckpointState>;
    
    async fn list_run_checkpoints(&self, run_id: Uuid) -> Result<Vec<CheckpointMeta>>;
    
    async fn list_node_checkpoints(&self, node_id: Uuid) -> Result<Vec<CheckpointMeta>>;
    
    async fn delete(&self, checkpoint_id: CheckpointId) -> Result<()>;
}
