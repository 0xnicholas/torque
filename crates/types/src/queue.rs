use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Pending,
    Locked,
    Done,
}

impl std::fmt::Display for QueueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            QueueStatus::Pending => "pending",
            QueueStatus::Locked => "locked",
            QueueStatus::Done => "done",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub priority: i32,
    pub status: QueueStatus,
    pub available_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl QueueEntry {
    pub fn new(tenant_id: Uuid, run_id: Uuid, node_id: Uuid, priority: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            run_id,
            node_id,
            priority,
            status: QueueStatus::Pending,
            available_at: Utc::now(),
            locked_at: None,
            locked_by: None,
            created_at: Utc::now(),
        }
    }
}
