use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct Checkpoint {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub task_id: Option<Uuid>,
    pub snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
