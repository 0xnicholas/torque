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
    AwaitingTool,
    AwaitingDelegation,
    AwaitingApproval,
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
            (TaskStatus::Created, TaskStatus::Running) => true,
            (TaskStatus::Queued, TaskStatus::Running) => true,
            (TaskStatus::Running, TaskStatus::AwaitingTool) => true,
            (TaskStatus::Running, TaskStatus::AwaitingDelegation) => true,
            (TaskStatus::Running, TaskStatus::AwaitingApproval) => true,
            (TaskStatus::Running, TaskStatus::Completed) => true,
            (TaskStatus::Running, TaskStatus::Failed) => true,
            (TaskStatus::AwaitingTool, TaskStatus::Running) => true,
            (TaskStatus::AwaitingTool, TaskStatus::Failed) => true,
            (TaskStatus::AwaitingDelegation, TaskStatus::Running) => true,
            (TaskStatus::AwaitingApproval, TaskStatus::Running) => true,
            (TaskStatus::AwaitingApproval, TaskStatus::Failed) => true,
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
