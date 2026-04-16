use serde::{Deserialize, Serialize};

use crate::ids::{AgentInstanceId, ApprovalRequestId, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalKind {
    ToolUse,
    Delegation,
    MemoryWrite,
    ExternalAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalRequestId,
    pub instance_id: AgentInstanceId,
    pub task_id: TaskId,
    pub kind: ApprovalKind,
    pub reason: String,
    pub state: ApprovalState,
}
