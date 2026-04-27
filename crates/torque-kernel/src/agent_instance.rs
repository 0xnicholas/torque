use serde::{Deserialize, Serialize};

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
    WaitingTool,
    WaitingSubagent,
    WaitingApproval,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInstance {
    id: AgentInstanceId,
    agent_definition_id: AgentDefinitionId,
    state: AgentInstanceState,
    active_task_id: Option<TaskId>,
    pending_approval_ids: Vec<ApprovalRequestId>,
    child_delegation_ids: Vec<DelegationRequestId>,
}

impl AgentInstance {
    pub fn new(agent_definition_id: AgentDefinitionId) -> Self {
        Self {
            id: AgentInstanceId::new(),
            agent_definition_id,
            state: AgentInstanceState::Created,
            active_task_id: None,
            pending_approval_ids: Vec::new(),
            child_delegation_ids: Vec::new(),
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
                AgentInstanceState::WaitingTool,
                AgentInstanceState::WaitingSubagent,
                AgentInstanceState::WaitingApproval,
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
                AgentInstanceState::WaitingTool,
                AgentInstanceState::WaitingSubagent,
                AgentInstanceState::WaitingApproval,
                AgentInstanceState::Suspended,
            ],
        )
    }

    pub fn wait_for_tool(&mut self) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::WaitingTool,
            &[AgentInstanceState::Running],
        )
    }

    pub fn wait_for_approval(
        &mut self,
        approval_id: ApprovalRequestId,
    ) -> Result<(), StateTransitionError> {
        self.transition(
            AgentInstanceState::WaitingApproval,
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
            AgentInstanceState::WaitingSubagent,
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
                AgentInstanceState::WaitingTool,
                AgentInstanceState::WaitingSubagent,
                AgentInstanceState::WaitingApproval,
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

    fn transition(
        &mut self,
        next: AgentInstanceState,
        allowed: &[AgentInstanceState],
    ) -> Result<(), StateTransitionError> {
        if allowed.contains(&self.state) {
            self.state = next;
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
