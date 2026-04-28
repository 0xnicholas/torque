use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt::Display;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct RunRequest {
    pub goal: String,
    pub instructions: Option<String>,
    #[serde(default)]
    pub input_artifacts: Vec<Uuid>,
    #[serde(default)]
    pub external_context_refs: Vec<serde_json::Value>,
    #[serde(default)]
    pub constraints: serde_json::Value,
    #[serde(default)]
    pub execution_mode: String,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub async_execution: bool,
    #[serde(default)]
    pub agent_instance_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct RunEvent {
    pub event: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "run_status", rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Queued => write!(f, "queued"),
            RunStatus::Running => write!(f, "running"),
            RunStatus::Completed => write!(f, "completed"),
            RunStatus::Failed => write!(f, "failed"),
            RunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Run {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: RunStatus,
    pub agent_instance_id: Uuid,
    pub instruction: String,
    pub request_payload: serde_json::Value,
    pub failure_policy: Option<String>,
    pub webhook_url: Option<String>,
    pub async_execution: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
    pub webhook_sent_at: Option<chrono::DateTime<chrono::Utc>>,
    pub webhook_attempts: Option<i32>,
}
