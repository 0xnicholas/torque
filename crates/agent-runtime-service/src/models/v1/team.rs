use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct TeamDefinition {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub supervisor_agent_definition_id: Uuid,
    pub sub_agents: serde_json::Value,
    pub policy: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamDefinitionCreate {
    pub name: String,
    pub description: Option<String>,
    pub supervisor_agent_definition_id: Uuid,
    #[serde(default)]
    pub sub_agents: Vec<serde_json::Value>,
    #[serde(default)]
    pub policy: serde_json::Value,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TeamInstance {
    pub id: Uuid,
    pub team_definition_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamInstanceCreate {
    pub team_definition_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct TeamTaskCreate {
    pub goal: String,
    pub instructions: Option<String>,
    pub idempotency_key: String,
    #[serde(default)]
    pub input_artifacts: Vec<Uuid>,
    #[serde(default)]
    pub parent_task_id: Option<Uuid>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TeamMember {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub agent_instance_id: Uuid,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamMemberCreate {
    pub team_instance_id: Uuid,
    pub agent_instance_id: Uuid,
    #[serde(default = "default_member_role")]
    pub role: String,
}

fn default_member_role() -> String {
    "member".to_string()
}
