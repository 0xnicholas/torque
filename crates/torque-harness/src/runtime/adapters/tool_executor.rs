use crate::infra::tool_registry::ToolExecutionContext;
use crate::service::governed_tool::GovernedToolRegistry;
use async_trait::async_trait;
use std::sync::Arc;
use torque_runtime::environment::{RuntimeExecutionContext, RuntimeToolExecutor};
use torque_runtime::tools::{RuntimeToolDef, RuntimeToolResult};

/// Production tool executor that routes through the governed registry.
///
/// All tool calls pass through `GovernedToolRegistry`, which applies
/// blocking checks and policy evaluation before delegating to the
/// inner `ToolRegistry`. This ensures the production agent path
/// respects tool governance rules.
pub struct HarnessToolExecutor {
    governed: Arc<GovernedToolRegistry>,
}

impl HarnessToolExecutor {
    pub fn new(governed: Arc<GovernedToolRegistry>) -> Self {
        Self { governed }
    }
}

#[async_trait]
impl RuntimeToolExecutor for HarnessToolExecutor {
    async fn execute(
        &self,
        ctx: RuntimeExecutionContext,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<RuntimeToolResult> {
        let result = self
            .governed
            .execute_with_context(
                tool_name,
                arguments,
                None, // policy_sources not available at runtime boundary; blocking still applies
                ToolExecutionContext {
                    source_instance_id: Some(ctx.instance_id),
                },
            )
            .await?;

        Ok(RuntimeToolResult {
            success: result.success,
            content: result.content,
            error: result.error,
            offload_ref: None,
        })
    }

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> {
        Ok(self
            .governed
            .to_llm_tools()
            .await
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
