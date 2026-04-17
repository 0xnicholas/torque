use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum TaskType {
    AgentTask,
    TeamTask,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub task_type: TaskType,
    pub parent_task_id: Option<Uuid>,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub status: String,
    pub goal: String,
    pub instructions: Option<String>,
    pub input_artifacts: serde_json::Value,
    pub produced_artifacts: serde_json::Value,
    pub delegation_ids: serde_json::Value,
    pub approval_ids: serde_json::Value,
    pub checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
