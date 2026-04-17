use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct AgentDefinition {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub tool_policy: serde_json::Value,
    pub memory_policy: serde_json::Value,
    pub delegation_policy: serde_json::Value,
    pub limits: serde_json::Value,
    pub default_model_policy: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AgentDefinitionCreate {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tool_policy: serde_json::Value,
    #[serde(default)]
    pub memory_policy: serde_json::Value,
    #[serde(default)]
    pub delegation_policy: serde_json::Value,
    #[serde(default)]
    pub limits: serde_json::Value,
    #[serde(default)]
    pub default_model_policy: serde_json::Value,
}
