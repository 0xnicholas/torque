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

impl std::fmt::Display for DelegationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelegationStatus::Pending => write!(f, "PENDING"),
            DelegationStatus::Accepted => write!(f, "ACCEPTED"),
            DelegationStatus::Rejected => write!(f, "REJECTED"),
            DelegationStatus::Completed => write!(f, "COMPLETED"),
            DelegationStatus::Failed => write!(f, "FAILED"),
            DelegationStatus::TimeoutPartial => write!(f, "TIMEOUT_PARTIAL"),
        }
    }
}

impl TryFrom<&str> for DelegationStatus {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "PENDING" => Ok(DelegationStatus::Pending),
            "ACCEPTED" => Ok(DelegationStatus::Accepted),
            "REJECTED" => Ok(DelegationStatus::Rejected),
            "COMPLETED" => Ok(DelegationStatus::Completed),
            "FAILED" => Ok(DelegationStatus::Failed),
            "TIMEOUT_PARTIAL" => Ok(DelegationStatus::TimeoutPartial),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

impl TryFrom<String> for DelegationStatus {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}

impl DelegationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DelegationStatus::Completed
                | DelegationStatus::Failed
                | DelegationStatus::TimeoutPartial
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
