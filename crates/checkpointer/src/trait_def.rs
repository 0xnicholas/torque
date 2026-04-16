use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::Result;

#[derive(Clone, Debug)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug)]
pub struct CheckpointMeta {
    pub id: CheckpointId,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub state_hash: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CheckpointState {
    pub data: serde_json::Value,
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
