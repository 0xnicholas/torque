use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Planning,
    Pending,
    Running,
    Done,
    Failed,
    PlanningFailed,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RunStatus::Planning => "planning",
            RunStatus::Pending => "pending",
            RunStatus::Running => "running",
            RunStatus::Done => "done",
            RunStatus::Failed => "failed",
            RunStatus::PlanningFailed => "planning_failed",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: RunStatus,
    pub instruction: String,
    pub failure_policy: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl Run {
    pub fn new(tenant_id: Uuid, instruction: String, failure_policy: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            status: RunStatus::Planning,
            instruction,
            failure_policy,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
        }
    }
}
