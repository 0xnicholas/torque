use checkpointer::CheckpointState;
use serde::{Deserialize, Serialize};
use torque_kernel::AgentInstanceId;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCheckpointRef {
    pub checkpoint_id: Uuid,
    pub instance_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCheckpointPayload {
    pub instance_id: AgentInstanceId,
    pub node_id: Uuid,
    pub reason: String,
    pub state: CheckpointState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HydrationState {
    pub agent_definition_id: Uuid,
    pub status: String,
    pub active_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
}
