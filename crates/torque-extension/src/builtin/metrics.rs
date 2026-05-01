//! Metrics Extension — Collects execution counters and exposes them
//!
//! ## Collected Metrics
//!
//! | Metric               | Source Hook            | Description                  |
//! |----------------------|------------------------|------------------------------|
//! | `tool_calls`         | `TOOL_CALL`            | Total tool invocations       |
//! | `tool_errors`        | `TOOL_RESULT` (error)  | Tool invocations that failed |
//! | `turns_completed`    | `TURN_END`            | Completed turns              |
//! | `errors`             | `ERROR`               | Execution errors             |
//! | `delegations`        | `DELEGATION_START`    | Delegation requests          |
//!
//! ## Supported Actions
//!
//! - `Query { key: "metrics" }` — returns all metrics as JSON
//! - `SetState { key: "reset", value: true }` — resets all counters

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    context::ExtensionContext,
    error::Result,
    hook::{
        context::HookContext,
        handler::HookResult,
        input::HookInput,
        HookHandler,
        definition::{
            TOOL_CALL, TOOL_RESULT, TURN_END, ERROR, DELEGATION_START,
        },
    },
    id::{ExtensionId, ExtensionVersion},
    message::{ExtensionAction, ExtensionMessage, ExtensionResponse, ResponseStatus},
    ExtensionActor,
};

// ── Handler ──────────────────────────────────────────────────────────

struct MetricsHandler {
    tool_calls: AtomicU64,
    tool_errors: AtomicU64,
    turns_completed: AtomicU64,
    errors: AtomicU64,
    delegations: AtomicU64,
    // Track the last couple of tool names for diagnostics
    last_tool: std::sync::Mutex<Option<String>>,
}

impl MetricsHandler {
    fn new() -> Self {
        Self {
            tool_calls: AtomicU64::new(0),
            tool_errors: AtomicU64::new(0),
            turns_completed: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            delegations: AtomicU64::new(0),
            last_tool: std::sync::Mutex::new(None),
        }
    }

    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "tool_calls": self.tool_calls.load(Ordering::Relaxed),
            "tool_errors": self.tool_errors.load(Ordering::Relaxed),
            "turns_completed": self.turns_completed.load(Ordering::Relaxed),
            "errors": self.errors.load(Ordering::Relaxed),
            "delegations": self.delegations.load(Ordering::Relaxed),
            "last_tool": self.last_tool.lock().unwrap().as_deref(),
        })
    }

    fn reset(&self) {
        self.tool_calls.store(0, Ordering::Relaxed);
        self.tool_errors.store(0, Ordering::Relaxed);
        self.turns_completed.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.delegations.store(0, Ordering::Relaxed);
        *self.last_tool.lock().unwrap() = None;
    }
}

#[async_trait]
impl HookHandler for MetricsHandler {
    async fn handle(&self, _ctx: &HookContext, input: &HookInput) -> HookResult {
        match input {
            HookInput::ToolCall { tool, .. } => {
                self.tool_calls.fetch_add(1, Ordering::Relaxed);
                *self.last_tool.lock().unwrap() = Some(tool.to_string());
            }
            HookInput::ToolResult { ref result, .. } => {
                // Heuristic: if the result contains an "error" key, count it as a tool error.
                if let Some(obj) = result.as_object() {
                    if obj.contains_key("error") {
                        self.tool_errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            HookInput::TurnEnd { .. } => {
                self.turns_completed.fetch_add(1, Ordering::Relaxed);
            }
            HookInput::Error { .. } => {
                self.errors.fetch_add(1, Ordering::Relaxed);
            }
            HookInput::DelegationStart { .. } => {
                self.delegations.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        HookResult::Continue
    }
}

// ── Actor ─────────────────────────────────────────────────────────────

/// A built-in Extension that collects execution metrics.
pub struct MetricsExtension {
    id: ExtensionId,
    version: ExtensionVersion,
    handler: Arc<MetricsHandler>,
}

impl MetricsExtension {
    pub fn new() -> Self {
        Self {
            id: ExtensionId::new(),
            version: ExtensionVersion::new(0, 1, 0),
            handler: Arc::new(MetricsHandler::new()),
        }
    }

    /// Return a snapshot of the current metrics as a JSON value.
    pub fn snapshot(&self) -> serde_json::Value {
        self.handler.snapshot()
    }
}

impl Default for MetricsExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtensionActor for MetricsExtension {
    fn id(&self) -> ExtensionId {
        self.id
    }

    fn name(&self) -> &'static str {
        "MetricsExtension"
    }

    fn version(&self) -> ExtensionVersion {
        self.version
    }

    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()> {
        let hooks = [
            TOOL_CALL,
            TOOL_RESULT,
            TURN_END,
            ERROR,
            DELEGATION_START,
        ];

        for hook_name in &hooks {
            ctx.register_hook(hook_name.name, self.handler.clone()).await?;
        }

        tracing::info!(extension = "MetricsExtension", "metrics extension started");
        Ok(())
    }

    async fn on_stop(&self, _ctx: &ExtensionContext) -> Result<()> {
        let metrics = self.handler.snapshot();
        tracing::info!(
            extension = "MetricsExtension",
            metrics = %metrics,
            "metrics extension stopped"
        );
        Ok(())
    }

    async fn handle(
        &self,
        _ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<ExtensionResponse> {
        match msg {
            ExtensionMessage::Command { action, .. } => match action {
                ExtensionAction::Query { key } if key == "metrics" => {
                    Ok(ExtensionResponse {
                        request_id: uuid::Uuid::new_v4(),
                        status: ResponseStatus::Success,
                        result: Some(self.handler.snapshot()),
                    })
                }
                ExtensionAction::SetState { key, value } if key == "reset" => {
                    if value.as_bool().unwrap_or(false) {
                        self.handler.reset();
                    }
                    Ok(ExtensionResponse {
                        request_id: uuid::Uuid::new_v4(),
                        status: ResponseStatus::Success,
                        result: Some(serde_json::json!({"reset": true})),
                    })
                }
                _ => Ok(ExtensionResponse {
                    request_id: uuid::Uuid::new_v4(),
                    status: ResponseStatus::Failure("unsupported action".into()),
                    result: Some(serde_json::json!({"error": "unsupported action"})),
                })
            },
            _ => Ok(ExtensionResponse {
                request_id: uuid::Uuid::new_v4(),
                status: ResponseStatus::Failure("unsupported message type".into()),
                result: Some(serde_json::json!({"error": "unsupported message type"})),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::in_memory::InMemoryExtensionRuntime;
    use crate::config::ExtensionConfig;
    use crate::runtime::ExtensionRuntime;
    use crate::hook::input::HookInput;

    #[tokio::test]
    async fn test_metrics_extension_register() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(MetricsExtension::new());

        let id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();
        assert_eq!(id, ext.id());

        let list = runtime.list().await;
        assert!(list.contains(&id));
    }

    #[tokio::test]
    async fn test_metrics_extension_snapshot_initially_zero() {
        let ext = MetricsExtension::new();
        let metrics = ext.snapshot();
        assert_eq!(metrics["tool_calls"], 0);
        assert_eq!(metrics["turns_completed"], 0);
        assert_eq!(metrics["errors"], 0);
        assert_eq!(metrics["delegations"], 0);
    }

    #[tokio::test]
    async fn test_metrics_extension_increments_on_hooks() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(MetricsExtension::new());
        let _id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();

        // Execute hooks that should increment counters
        runtime.execute_hook(
            "tool_call",
            HookInput::ToolCall { tool: serde_json::json!("search"), args: serde_json::json!({}) },
            None,
        ).await;

        runtime.execute_hook(
            "turn_end",
            HookInput::TurnEnd { turn_number: 1, response: serde_json::json!("ok") },
            None,
        ).await;

        runtime.execute_hook(
            "error",
            HookInput::Error { error: serde_json::json!("test error") },
            None,
        ).await;

        runtime.execute_hook(
            "delegation_start",
            HookInput::DelegationStart { delegation_id: uuid::Uuid::new_v4() },
            None,
        ).await;

        // Allow hooks to propagate
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let metrics = ext.snapshot();
        assert_eq!(metrics["tool_calls"], 1);
        assert_eq!(metrics["turns_completed"], 1);
        assert_eq!(metrics["errors"], 1);
        assert_eq!(metrics["delegations"], 1);
    }

    #[tokio::test]
    async fn test_metrics_extension_reset() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(MetricsExtension::new());
        let id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();

        // Trigger some metrics via hooks
        runtime.execute_hook(
            "tool_call",
            HookInput::ToolCall { tool: serde_json::json!("search"), args: serde_json::json!({}) },
            None,
        ).await;

        // Verify non-zero via direct snapshot
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(ext.snapshot()["tool_calls"], 1);

        // Reset via context.send() (fire-and-forget)
        let ctx = runtime.context(id).await.unwrap();
        ctx.send(
            id,
            ExtensionAction::SetState { key: "reset".into(), value: serde_json::json!(true) },
        ).unwrap();

        // Verify reset
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(ext.snapshot()["tool_calls"], 0);
    }

    #[tokio::test]
    async fn test_metrics_extension_query_via_call() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(MetricsExtension::new());
        let id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();

        let ctx = runtime.context(id).await.unwrap();
        let response = ctx.call(
            id,
            ExtensionAction::Query { key: "metrics".into() },
        ).await.unwrap();

        assert_eq!(response.status, ResponseStatus::Success);
        assert!(response.result.is_some());
        assert_eq!(response.result.unwrap()["tool_calls"], 0);
    }
}
