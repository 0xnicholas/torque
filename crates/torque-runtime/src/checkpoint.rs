use crate::tools::RuntimeToolCall;
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
    pub state: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HydrationState {
    pub agent_definition_id: Uuid,
    pub status: String,
    pub active_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
}

/// A chat message captured in a checkpoint snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<RuntimeToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A lightweight reference to an artifact stored in a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPointer {
    pub task_id: String,
    pub storage: String,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
}
