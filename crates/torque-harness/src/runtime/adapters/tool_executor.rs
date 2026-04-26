use crate::infra::tool_registry::ToolExecutionContext;
use crate::service::ToolService;
use async_trait::async_trait;
use std::sync::Arc;
use torque_runtime::environment::{RuntimeExecutionContext, RuntimeToolExecutor};
use torque_runtime::tools::{RuntimeToolDef, RuntimeToolResult};

pub struct HarnessToolExecutor {
    tools: Arc<ToolService>,
}

impl HarnessToolExecutor {
    pub fn new(tools: Arc<ToolService>) -> Self {
        Self { tools }
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
            .tools
            .registry()
            .execute_with_context(
                tool_name,
                arguments,
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
            .tools
            .registry()
            .to_llm_tools()
            .await
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
