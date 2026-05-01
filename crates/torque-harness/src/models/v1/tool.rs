use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request body for registering a new tool.
#[derive(Debug, Deserialize)]
pub struct ToolRegisterRequest {
    /// Unique tool name (LLM-facing identifier).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema describing the parameters this tool accepts.
    pub parameters_schema: Value,
    /// Optional risk level for governance (defaults to system config).
    #[serde(default)]
    pub risk_level: Option<String>,
    /// Whether this tool requires approval before execution.
    #[serde(default)]
    pub requires_approval: Option<bool>,
    /// Source label for tracking (e.g. "manual", "extension:<ext_id>").
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "manual".to_string()
}

/// Request body for updating an existing tool.
#[derive(Debug, Deserialize)]
pub struct ToolUpdateRequest {
    /// Updated description (optional).
    pub description: Option<String>,
    /// Updated JSON Schema (optional).
    pub parameters_schema: Option<Value>,
    /// Updated risk level (optional).
    pub risk_level: Option<String>,
    /// Updated approval requirement (optional).
    pub requires_approval: Option<bool>,
}

/// Response for a single tool listing.
#[derive(Debug, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters_schema: Value,
    pub source: String,
}

/// Response for listing all tools.
#[derive(Debug, Serialize)]
pub struct ToolListResponse {
    pub tools: Vec<ToolInfo>,
    pub total: usize,
}

/// Response for a register/update operation.
#[derive(Debug, Serialize)]
pub struct ToolRegisterResponse {
    pub name: String,
    pub message: String,
}
