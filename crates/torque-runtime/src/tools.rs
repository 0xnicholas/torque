use llm::{ToolCall as LlmToolCall, ToolDef as LlmToolDef};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOffloadRef {
    pub storage: String,
    pub locator: String,
    pub artifact_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeToolResult {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
    pub offload_ref: Option<RuntimeOffloadRef>,
}

impl RuntimeToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
            offload_ref: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            error: Some(error.into()),
            offload_ref: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl From<LlmToolDef> for RuntimeToolDef {
    fn from(value: LlmToolDef) -> Self {
        Self {
            name: value.name,
            description: value.description,
            parameters: value.parameters,
        }
    }
}

impl From<RuntimeToolDef> for LlmToolDef {
    fn from(value: RuntimeToolDef) -> Self {
        LlmToolDef {
            name: value.name,
            description: value.description,
            parameters: value.parameters,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl From<LlmToolCall> for RuntimeToolCall {
    fn from(value: LlmToolCall) -> Self {
        Self {
            id: value.id,
            name: value.name,
            arguments: value.arguments,
        }
    }
}

impl From<RuntimeToolCall> for LlmToolCall {
    fn from(value: RuntimeToolCall) -> Self {
        LlmToolCall {
            id: value.id,
            name: value.name,
            arguments: value.arguments,
        }
    }
}

pub type ToolExecutionResult = RuntimeToolResult;
