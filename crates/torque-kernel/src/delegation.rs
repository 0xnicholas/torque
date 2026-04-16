use serde::{Deserialize, Serialize};

use crate::ids::{AgentDefinitionId, AgentInstanceId, ArtifactId, DelegationRequestId, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegationState {
    Open,
    InProgress,
    Completed,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationRequest {
    pub id: DelegationRequestId,
    pub parent_instance_id: AgentInstanceId,
    pub parent_task_id: TaskId,
    pub child_agent_definition_id: AgentDefinitionId,
    pub goal: String,
    pub instructions: Vec<String>,
    pub input_refs: Vec<String>,
    pub constraints: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub state: DelegationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationResult {
    pub delegation_request_id: DelegationRequestId,
    pub state: DelegationState,
    pub artifact_ids: Vec<ArtifactId>,
    pub summary: Option<String>,
}
