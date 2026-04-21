use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct Delegation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DelegationCreate {
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
}
