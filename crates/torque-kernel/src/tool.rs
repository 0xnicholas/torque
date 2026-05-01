//! Core Tool contracts for the Torque kernel.
//!
//! These types define the minimal interface that every Tool must implement.
//! They are shared across `torque-extension`, `torque-harness`, and any other
//! crate that needs to define, register, or execute tools.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// The result of a single tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
}

/// The core Tool trait that every tool must implement.
///
/// Tools are the primitive unit of extensible capability in Torque.
/// An LLM can discover tools through their [`name`] and [`description`],
/// decide whether to call them based on [`parameters_schema`], and the
/// actual work is performed by [`execute`].
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name identifying this tool to the LLM.
    fn name(&self) -> &str;

    /// Human-readable description of what this tool does.
    fn description(&self) -> &str;

    /// JSON Schema describing the parameters this tool accepts.
    fn parameters_schema(&self) -> Value;

    /// Execute the tool with the given JSON arguments.
    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult>;
}

/// A thread-safe, reference-counted pointer to a dynamic Tool.
pub type ToolArc = Arc<dyn Tool>;
