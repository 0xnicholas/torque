use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StreamEvent {
    Chunk {
        content: String,
    },
    ToolCall {
        name: String,
        arguments: Value,
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
    pub fn to_sse(&self) -> String {
        format!(
            "data: {}\n\n",
            serde_json::to_string(self).unwrap_or_default()
        )
    }
}
