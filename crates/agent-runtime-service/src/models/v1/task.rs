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

#[derive(Debug, sqlx::Type, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[sqlx(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    Queued,
    Running,
    WaitingTool,
    WaitingSubagent,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        match (self, next) {
            (TaskStatus::Created, TaskStatus::Queued) => true,
            (TaskStatus::Queued, TaskStatus::Running) => true,
            (TaskStatus::Running, TaskStatus::WaitingTool) => true,
            (TaskStatus::Running, TaskStatus::WaitingSubagent) => true,
            (TaskStatus::Running, TaskStatus::WaitingApproval) => true,
            (TaskStatus::Running, TaskStatus::Completed) => true,
            (TaskStatus::Running, TaskStatus::Failed) => true,
            (TaskStatus::WaitingTool, TaskStatus::Running) => true,
            (TaskStatus::WaitingTool, TaskStatus::Failed) => true,
            (TaskStatus::WaitingSubagent, TaskStatus::Running) => true,
            (TaskStatus::WaitingApproval, TaskStatus::Running) => true,
            (TaskStatus::WaitingApproval, TaskStatus::Failed) => true,
            (TaskStatus::Queued, TaskStatus::Cancelled) => true,
            (TaskStatus::Running, TaskStatus::Cancelled) => true,
            (s, t) if s == t => true, // same state is idempotent
            _ => false,
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub task_type: TaskType,
    pub parent_task_id: Option<Uuid>,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub status: TaskStatus,
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
