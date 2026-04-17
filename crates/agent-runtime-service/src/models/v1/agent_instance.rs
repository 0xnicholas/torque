use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentInstanceStatus {
    Created,
    Hydrating,
    Ready,
    Running,
    WaitingTool,
    WaitingSubagent,
    WaitingApproval,
    Suspended,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AgentInstance {
    pub id: Uuid,
    pub agent_definition_id: Uuid,
    pub status: AgentInstanceStatus,
    pub external_context_refs: serde_json::Value,
    pub current_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AgentInstanceCreate {
    pub agent_definition_id: Uuid,
    #[serde(default)]
    pub external_context_refs: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TimeTravelRequest {
    pub checkpoint_id: Uuid,
    pub branch_name: Option<String>,
}
