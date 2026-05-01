use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use super::{
    context::{AbortSignal, HookContext},
    definition::{get_hook_def, HookMode},
    handler::HookResult,
    input::HookInput,
    registry::HookRegistry,
};

/// Configurable hook executor.
///
/// Handles the sequential dispatch of hook handlers.
#[derive(Debug, Clone)]
pub struct HookExecutor {
    registry: Arc<HookRegistry>,
    config: HookExecutorConfig,
}

/// Configuration for the hook executor.
#[derive(Debug, Clone)]
pub struct HookExecutorConfig {
    /// Default timeout per handler (default: 30 s).
    pub default_handler_timeout: Duration,
    /// Stop on first rejection (default: true).
    pub stop_on_first_reject: bool,
}

impl Default for HookExecutorConfig {
    fn default() -> Self {
        Self {
            default_handler_timeout: Duration::from_secs(30),
            stop_on_first_reject: true,
        }
    }
}

impl HookExecutor {
    /// Create a new executor backed by the given registry.
    pub fn new(registry: Arc<HookRegistry>) -> Self {
        Self {
            registry,
            config: HookExecutorConfig::default(),
        }
    }

    /// Create a new executor with a custom config.
    pub fn with_config(registry: Arc<HookRegistry>, config: HookExecutorConfig) -> Self {
        Self { registry, config }
    }

    /// Execute all handlers for a given hook point.
    ///
    /// Handlers run **in registration order** (no concurrency).
    /// If an intercept handler returns `Rejected`, subsequent handlers
    /// are skipped (first-block-wins).
    pub async fn execute(
        &self,
        hook_name: &'static str,
        input: HookInput,
        agent_id: Option<torque_kernel::AgentInstanceId>,
    ) -> HookExecutionOutcome {
        let hook_def = get_hook_def(hook_name);
        let mode = hook_def.map(|d| d.mode).unwrap_or(HookMode::Observational);

        let handlers = self.registry.get_handlers(hook_name).await;
        if handlers.is_empty() {
            return HookExecutionOutcome::Passed(input);
        }

        let signal = AbortSignal::new();
        let mut current_input = input;

        for entry in &handlers {
            if signal.is_aborted() {
                return HookExecutionOutcome::Cancelled {
                    hook_name,
                    cancelled_at: entry.extension_id,
                };
            }

            let ctx = HookContext {
                extension_id: entry.extension_id,
                hook_name,
                agent_id,
                signal: signal.clone(),
            };

            let timer = timeout(
                entry.timeout.unwrap_or(self.config.default_handler_timeout),
                entry.handler.handle(&ctx, &current_input),
            );

            let result = match timer.await {
                Ok(r) => r,
                Err(_) => {
                    return HookExecutionOutcome::HandlerTimeout {
                        hook_name,
                        extension_id: entry.extension_id,
                    };
                }
            };

            match result {
                HookResult::Continue => {
                    // Observational: Continue means "recorded, move on"
                    // Intercept: Continue means "no modification, move on"
                }

                HookResult::Rejected { reason } => {
                    if mode == HookMode::Intercept && self.config.stop_on_first_reject {
                        return HookExecutionOutcome::Rejected {
                            hook_name,
                            reason,
                            rejected_by: entry.extension_id,
                        };
                    }
                    // Observational: log but don't stop
                    tracing::debug!(
                        "observational hook '{}' rejected by {}: {}",
                        hook_name,
                        entry.extension_id,
                        reason
                    );
                }

                HookResult::Modified(new_input) => {
                    if mode == HookMode::Intercept {
                        current_input = new_input;
                    }
                    // Observational: ignore modification
                }

                HookResult::ShortCircuit { value } => {
                    if mode == HookMode::Intercept {
                        return HookExecutionOutcome::ShortCircuited {
                            hook_name,
                            value,
                            triggered_by: entry.extension_id,
                        };
                    }
                    // Observational: log but don't short-circuit
                    tracing::debug!(
                        "observational hook '{}' short-circuit attempt by {}",
                        hook_name,
                        entry.extension_id
                    );
                }
            }
        }

        HookExecutionOutcome::Passed(current_input)
    }
}

/// Outcome of a full hook execution chain.
#[derive(Debug)]
pub enum HookExecutionOutcome {
    /// All handlers passed; execution may proceed (with possibly modified input).
    Passed(HookInput),
    /// A handler rejected the operation.
    Rejected {
        hook_name: &'static str,
        reason: String,
        rejected_by: crate::id::ExtensionId,
    },
    /// A handler short-circuited with an immediate value.
    ShortCircuited {
        hook_name: &'static str,
        value: serde_json::Value,
        triggered_by: crate::id::ExtensionId,
    },
    /// Execution was cancelled (AbortSignal triggered).
    Cancelled {
        hook_name: &'static str,
        cancelled_at: crate::id::ExtensionId,
    },
    /// A handler timed out.
    HandlerTimeout {
        hook_name: &'static str,
        extension_id: crate::id::ExtensionId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::handler::TestHandler;
    use crate::id::ExtensionId;

    fn make_input() -> HookInput {
        HookInput::ToolCall {
            tool: serde_json::json!("test_tool"),
            args: serde_json::json!({}),
        }
    }

    fn make_modified_input() -> HookInput {
        HookInput::ToolCall {
            tool: serde_json::json!("modified_tool"),
            args: serde_json::json!({ "modified": true }),
        }
    }

    #[tokio::test]
    async fn test_no_handlers_returns_passed() {
        let registry = Arc::new(HookRegistry::new());
        let executor = HookExecutor::new(registry);
        let input = make_input();
        let outcome = executor.execute("tool_call", input.clone(), None).await;
        match outcome {
            HookExecutionOutcome::Passed(modified) => {
                // No handlers, input should be unchanged
                assert!(matches!(modified, HookInput::ToolCall { tool, .. } if tool == "test_tool"));
            }
            _ => panic!("expected Passed, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_continue_passes_through() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();
        registry
            .register("tool_call", ext_id, Arc::new(TestHandler::always_continue()))
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("tool_call", make_input(), None).await;
        match outcome {
            HookExecutionOutcome::Passed(input) => {
                assert!(matches!(input, HookInput::ToolCall { tool, .. } if tool == "test_tool"));
            }
            _ => panic!("expected Passed, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_intercept_modified_propagates() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        let orig = make_input();
        let modified = make_modified_input();
        // Handler that modifies input
        registry
            .register(
                "tool_call",
                ext_id,
                Arc::new(TestHandler::new(HookResult::Modified(modified))),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("tool_call", orig, None).await;
        match outcome {
            HookExecutionOutcome::Passed(input) => {
                assert!(matches!(input, HookInput::ToolCall { tool, .. } if tool == "modified_tool"));
            }
            _ => panic!("expected Passed with modified input, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_intercept_rejected_blocks() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        registry
            .register(
                "tool_call",
                ext_id,
                Arc::new(TestHandler::new(HookResult::Rejected {
                    reason: "not allowed".into(),
                })),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("tool_call", make_input(), None).await;
        match outcome {
            HookExecutionOutcome::Rejected {
                reason,
                rejected_by,
                ..
            } => {
                assert_eq!(reason, "not allowed");
                assert_eq!(rejected_by, ext_id);
            }
            _ => panic!("expected Rejected, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_intercept_short_circuit() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        registry
            .register(
                "tool_call",
                ext_id,
                Arc::new(TestHandler::new(HookResult::ShortCircuit {
                    value: serde_json::json!("immediate_result"),
                })),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("tool_call", make_input(), None).await;
        match outcome {
            HookExecutionOutcome::ShortCircuited {
                value,
                triggered_by,
                ..
            } => {
                assert_eq!(value, "immediate_result");
                assert_eq!(triggered_by, ext_id);
            }
            _ => panic!("expected ShortCircuited, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_first_block_wins_rejected() {
        let registry = Arc::new(HookRegistry::new());
        let ext1 = ExtensionId::new();
        let ext2 = ExtensionId::new();

        registry
            .register(
                "tool_call",
                ext1,
                Arc::new(TestHandler::new(HookResult::Rejected {
                    reason: "blocked by first".into(),
                })),
            )
            .await
            .unwrap();
        registry
            .register(
                "tool_call",
                ext2,
                Arc::new(TestHandler::new(HookResult::Rejected {
                    reason: "should not be reached".into(),
                })),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("tool_call", make_input(), None).await;
        match outcome {
            HookExecutionOutcome::Rejected {
                reason, rejected_by, ..
            } => {
                assert_eq!(reason, "blocked by first");
                assert_eq!(rejected_by, ext1);
            }
            _ => panic!("expected Rejected, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_observational_rejected_does_not_block() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        // "turn_start" is observational
        registry
            .register(
                "turn_start",
                ext_id,
                Arc::new(TestHandler::new(HookResult::Rejected {
                    reason: "observational reject".into(),
                })),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        // Observational hooks should not block even on reject
        let outcome = executor.execute("turn_start", HookInput::TurnStart { turn_number: 1 }, None).await;
        match outcome {
            HookExecutionOutcome::Passed(_) => {} // expected
            _ => panic!("expected Passed for observational hook, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_observational_modified_does_not_propagate() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        registry
            .register(
                "turn_start",
                ext_id,
                Arc::new(TestHandler::new(HookResult::Modified(HookInput::TurnStart {
                    turn_number: 42,
                }))),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("turn_start", HookInput::TurnStart { turn_number: 1 }, None).await;
        match outcome {
            HookExecutionOutcome::Passed(HookInput::TurnStart { turn_number }) => {
                // Observational: modification should be ignored
                assert_eq!(turn_number, 1);
            }
            _ => panic!("expected Passed with original input, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_observational_short_circuit_does_not_short_circuit() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        registry
            .register(
                "turn_start",
                ext_id,
                Arc::new(TestHandler::new(HookResult::ShortCircuit {
                    value: serde_json::json!("should_not_appear"),
                })),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("turn_start", HookInput::TurnStart { turn_number: 1 }, None).await;
        match outcome {
            HookExecutionOutcome::Passed(_) => {} // expected
            _ => panic!("expected Passed for observational hook, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_handler_timeout() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        // Handler that sleeps forever
        struct SlowHandler;
        #[async_trait::async_trait]
        impl super::super::handler::HookHandler for SlowHandler {
            async fn handle(&self, _ctx: &HookContext, _input: &HookInput) -> HookResult {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                HookResult::Continue
            }
        }

        registry
            .register(
                "tool_call",
                ext_id,
                Arc::new(SlowHandler),
            )
            .await
            .unwrap();

        let config = HookExecutorConfig {
            default_handler_timeout: std::time::Duration::from_millis(10),
            stop_on_first_reject: true,
        };
        let executor = HookExecutor::with_config(registry.clone(), config);
        let outcome = executor.execute("tool_call", make_input(), None).await;
        match outcome {
            HookExecutionOutcome::HandlerTimeout {
                extension_id, ..
            } => {
                assert_eq!(extension_id, ext_id);
            }
            _ => panic!("expected HandlerTimeout, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_multiple_handlers_chain() {
        let registry = Arc::new(HookRegistry::new());
        let ext1 = ExtensionId::new();
        let ext2 = ExtensionId::new();

        // First handler modifies
        registry
            .register(
                "tool_call",
                ext1,
                Arc::new(TestHandler::new(HookResult::Modified(HookInput::ToolCall {
                    tool: serde_json::json!("step1_tool"),
                    args: serde_json::json!({ "step": 1 }),
                }))),
            )
            .await
            .unwrap();

        // Second handler continues (receives modified input)
        registry
            .register(
                "tool_call",
                ext2,
                Arc::new(TestHandler::always_continue()),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry.clone());
        let outcome = executor.execute("tool_call", make_input(), None).await;
        match outcome {
            HookExecutionOutcome::Passed(HookInput::ToolCall { tool, .. }) => {
                assert_eq!(tool, "step1_tool");
            }
            _ => panic!("expected Passed with modified input, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_custom_hook_name_with_no_def() {
        let registry = Arc::new(HookRegistry::new());
        let ext_id = ExtensionId::new();

        registry
            .register(
                "custom:my_hook",
                ext_id,
                Arc::new(TestHandler::always_continue()),
            )
            .await
            .unwrap();

        let executor = HookExecutor::new(registry);
        let outcome = executor.execute("custom:my_hook", make_input(), None).await;
        // Custom hooks are "Observational" by default (no HookPointDef)
        match outcome {
            HookExecutionOutcome::Passed(_) => {} // expected
            _ => panic!("expected Passed, got {:?}", outcome),
        }
    }
}
