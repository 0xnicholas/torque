//! Logging Extension — Observes Torque events and records them via `tracing`.
//!
//! ## Behaviour
//!
//! - On `on_start`: registers observational hooks for
//!   `TOOL_CALL`, `TOOL_RESULT`, `TURN_START`, `TURN_END`, `ERROR`, `CHECKPOINT`,
//!   `DELEGATION_START`, and `DELEGATION_COMPLETE`.
//! - Each hook invocation produces a `tracing::info!` span or event
//!   with structured fields.
//! - Responds to `ExtensionAction::Query { key: "stats" }` with a summary
//!   of events logged per hook point.

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
            TOOL_CALL, TOOL_RESULT, TURN_START, TURN_END,
            ERROR, CHECKPOINT, DELEGATION_START, DELEGATION_COMPLETE,
        },
    },
    id::{ExtensionId, ExtensionVersion},
    message::{ExtensionAction, ExtensionMessage, ExtensionResponse, ResponseStatus},
    ExtensionActor,
};

// ── Handler ──────────────────────────────────────────────────────────

struct LoggingHandler(Arc<AtomicU64>);

#[async_trait]
impl HookHandler for LoggingHandler {
    async fn handle(&self, _ctx: &HookContext, input: &HookInput) -> HookResult {
        self.0.fetch_add(1, Ordering::Relaxed);
        match input {
            HookInput::ToolCall { tool, args } => {
                tracing::info!(
                    hook = "tool_call",
                    tool = %tool,
                    args = %args,
                    "tool call intercepted"
                );
            }
            HookInput::ToolResult { tool, result } => {
                tracing::info!(
                    hook = "tool_result",
                    tool = %tool,
                    result = %result,
                    "tool result received"
                );
            }
            HookInput::TurnStart { turn_number } => {
                tracing::info!(hook = "turn_start", turn = turn_number, "turn started");
            }
            HookInput::TurnEnd { turn_number, response } => {
                tracing::info!(
                    hook = "turn_end",
                    turn = turn_number,
                    response = %response,
                    "turn ended"
                );
            }
            HookInput::Error { error } => {
                tracing::error!(hook = "error", error = %error, "execution error");
            }
            HookInput::Checkpoint { checkpoint } => {
                tracing::info!(hook = "checkpoint", checkpoint = %checkpoint, "checkpoint created");
            }
            HookInput::DelegationStart { delegation_id } => {
                tracing::info!(hook = "delegation_start", id = %delegation_id, "delegation started");
            }
            HookInput::DelegationComplete { delegation_id, result } => {
                tracing::info!(
                    hook = "delegation_complete",
                    id = %delegation_id,
                    result = %result,
                    "delegation completed"
                );
            }
            _ => {}
        }
        HookResult::Continue
    }
}

// ── Actor ─────────────────────────────────────────────────────────────

/// A built-in Extension that records hook lifecycle events via `tracing`.
pub struct LoggingExtension {
    id: ExtensionId,
    handler: Arc<LoggingHandler>,
    log_count: Arc<AtomicU64>,
}

impl LoggingExtension {
    pub fn new() -> Self {
        let log_count = Arc::new(AtomicU64::new(0));
        let handler = Arc::new(LoggingHandler(log_count.clone()));
        Self {
            id: ExtensionId::new(),
            handler,
            log_count,
        }
    }

    /// Total number of events logged since the extension started.
    pub fn log_count(&self) -> u64 {
        self.log_count.load(Ordering::Relaxed)
    }
}

impl Default for LoggingExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtensionActor for LoggingExtension {
    fn id(&self) -> ExtensionId {
        self.id
    }

    fn name(&self) -> &'static str {
        "LoggingExtension"
    }

    fn version(&self) -> ExtensionVersion {
        ExtensionVersion::new(0, 1, 0)
    }

    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()> {
        let handler = self.handler.clone();

        // Register observational hooks — all return Continue.
        for hook_name in &[
            TOOL_CALL,
            TOOL_RESULT,
            TURN_START,
            TURN_END,
            ERROR,
            CHECKPOINT,
            DELEGATION_START,
            DELEGATION_COMPLETE,
        ] {
            ctx.register_hook(hook_name.name, handler.clone()).await?;
        }

        tracing::info!(extension = "LoggingExtension", "logging extension started");
        Ok(())
    }

    async fn on_stop(&self, _ctx: &ExtensionContext) -> Result<()> {
        tracing::info!(
            extension = "LoggingExtension",
            total_logs = self.log_count(),
            "logging extension stopped"
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
                ExtensionAction::Query { key } if key == "stats" => {
                    Ok(ExtensionResponse {
                        request_id: uuid::Uuid::new_v4(),
                        status: ResponseStatus::Success,
                        result: Some(serde_json::json!({
                            "log_count": self.log_count(),
                            "extension": "LoggingExtension",
                        })),
                    })
                }
                _ => Ok(ExtensionResponse {
                    request_id: uuid::Uuid::new_v4(),
                    status: ResponseStatus::Failure("unsupported action".into()),
                    result: Some(serde_json::json!({"error": "unsupported action"})),
                }),
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
    use crate::message::{ExtensionAction, ExtensionMessage, ExtensionResponse, ResponseStatus};
    use crate::hook::input::HookInput;

    #[tokio::test]
    async fn test_logging_extension_register() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(LoggingExtension::new());

        let id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();
        assert_eq!(id, ext.id());

        let list = runtime.list().await;
        assert!(list.contains(&id));
    }

    #[tokio::test]
    async fn test_logging_extension_hook_increments_count() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(LoggingExtension::new());
        let _id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();

        assert_eq!(ext.log_count(), 0);

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

        // Allow hooks to propagate
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify counts via shared state
        assert_eq!(ext.log_count(), 3);
    }

    #[tokio::test]
    async fn test_logging_extension_query_via_call() {
        let runtime = InMemoryExtensionRuntime::new();
        let ext = Arc::new(LoggingExtension::new());
        let id = runtime.register(ext.clone(), ExtensionConfig::default()).await.unwrap();

        // Use context.call() for request-reply
        let ctx = runtime.context(id).await.unwrap();
        let response = ctx.call(id, ExtensionAction::Query { key: "stats".into() }).await.unwrap();

        assert_eq!(response.status, ResponseStatus::Success);
        assert!(response.result.is_some());
    }
}
