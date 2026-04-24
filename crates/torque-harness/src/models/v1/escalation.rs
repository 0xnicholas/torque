use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Escalation {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub team_instance_id: Option<Uuid>,
    pub escalation_type: EscalationType,
    pub severity: EscalationSeverity,
    pub status: EscalationStatus,
    pub description: String,
    pub context: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum EscalationType {
    RecoveryFailed,
    TeamMemberFailed,
    ApprovalRequired,
    PolicyViolation,
    ResourceExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum EscalationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum EscalationStatus {
    Pending,
    Acknowledged,
    InProgress,
    Resolved,
    Cancelled,
}