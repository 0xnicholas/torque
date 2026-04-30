use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{
    error::StateTransitionError,
    ids::{AgentDefinitionId, AgentInstanceId, ApprovalRequestId, DelegationRequestId, TaskId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentInstanceState {
    Created,
    Hydrating,
    Ready,
    Running,
    AwaitingTool,
    AwaitingDelegation,
    AwaitingApproval,
    Suspended,
    /// Terminal: task completed successfully.
    Completed,
    /// Terminal: execution failed.
    Failed,
    /// Terminal: execution was cancelled.
    Cancelled,
}

impl AgentInstanceState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl fmt::Display for AgentInstanceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Hydrating => write!(f, "hydrating"),
            Self::Ready => write!(f, "ready"),
            Self::Running => write!(f, "running"),
            Self::AwaitingTool => write!(f, "awaiting_tool"),
            Self::AwaitingDelegation => write!(f, "awaiting_delegation"),
            Self::AwaitingApproval => write!(f, "awaiting_approval"),
            Self::Suspended => write!(f, "suspended"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInstance {
    id: AgentInstanceId,
    agent_definition_id: AgentDefinitionId,
    state: AgentInstanceState,
    active_task_id: Option<TaskId>,
    pending_approval_ids: Vec<ApprovalRequestId>,
    child_delegation_ids: Vec<DelegationRequestId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl AgentInstance {
    pub fn new(agent_definition_id: AgentDefinitionId) -> Self {
        let now = Utc::now();
        Self {
            id: AgentInstanceId::new(),
            agent_definition_id,
            state: AgentInstanceState::Created,
            active_task_id: None,
            pending_approval_ids: Vec::new(),
            child_delegation_ids: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn id(&self) -> AgentInstanceId {
        self.id
    }

    pub fn agent_definition_id(&self) -> AgentDefinitionId {
        self.agent_definition_id
    }

    pub fn state(&self) -> AgentInstanceState {
        self.state
    }

    pub fn active_task_id(&self) -> Option<TaskId> {
        self.active_task_id
    }

    pub fn pending_approval_ids(&self) -> &[ApprovalRequestId] {
        &self.pending_approval_ids
    }

    pub fn child_delegation_ids(&self) -> &[DelegationRequestId] {
        &self.child_delegation_ids
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    pub fn begin_hydrating(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Hydrating,
            &[AgentInstanceState::Created],
        )
    }

    pub fn mark_ready(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Ready,
            &[
                AgentInstanceState::Hydrating,
                AgentInstanceState::Running,
                AgentInstanceState::AwaitingTool,
                AgentInstanceState::AwaitingDelegation,
                AgentInstanceState::AwaitingApproval,
                AgentInstanceState::Suspended,
            ],
        )
    }

    pub fn begin_running(&mut self) -> Result<(), StateTransitionError> {
        self.transition(AgentInstanceState::Running, &[AgentInstanceState::Ready])
    }

    pub fn resume_running(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Running,
            &[
                AgentInstanceState::AwaitingTool,
                AgentInstanceState::AwaitingDelegation,
                AgentInstanceState::AwaitingApproval,
                AgentInstanceState::Suspended,
            ],
        )
    }

    pub fn wait_for_tool(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::AwaitingTool,
            &[AgentInstanceState::Running],
        )
    }

    pub fn wait_for_approval(
        &mut self,
        approval_id: ApprovalRequestId,
    ) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::AwaitingApproval,
            &[AgentInstanceState::Running],
        )?;
        self.pending_approval_ids.push(approval_id);
        Ok(())
    }

    pub fn wait_for_delegation(
        &mut self,
        delegation_id: DelegationRequestId,
    ) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::AwaitingDelegation,
            &[AgentInstanceState::Running],
        )?;
        self.child_delegation_ids.push(delegation_id);
        Ok(())
    }

    pub fn suspend(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Suspended,
            &[
                AgentInstanceState::Running,
                AgentInstanceState::AwaitingTool,
                AgentInstanceState::AwaitingDelegation,
                AgentInstanceState::AwaitingApproval,
            ],
        )
    }

    pub fn bind_active_task(&mut self, task_id: TaskId) -> Result<(), StateTransitionError> {
        match self.active_task_id {
            Some(existing) if existing != task_id => Err(StateTransitionError::new(
                "AgentInstance",
                "bound_to_other_task",
                "bind_new_active_task",
            )),
            _ => {
                self.active_task_id = Some(task_id);
                Ok(())
            }
        }
    }

    pub fn clear_active_task(&mut self) {
        self.active_task_id = None;
    }

    pub fn resolve_approval(
        &mut self,
        approval_id: ApprovalRequestId,
    ) -> Result<(), StateTransitionError> {
        if let Some(index) = self
            .pending_approval_ids
            .iter()
            .position(|pending| *pending == approval_id)
        {
            self.pending_approval_ids.remove(index);
            Ok(())
        } else {
            Err(StateTransitionError::new(
                "AgentInstance",
                "approval_not_pending",
                "resolve_approval",
            ))
        }
    }

    pub fn resolve_delegation(
        &mut self,
        delegation_id: DelegationRequestId,
    ) -> Result<(), StateTransitionError> {
        if let Some(index) = self
            .child_delegation_ids
            .iter()
            .position(|pending| *pending == delegation_id)
        {
            self.child_delegation_ids.remove(index);
            Ok(())
        } else {
            Err(StateTransitionError::new(
                "AgentInstance",
                "delegation_not_pending",
                "resolve_delegation",
            ))
        }
    }

    pub fn complete(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Completed,
            &[
                AgentInstanceState::Running,
                AgentInstanceState::AwaitingTool,
                AgentInstanceState::AwaitingDelegation,
                AgentInstanceState::AwaitingApproval,
            ],
        )
    }

    pub fn fail(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Failed,
            &[
                AgentInstanceState::Running,
                AgentInstanceState::AwaitingTool,
                AgentInstanceState::AwaitingDelegation,
                AgentInstanceState::AwaitingApproval,
            ],
        )
    }

    pub fn cancel(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::Cancelled,
            &[
                AgentInstanceState::Running,
                AgentInstanceState::AwaitingTool,
                AgentInstanceState::AwaitingDelegation,
                AgentInstanceState::AwaitingApproval,
                AgentInstanceState::Suspended,
            ],
        )
    }

    fn transition(
        &mut self,
        next: AgentInstanceState,
        allowed: &[AgentInstanceState],
    ) -> Result<(), StateTransitionError> {
        if allowed.contains(&self.state) {
            self.state = next;
            self.updated_at = Utc::now();
            Ok(())
        } else {
            Err(StateTransitionError::new(
                "AgentInstance",
                format!("{:?}", self.state),
                format!("{next:?}"),
            ))
        }
    }
}
