use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StreamEvent {
    Start {
        session_id: Uuid,
    },
    Chunk {
        content: String,
    },
    ToolCall {
        name: String,
        arguments: Value,
    },
    ToolResult {
        name: String,
        success: bool,
        content: String,
        error: Option<String>,
    },
    Done {
        message_id: Uuid,
        artifacts: Option<Value>,
    },
    Error {
        code: String,
        message: String,
    },
}

impl StreamEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            StreamEvent::Start { .. } => "start",
            StreamEvent::Chunk { .. } => "chunk",
            StreamEvent::ToolCall { .. } => "tool_call",
            StreamEvent::ToolResult { .. } => "tool_result",
            StreamEvent::Done { .. } => "done",
            StreamEvent::Error { .. } => "error",
        }
    }

    pub fn to_sse(&self) -> String {
        format!(
            "data: {}\n\n",
            serde_json::to_string(self).unwrap_or_default()
        )
    }
}
