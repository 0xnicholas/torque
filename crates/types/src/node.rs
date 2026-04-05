use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
    PendingApproval,
    Cancelled,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeStatus::Pending => "pending",
            NodeStatus::Running => "running",
            NodeStatus::Done => "done",
            NodeStatus::Failed => "failed",
            NodeStatus::Skipped => "skipped",
            NodeStatus::PendingApproval => "pending_approval",
            NodeStatus::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub run_id: Uuid,
    pub tenant_id: Uuid,
    pub agent_type: String,
    pub fallback_agent_type: Option<String>,
    pub instruction: String,
    pub tools: Option<Vec<String>>,
    pub failure_policy: Option<String>,
    pub requires_approval: bool,
    pub status: NodeStatus,
    pub layer: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub error: Option<String>,
    pub executor_id: Option<String>,
}

impl Node {
    pub fn new(run_id: Uuid, tenant_id: Uuid, agent_type: String, instruction: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            run_id,
            tenant_id,
            agent_type,
            fallback_agent_type: None,
            instruction,
            tools: None,
            failure_policy: None,
            requires_approval: false,
            status: NodeStatus::Pending,
            layer: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            error: None,
            executor_id: None,
        }
    }
}
