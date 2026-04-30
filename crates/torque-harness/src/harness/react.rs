use crate::agent::stream::StreamEvent;
use crate::infra::llm::{Chunk, LlmClient, LlmMessage, ToolCall};
use crate::infra::tool_registry::{ToolExecutionContext, ToolRegistry};
use crate::models::v1::team::{ProcessingPath, TaskComplexity, TeamMode, TriageResult};
use crate::service::governed_tool::GovernedToolRegistry;
use crate::tools::{ToolArc, ToolResult};
use llm::{ChatRequest, FinishReason, Message, ResponseFormat};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_runtime::StepDecision;

/// Maximum consecutive tool failures before aborting execution.
const MAX_CONSECUTIVE_TOOL_FAILURES: usize = 3;

pub enum ToolExecution {
    Registry(Arc<ToolRegistry>),
    Governed(Arc<GovernedToolRegistry>),
}

impl ToolExecution {
    pub async fn execute(&self, name: &str, args: Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(name, args, ToolExecutionContext::default())
            .await
    }

    pub async fn execute_with_context(
        &self,
        name: &str,
        args: Value,
        context: ToolExecutionContext,
    ) -> anyhow::Result<ToolResult> {
        match self {
            ToolExecution::Registry(r) => r.execute_with_context(name, args, context).await,
            ToolExecution::Governed(g) => g.execute_with_context(name, args, None, context).await,
        }
    }

    pub async fn list(&self) -> Vec<ToolArc> {
        match self {
            ToolExecution::Registry(r) => r.list().await,
            ToolExecution::Governed(g) => g.list().await,
        }
    }

    pub async fn to_llm_tools(&self) -> Vec<llm::ToolDef> {
        match self {
            ToolExecution::Registry(r) => r.to_llm_tools().await,
            ToolExecution::Governed(g) => g.to_llm_tools().await,
        }
    }
}

/// A provider-agnostic agent loop driven by native tool calling.
///
/// The loop continues until the LLM returns `finish_reason = Stop`
/// (task complete) or an unrecoverable error occurs. There is no
/// maximum step limit — the LLM decides when the task is done via
/// its `finish_reason`.
///
/// **DEPRECATED**: New code should use `RuntimeHost::execute_v1_with_queue`
/// via `RunService`.  `AgentLoop` remains solely for `SupervisorAgent`
/// team orchestration and will be replaced by a team-native harness in
/// a future iteration.
pub struct AgentLoop {
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolExecution>,
    turn_count: u32,
    execution_context: ToolExecutionContext,
}

impl AgentLoop {
    pub fn new(llm: Arc<dyn LlmClient>, tools: Arc<ToolExecution>) -> Self {
        Self {
            llm,
            tools,
            turn_count: 0,
            execution_context: ToolExecutionContext::default(),
        }
    }

    pub fn set_execution_context(&mut self, execution_context: ToolExecutionContext) {
        self.execution_context = execution_context;
    }

    /// Returns the number of LLM turns executed so far.
    pub fn turn_count(&self) -> u32 {
        self.turn_count
    }

    /// Run the agent loop for a given task.
    ///
    /// The loop:
    /// 1. Sends the system prompt + task to the LLM (with tool definitions)
    /// 2. If `finish_reason = ToolCalls`, executes each tool call and feeds
    ///    results back to the LLM
    /// 3. Repeats until `finish_reason = Stop` or an error occurs
    ///
    /// A `CancellationToken` (not yet wired) should be checked at the top
    /// of each loop iteration when available.
    pub async fn run(
        &mut self,
        task: &str,
        system_prompt: Option<&str>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<StepDecision, AgentLoopError> {
        let mut messages = self.build_initial_messages(task, system_prompt);
        let mut consecutive_failures = 0;

        loop {
            self.turn_count += 1;

            let (decision, tool_calls) = self
                .execute_step(&mut messages, event_sink.clone())
                .await?;

            match decision {
                StepDecision::Continue => {
                    continue;
                }
                StepDecision::AwaitTool => {
                    for tool_call in tool_calls {
                        let result = self.execute_tool(&tool_call, event_sink.clone()).await?;
                        consecutive_failures = if result.success {
                            0
                        } else {
                            consecutive_failures + 1
                        };
                        // Feed tool result back as structured JSON message.
                        messages.push(LlmMessage::tool(
                            &tool_call.id,
                            &serde_json::json!({
                                "tool": tool_call.name,
                                "success": result.success,
                                "content": result.content,
                                "error": result.error,
                            })
                            .to_string(),
                        ));
                        if consecutive_failures >= MAX_CONSECUTIVE_TOOL_FAILURES {
                            return Ok(StepDecision::FailTask(format!(
                                "Tool execution failed {} times consecutively",
                                consecutive_failures
                            )));
                        }
                    }
                }
                StepDecision::CompleteTask(summary) => {
                    return Ok(StepDecision::CompleteTask(summary));
                }
                StepDecision::FailTask(reason) => {
                    return Ok(StepDecision::FailTask(reason));
                }
                StepDecision::SuspendInstance => {
                    return Ok(StepDecision::SuspendInstance);
                }
                StepDecision::AwaitApproval(id) => {
                    return Ok(StepDecision::AwaitApproval(id.clone()));
                }
                StepDecision::AwaitDelegation(id) => {
                    return Ok(StepDecision::AwaitDelegation(id.clone()));
                }
                StepDecision::ProduceArtifacts(ids) => {
                    return Ok(StepDecision::ProduceArtifacts(ids.clone()));
                }
            }
        }
    }

    fn build_initial_messages(&self, task: &str, system_prompt: Option<&str>) -> Vec<LlmMessage> {
        let mut messages = Vec::new();

        let default_system = "\
You are a capable agent with access to tools.

When given a task, use the available tools to accomplish it. \
You may call multiple tools in sequence if needed. \
Call tools directly — do not describe them in text.

When the task is complete, provide a clear summary of what was accomplished.";

        messages.push(LlmMessage::system(
            system_prompt.unwrap_or(default_system),
        ));
        messages.push(LlmMessage::user(task.to_string()));

        messages
    }

    async fn execute_step(
        &self,
        messages: &mut Vec<LlmMessage>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<(StepDecision, Vec<ToolCall>), AgentLoopError> {
        let tool_defs = self.tools.to_llm_tools().await;

        let content_buffer = Arc::new(std::sync::Mutex::new(String::new()));
        let tool_calls_buffer: Arc<std::sync::Mutex<Vec<ToolCall>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let content_buffer_clone = content_buffer.clone();
        let tool_calls_buffer_clone = tool_calls_buffer.clone();
        let tx_clone = event_sink.clone();

        let callback = Box::new(move |chunk: Chunk| {
            if let Some(tool_call) = &chunk.tool_call {
                let mut calls = tool_calls_buffer_clone.lock().unwrap();
                if !calls.iter().any(|t| t.id == tool_call.id) {
                    calls.push(tool_call.clone());
                }
            }
            if !chunk.content.is_empty() {
                content_buffer_clone
                    .lock()
                    .unwrap()
                    .push_str(&chunk.content);
                let _ = tx_clone.try_send(StreamEvent::Chunk {
                    content: chunk.content.clone(),
                });
            }
        });

        let request =
            ChatRequest::new(self.llm.model().to_string(), messages.clone()).with_tools(tool_defs);

        let response = self
            .llm
            .chat_streaming(request, callback)
            .await
            .map_err(|e| AgentLoopError::LlmError(e.to_string()))?;

        let content = Arc::try_unwrap(content_buffer)
            .map(|m| m.into_inner().unwrap_or_default())
            .unwrap_or_default();
        let tool_calls = Arc::try_unwrap(tool_calls_buffer)
            .map(|m| m.into_inner().unwrap_or_default())
            .unwrap_or_default();

        // Build assistant message with native tool calls.
        let mut assistant_msg = LlmMessage::assistant(&content);
        if !tool_calls.is_empty() {
            assistant_msg.tool_calls = Some(
                tool_calls
                    .iter()
                    .map(|tc| llm::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    })
                    .collect(),
            );
        }
        messages.push(assistant_msg);

        match response.finish_reason {
            FinishReason::ToolCalls => {
                Ok((StepDecision::AwaitTool, tool_calls))
            }
            FinishReason::Stop => {
                Ok((StepDecision::CompleteTask(content), Vec::new()))
            }
            FinishReason::Length => Ok((
                StepDecision::FailTask("Maximum context length reached".to_string()),
                Vec::new(),
            )),
            FinishReason::ContentFilter => Ok((
                StepDecision::FailTask("Content was filtered".to_string()),
                Vec::new(),
            )),
            _ => Ok((StepDecision::Continue, Vec::new())),
        }
    }

    async fn execute_tool(
        &self,
        tool_call: &ToolCall,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<ToolResult, AgentLoopError> {
        let _ = event_sink
            .send(StreamEvent::ToolCall {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            })
            .await;

        let result = self
            .tools
            .execute_with_context(
                &tool_call.name,
                tool_call.arguments.clone(),
                self.execution_context.clone(),
            )
            .await
            .unwrap_or_else(|e| ToolResult {
                success: false,
                content: String::new(),
                error: Some(e.to_string()),
            });

        // Size guard: truncate large outputs to prevent context window bloat.
        const MAX_INLINE_BYTES: usize = 8 * 1024;
        let result = if result.content.len() > MAX_INLINE_BYTES {
            let total = result.content.len();
            let preview: String = result.content.chars().take(2048).collect();
            ToolResult {
                content: format!(
                    "{}...\n[truncated: {} bytes total, {} bytes shown]",
                    preview,
                    total,
                    MAX_INLINE_BYTES.min(preview.len())
                ),
                ..result
            }
        } else {
            result
        };

        let _ = event_sink
            .send(StreamEvent::ToolResult {
                name: tool_call.name.clone(),
                success: result.success,
                content: result.content.clone(),
                error: result.error.clone(),
            })
            .await;

        Ok(result)
    }

    /// Classify a task using structured output (JSON mode).
    ///
    /// Uses `response_format = json_object` for guaranteed valid JSON
    /// instead of heuristic text parsing.
    pub async fn triage(&self, task: &str) -> Result<TriageResult, AgentLoopError> {
        let system = "\
You are a task classifier. Analyze the given task and classify it.

Output a JSON object with these fields:
- complexity: \"Simple\" | \"Medium\" | \"Complex\"
- processing_path: \"SingleRoute\" | \"GuidedDelegate\" | \"StructuredOrchestration\"
- selected_mode: \"Route\" | \"Tasks\"
- lead_member_ref: null or a member reference string
- rationale: a brief explanation of your classification

Complexity guidelines:
- Simple: Single step, one team member, straightforward
- Medium: Multi-step, 2-3 team members, some coordination
- Complex: Multi-team, many dependencies, significant coordination

Processing path guidelines:
- SingleRoute: One team member handles everything
- GuidedDelegate: Supervisor delegates but coordinates closely
- StructuredOrchestration: Multiple teams with structured handoffs

Mode guidelines:
- Route: Tasks routed to appropriate members
- Tasks: Full task decomposition and management";

        let request = ChatRequest::new(
            self.llm.model().to_string(),
            vec![
                Message::system(system),
                Message::user(task.to_string()),
            ],
        )
        .with_response_format(ResponseFormat::JsonObject);

        let response = self
            .llm
            .chat(request)
            .await
            .map_err(|e| AgentLoopError::LlmError(format!("triage failed: {}", e)))?;

        let content = response.message.content.trim();
        let parsed: Value = serde_json::from_str(content).map_err(|e| {
            AgentLoopError::LlmError(format!(
                "failed to parse triage JSON: {} — input: {}",
                e, content
            ))
        })?;

        let complexity = match parsed.get("complexity").and_then(|v| v.as_str()) {
            Some("Simple") => TaskComplexity::Simple,
            Some("Medium") => TaskComplexity::Medium,
            Some("Complex") => TaskComplexity::Complex,
            _ => TaskComplexity::Medium,
        };

        let processing_path = match parsed.get("processing_path").and_then(|v| v.as_str()) {
            Some("SingleRoute") => ProcessingPath::SingleRoute,
            Some("GuidedDelegate") => ProcessingPath::GuidedDelegate,
            Some("StructuredOrchestration") => ProcessingPath::StructuredOrchestration,
            _ => ProcessingPath::GuidedDelegate,
        };

        let selected_mode = match parsed.get("selected_mode").and_then(|v| v.as_str()) {
            Some("Route") => TeamMode::Route,
            Some("Tasks") => TeamMode::Tasks,
            _ => TeamMode::Route,
        };

        let lead_member_ref = parsed
            .get("lead_member_ref")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .map(String::from);

        let rationale = parsed
            .get("rationale")
            .and_then(|v| v.as_str())
            .unwrap_or("No rationale provided")
            .to_string();

        Ok(TriageResult {
            complexity,
            processing_path,
            selected_mode,
            lead_member_ref,
            rationale,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentLoopError {
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("tool execution error: {0}")]
    ToolError(String),
}

impl From<anyhow::Error> for AgentLoopError {
    fn from(e: anyhow::Error) -> Self {
        AgentLoopError::ToolError(e.to_string())
    }
}
