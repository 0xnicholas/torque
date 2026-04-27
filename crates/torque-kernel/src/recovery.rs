use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AgentInstanceId, AgentInstanceState, ApprovalRequestId, CheckpointId, DelegationRequestId,
    ExecutionOutcome, ExecutionResult, TaskId, TaskState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub instance_id: AgentInstanceId,
    pub active_task_id: Option<TaskId>,
    pub active_task_state: Option<TaskState>,
    pub instance_state: AgentInstanceState,
    pub pending_approval_ids: Vec<ApprovalRequestId>,
    pub child_delegation_ids: Vec<DelegationRequestId>,
    pub event_sequence: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointStateView {
    pub instance_id: AgentInstanceId,
    pub active_task_id: Option<TaskId>,
    pub active_task_state: Option<TaskState>,
    pub instance_state: AgentInstanceState,
    pub pending_approval_ids: Vec<ApprovalRequestId>,
    pub child_delegation_ids: Vec<DelegationRequestId>,
    pub event_sequence: u64,
    pub latest_outcome: Option<ExecutionOutcome>,
}

impl CheckpointStateView {
    pub fn to_json_value(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    pub fn from_json_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryView {
    pub checkpoint: Checkpoint,
    pub tail_events: Vec<ExecutionResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDisposition {
    ResumeCurrent,
    AwaitingApproval,
    AwaitingTool,
    AwaitingDelegation,
    Suspended,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    ReplayTailEvents,
    ResumeExecution,
    AwaitApprovalDecision,
    AwaitToolCompletion,
    AwaitDelegationCompletion,
    StaySuspended,
    AcceptCompletedState,
    EscalateFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryAssessment {
    pub view: RecoveryView,
    pub disposition: RecoveryDisposition,
    pub requires_replay: bool,
    pub latest_outcome: Option<ExecutionOutcome>,
    pub recommended_action: RecoveryAction,
}

impl RecoveryAssessment {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.disposition,
            RecoveryDisposition::Completed | RecoveryDisposition::Failed
        )
    }

    pub fn requires_operator_action(&self) -> bool {
        matches!(
            self.recommended_action,
            RecoveryAction::AwaitApprovalDecision
                | RecoveryAction::StaySuspended
                | RecoveryAction::EscalateFailure
        )
    }

    pub fn summary(&self) -> String {
        let disposition = match self.disposition {
            RecoveryDisposition::ResumeCurrent => "resume current execution",
            RecoveryDisposition::AwaitingApproval => "awaiting approval",
            RecoveryDisposition::AwaitingTool => "awaiting tool",
            RecoveryDisposition::AwaitingDelegation => "awaiting delegation",
            RecoveryDisposition::Suspended => "suspended",
            RecoveryDisposition::Completed => "completed",
            RecoveryDisposition::Failed => "failed",
        };

        let action = match self.recommended_action {
            RecoveryAction::ReplayTailEvents => "replay tail events",
            RecoveryAction::ResumeExecution => "resume execution",
            RecoveryAction::AwaitApprovalDecision => "await approval decision",
            RecoveryAction::AwaitToolCompletion => "await tool completion",
            RecoveryAction::AwaitDelegationCompletion => "await delegation completion",
            RecoveryAction::StaySuspended => "stay suspended",
            RecoveryAction::AcceptCompletedState => "accept completed state",
            RecoveryAction::EscalateFailure => "escalate failure",
        };

        let outcome = self
            .latest_outcome
            .map(|outcome| format!("{outcome:?}"))
            .unwrap_or_else(|| "none".to_string());

        format!("{disposition}; {action}; latest={outcome}")
    }
}
