use async_trait::async_trait;

use super::context::HookContext;
use super::input::HookInput;

/// Result returned by a hook handler.
#[derive(Debug)]
pub enum HookResult {
    /// Continue to the next handler (or proceed with execution).
    Continue,
    /// Reject the operation — subsequent handlers are skipped.
    Rejected { reason: String },
    /// Modify the input — the modified value is passed to the next handler.
    Modified(HookInput),
    /// Short-circuit execution with an immediate value.
    ShortCircuit { value: serde_json::Value },
}

/// Trait that every hook handler must implement.
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Handle a hook invocation.
    ///
    /// The handler receives:
    /// - `ctx`: context with access to the AbortSignal and metadata
    /// - `input`: the current hook input (possibly modified by previous handlers)
    async fn handle(&self, ctx: &HookContext, input: &HookInput) -> HookResult;

    /// Optional human-readable name for diagnostics.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[cfg(test)]
pub(crate) struct TestHandler {
    result: std::sync::Mutex<std::option::Option<HookResult>>,
}

#[cfg(test)]
impl TestHandler {
    pub(crate) fn new(result: HookResult) -> Self {
        Self {
            result: std::sync::Mutex::new(Some(result)),
        }
    }

    pub(crate) fn always_continue() -> Self {
        Self::new(HookResult::Continue)
    }
}

#[cfg(test)]
impl Default for TestHandler {
    fn default() -> Self {
        Self::always_continue()
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl HookHandler for TestHandler {
    async fn handle(&self, _ctx: &HookContext, _input: &HookInput) -> HookResult {
        self.result
            .lock()
            .unwrap()
            .take()
            .unwrap_or(HookResult::Continue)
    }

    fn name(&self) -> &'static str {
        "TestHandler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_result_continue() {
        assert!(matches!(HookResult::Continue, HookResult::Continue));
    }

    #[test]
    fn test_hook_result_rejected() {
        let result = HookResult::Rejected {
            reason: "not allowed".into(),
        };
        match result {
            HookResult::Rejected { reason } => assert_eq!(reason, "not allowed"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_result_modified() {
        let input = HookInput::ToolCall {
            tool: serde_json::json!("test_tool"),
            args: serde_json::json!({}),
        };
        let result = HookResult::Modified(input);
        match &result {
            HookResult::Modified(HookInput::ToolCall { tool, .. }) => {
                assert_eq!(*tool, serde_json::json!("test_tool"));
            }
            _ => panic!("wrong variant"),
        }
        // Verify debug output
        let debug = format!("{:?}", result);
        assert!(debug.contains("Modified"));
    }

    #[test]
    fn test_hook_result_short_circuit() {
        let result = HookResult::ShortCircuit {
            value: serde_json::json!("done"),
        };
        match result {
            HookResult::ShortCircuit { value } => assert_eq!(value, serde_json::json!("done")),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_handler_default_name() {
        assert_eq!(TestHandler::default().name(), "TestHandler");
    }
}
