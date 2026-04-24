use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAnchor {
    pub anchor_type: ContextAnchorType,
    pub reference_id: Uuid,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextAnchorType {
    ExternalContextRef,
    Artifact,
    MemoryEntry,
    SharedState,
    EventAnchor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub task_id: Option<Uuid>,
    pub snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub context_anchors: Vec<ContextAnchor>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CheckpointRow {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub task_id: Option<Uuid>,
    pub snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub context_anchors: Option<serde_json::Value>,
}

impl From<CheckpointRow> for Checkpoint {
    fn from(row: CheckpointRow) -> Self {
        let context_anchors = row
            .context_anchors
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        Checkpoint {
            id: row.id,
            agent_instance_id: row.agent_instance_id,
            task_id: row.task_id,
            snapshot: row.snapshot,
            created_at: row.created_at,
            context_anchors,
        }
    }
}
