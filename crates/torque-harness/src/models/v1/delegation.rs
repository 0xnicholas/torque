use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DelegationStatus {
    Pending,
    Accepted,
    Rejected,
    Completed,
    Failed,
    TimeoutPartial,
}

impl DelegationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DelegationStatus::Completed | DelegationStatus::Failed | DelegationStatus::TimeoutPartial
        )
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct Delegation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
    pub status: DelegationStatus,
    pub result_artifact_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DelegationCreate {
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
}
