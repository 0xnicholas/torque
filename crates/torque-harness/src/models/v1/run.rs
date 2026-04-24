use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Serialize)]
pub struct RunEvent {
    pub event: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "run_status", rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Run {
    pub id: Uuid,
    pub webhook_url: Option<String>,
    pub async_execution: bool,
    pub status: RunStatus,
}