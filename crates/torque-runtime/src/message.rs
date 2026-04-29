use llm::Message as LlmMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeMessage {
    pub role: RuntimeMessageRole,
    pub content: String,
}

impl RuntimeMessage {
    pub fn new(role: RuntimeMessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::Assistant, content)
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::Tool, content)
    }
}

impl From<LlmMessage> for RuntimeMessage {
    fn from(value: LlmMessage) -> Self {
        let role = match value.role.as_str() {
            "system" => RuntimeMessageRole::System,
            "assistant" => RuntimeMessageRole::Assistant,
            "tool" => RuntimeMessageRole::Tool,
            _ => RuntimeMessageRole::User,
        };
        Self {
            role,
            content: value.content,
        }
    }
}

impl From<RuntimeMessage> for LlmMessage {
    fn from(value: RuntimeMessage) -> Self {
        let role = match value.role {
            RuntimeMessageRole::System => "system",
            RuntimeMessageRole::User => "user",
            RuntimeMessageRole::Assistant => "assistant",
            RuntimeMessageRole::Tool => "tool",
        };
        LlmMessage {
            role: role.to_string(),
            content: value.content,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
}

impl From<crate::checkpoint::Message> for RuntimeMessage {
    fn from(m: crate::checkpoint::Message) -> Self {
        let role = match m.role.as_str() {
            "system" => RuntimeMessageRole::System,
            "assistant" => RuntimeMessageRole::Assistant,
            "tool" => RuntimeMessageRole::Tool,
            _ => RuntimeMessageRole::User,
        };
        Self {
            role,
            content: m.content,
        }
    }
}
