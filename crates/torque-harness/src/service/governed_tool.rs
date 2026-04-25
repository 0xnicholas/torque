use crate::infra::tool_registry::{ToolExecutionContext, ToolRegistry};
use crate::policy::PolicyEvaluator;
use crate::policy::PolicyInput;
use crate::policy::PolicySources;
use crate::policy::ToolGovernanceService;
use crate::tools::{ToolArc, ToolResult};
use serde_json::Value;
use std::sync::Arc;

pub struct GovernedToolRegistry {
    inner: Arc<ToolRegistry>,
    governance: Arc<ToolGovernanceService>,
    policy_evaluator: PolicyEvaluator,
}

impl GovernedToolRegistry {
    pub fn new(inner: Arc<ToolRegistry>, governance: Arc<ToolGovernanceService>) -> Self {
        Self {
            inner,
            governance,
            policy_evaluator: PolicyEvaluator::new(),
        }
    }

    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        policy_sources: Option<&PolicySources>,
    ) -> anyhow::Result<ToolResult> {
        self.execute_with_context(name, args, policy_sources, ToolExecutionContext::default())
            .await
    }

    pub async fn execute_with_context(
        &self,
        name: &str,
        args: Value,
        policy_sources: Option<&PolicySources>,
        context: ToolExecutionContext,
    ) -> anyhow::Result<ToolResult> {
        if let Some(reason) = self.governance.should_block(name).await {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(reason),
            });
        }

        if let Some(sources) = policy_sources {
            let input = PolicyInput {
                action_type: "tool_call".to_string(),
                tool_name: Some(name.to_string()),
                ..Default::default()
            };
            let decision = self.policy_evaluator.evaluate(&input, sources);

            if !decision.allowed {
                return Ok(ToolResult {
                    success: false,
                    content: String::new(),
                    error: Some(
                        decision
                            .reasons
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "Tool blocked by policy".to_string()),
                    ),
                });
            }

            if decision.requires_approval {
                return Ok(ToolResult {
                    success: false,
                    content: "TOOL_REQUIRES_APPROVAL".to_string(),
                    error: Some("Tool requires approval before execution".to_string()),
                });
            }
        }

        self.inner.execute_with_context(name, args, context).await
    }

    pub async fn get(&self, name: &str) -> Option<ToolArc> {
        self.inner.get(name).await
    }

    pub async fn list(&self) -> Vec<ToolArc> {
        self.inner.list().await
    }

    pub async fn to_llm_tools(&self) -> Vec<llm::ToolDef> {
        self.inner.to_llm_tools().await
    }
}
