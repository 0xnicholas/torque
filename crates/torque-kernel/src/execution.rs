use serde::{Deserialize, Serialize};

use crate::{
    agent_instance::AgentInstanceState,
    context_ref::ExternalContextRef,
    ids::{
        AgentDefinitionId, AgentInstanceId, ApprovalRequestId, ArtifactId, DelegationRequestId,
        ExecutionRequestId, TaskId,
    },
    runtime::ResumeSignal,
    task::TaskState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    Sync,
    Async,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    Continue,
    AwaitTool,
    AwaitApproval,
    AwaitDelegation,
    ProducedArtifacts,
    CompletedTask,
    FailedTask,
    SuspendedInstance,
}

impl ExecutionOutcome {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::CompletedTask | Self::FailedTask)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionEvent {
    InstanceStateChanged {
        from: AgentInstanceState,
        to: AgentInstanceState,
    },
    TaskStateChanged {
        from: TaskState,
        to: TaskState,
    },
    ApprovalRequested {
        approval_request_id: ApprovalRequestId,
    },
    DelegationRequested {
        delegation_request_id: DelegationRequestId,
    },
    ArtifactProduced {
        artifact_id: ArtifactId,
    },
    ResumeApplied {
        resume_signal: ResumeSignal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    id: ExecutionRequestId,
    agent_definition_id: AgentDefinitionId,
    instance_id: Option<AgentInstanceId>,
    goal: String,
    instructions: Vec<String>,
    input_artifact_ids: Vec<ArtifactId>,
    external_context_refs: Vec<ExternalContextRef>,
    constraints: Vec<String>,
    execution_mode: ExecutionMode,
    expected_outputs: Vec<String>,
    caller_ref: Option<String>,
    idempotency_key: Option<String>,
}

impl ExecutionRequest {
    pub fn new(
        agent_definition_id: AgentDefinitionId,
        goal: impl Into<String>,
        instructions: Vec<String>,
    ) -> Self {
        Self {
            id: ExecutionRequestId::new(),
            agent_definition_id,
            instance_id: None,
            goal: goal.into(),
            instructions,
            input_artifact_ids: Vec::new(),
            external_context_refs: Vec::new(),
            constraints: Vec::new(),
            execution_mode: ExecutionMode::Async,
            expected_outputs: Vec::new(),
            caller_ref: None,
            idempotency_key: None,
        }
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn agent_definition_id(&self) -> AgentDefinitionId {
        self.agent_definition_id
    }

    pub fn instructions(&self) -> &[String] {
        &self.instructions
    }

    pub fn constraints(&self) -> &[String] {
        &self.constraints
    }

    pub fn input_artifact_ids(&self) -> &[ArtifactId] {
        &self.input_artifact_ids
    }

    pub fn external_context_refs(&self) -> &[ExternalContextRef] {
        &self.external_context_refs
    }

    pub fn expected_outputs(&self) -> &[String] {
        &self.expected_outputs
    }

    pub fn instance_id(&self) -> Option<AgentInstanceId> {
        self.instance_id
    }

    pub fn execution_mode(&self) -> ExecutionMode {
        self.execution_mode
    }

    pub fn with_instance_id(mut self, instance_id: AgentInstanceId) -> Self {
        self.instance_id = Some(instance_id);
        self
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraints.push(constraint.into());
        self
    }

    pub fn with_input_artifact(mut self, artifact_id: ArtifactId) -> Self {
        self.input_artifact_ids.push(artifact_id);
        self
    }

    pub fn with_external_context_ref(mut self, external_context_ref: ExternalContextRef) -> Self {
        self.external_context_refs.push(external_context_ref);
        self
    }

    pub fn with_expected_output(mut self, expected_output: impl Into<String>) -> Self {
        self.expected_outputs.push(expected_output.into());
        self
    }

    pub fn with_execution_mode(mut self, execution_mode: ExecutionMode) -> Self {
        self.execution_mode = execution_mode;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub instance_id: AgentInstanceId,
    pub task_id: TaskId,
    pub sequence_number: u64,
    pub outcome: ExecutionOutcome,
    pub instance_state: AgentInstanceState,
    pub task_state: TaskState,
    pub artifact_ids: Vec<ArtifactId>,
    pub approval_request_ids: Vec<ApprovalRequestId>,
    pub delegation_request_ids: Vec<DelegationRequestId>,
    pub events: Vec<ExecutionEvent>,
    pub summary: Option<String>,
}
