use serde::{Deserialize, Serialize};
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
