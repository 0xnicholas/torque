use crate::tools::{RuntimeToolCall, RuntimeToolResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCheckpointEvent {
    pub checkpoint_id: Uuid,
    pub instance_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTurnResult {
    pub finish_reason: RuntimeFinishReason,
    pub assistant_text: String,
    pub tool_calls: Vec<RuntimeToolCall>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RuntimeOutputEvent {
    TextChunk { chunk: String },
    ToolCall { call: RuntimeToolCall },
    ToolResult { tool_name: String, result: RuntimeToolResult },
    CheckpointCreated { checkpoint_id: Uuid, reason: String },
}
