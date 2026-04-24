use crate::agent::stream::StreamEvent;
use crate::infra::llm::{Chunk, LlmClient, LlmMessage, ToolCall};
use crate::infra::tool_registry::ToolRegistry;
use crate::models::v1::team::{ProcessingPath, TaskComplexity, TeamMode, TriageResult};
use crate::tools::ToolResult;
use llm::{ChatRequest, FinishReason, Message};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_kernel::StepDecision;

const MAX_STEPS: usize = 50;
const MAX_CONSECUTIVE_TOOL_FAILURES: usize = 3;

pub enum ReActStep {
    Think {
        thought: String,
        action: Option<ReActAction>,
    },
    Complete {
        summary: String,
    },
    Fail {
        reason: String,
    },
}

pub enum ReActAction {
    ToolCall { name: String, arguments: Value },
    Delegated { description: String },
    ApprovalRequired { description: String },
}

pub struct ReActHarness {
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolRegistry>,
    step_history: Vec<ReActStep>,
}

impl ReActHarness {
    pub fn new(llm: Arc<dyn LlmClient>, tools: Arc<ToolRegistry>) -> Self {
        Self {
            llm,
            tools,
            step_history: Vec::new(),
        }
    }

    pub async fn run(
        &mut self,
        task: &str,
        system_prompt: Option<&str>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<StepDecision, ReActHarnessError> {
        let mut messages = self.build_initial_messages(task, system_prompt);
        let mut consecutive_failures = 0;

        for step_num in 0..MAX_STEPS {
            let decision = self
                .execute_step(&mut messages, step_num, event_sink.clone())
                .await?;

            match &decision {
                StepDecision::Continue => {
                    continue;
                }
                StepDecision::AwaitTool => {
                    let last_msg = messages.last();
                    if let Some(msg) = last_msg {
                        if msg.role == "user" {
                            if let Ok(action) = self.parse_tool_call_from_message(&msg.content) {
                                let result = self.execute_tool(&action, event_sink.clone()).await?;
                                self.step_history.push(ReActStep::Think {
                                    thought: format!("Executed tool: {}", action.name),
                                    action: Some(ReActAction::ToolCall {
                                        name: action.name.clone(),
                                        arguments: action.arguments.clone(),
                                    }),
                                });
                                consecutive_failures = if result.success {
                                    0
                                } else {
                                    consecutive_failures + 1
                                };
                                messages.push(LlmMessage::user(format!(
                                    "Tool '{}' result: {}",
                                    action.name, result.content
                                )));
                                if consecutive_failures >= MAX_CONSECUTIVE_TOOL_FAILURES {
                                    return Ok(StepDecision::FailTask(format!(
                                        "Tool execution failed {} times consecutively",
                                        consecutive_failures
                                    )));
                                }
                                continue;
                            }
                        }
                    }
                    messages.push(LlmMessage::user(
                        "No tool call detected. Continue reasoning or complete the task."
                            .to_string(),
                    ));
                }
                StepDecision::CompleteTask(summary) => {
                    self.step_history.push(ReActStep::Complete {
                        summary: summary.clone(),
                    });
                    return Ok(StepDecision::CompleteTask(summary.clone()));
                }
                StepDecision::FailTask(reason) => {
                    self.step_history.push(ReActStep::Fail {
                        reason: reason.clone(),
                    });
                    return Ok(StepDecision::FailTask(reason.clone()));
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

        Ok(StepDecision::FailTask(format!(
            "Maximum step limit ({}) reached",
            MAX_STEPS
        )))
    }

    fn build_initial_messages(&self, task: &str, system_prompt: Option<&str>) -> Vec<LlmMessage> {
        let mut messages = Vec::new();

        let react_system = system_prompt.unwrap_or(
            r#"You are a ReAct agent (Reasoning + Acting).

For each step, you must output your reasoning and then take ONE action.

Output format for each step:
THINK: <your reasoning about what to do next>
ACT: <one of the following>
  - tool:{"name": "tool_name", "arguments": {"arg1": "value1"}}
  - delegate: <description of what to delegate>
  - approve: <description of what needs approval>
  - complete: <final answer or summary>

You have access to tools. When using a tool, output the full tool call.
If the task is complete, output complete with your summary.
If you need approval for something, output approve with description.
"#,
        );

        messages.push(LlmMessage::system(react_system));
        messages.push(LlmMessage::user(format!(
            "Task: {}\n\nProvide your first step (think + act).",
            task
        )));

        messages
    }

    async fn execute_step(
        &self,
        messages: &mut Vec<LlmMessage>,
        step_num: usize,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<StepDecision, ReActHarnessError> {
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
            .map_err(|e| ReActHarnessError::LlmError(e.to_string()))?;

        let content = Arc::try_unwrap(content_buffer)
            .map(|m| m.into_inner().unwrap_or_default())
            .unwrap_or_default();
        let tool_calls = Arc::try_unwrap(tool_calls_buffer)
            .map(|m| m.into_inner().unwrap_or_default())
            .unwrap_or_default();

        messages.push(LlmMessage::assistant(&content));

        match response.finish_reason {
            FinishReason::ToolCalls => {
                if let Some(tool_call) = tool_calls.first() {
                    return Ok(StepDecision::AwaitTool);
                }
                Ok(StepDecision::Continue)
            }
            FinishReason::Stop => {
                if let Ok(action) = self.parse_action_from_response(&content) {
                    match action {
                        ReActAction::ToolCall { name, arguments } => {
                            messages.push(LlmMessage::user(format!(
                                "Tool call: {} with {:?}",
                                name, arguments
                            )));
                            Ok(StepDecision::AwaitTool)
                        }
                        ReActAction::Delegated { .. } => Ok(StepDecision::AwaitDelegation(
                            torque_kernel::DelegationRequestId::new(),
                        )),
                        ReActAction::ApprovalRequired { .. } => Ok(StepDecision::AwaitApproval(
                            torque_kernel::ApprovalRequestId::new(),
                        )),
                    }
                } else {
                    Ok(StepDecision::CompleteTask(content.clone()))
                }
            }
            FinishReason::Length => Ok(StepDecision::FailTask(
                "Maximum context length reached".to_string(),
            )),
            FinishReason::ContentFilter => {
                Ok(StepDecision::FailTask("Content was filtered".to_string()))
            }
            _ => Ok(StepDecision::Continue),
        }
    }

    fn parse_action_from_response(&self, content: &str) -> Result<ReActAction, ()> {
        let lines: Vec<&str> = content.lines().collect();
        let mut act_line = None;

        for line in lines.iter().rev() {
            let trimmed = line.trim();
            if trimmed.starts_with("ACT:") {
                act_line = Some(trimmed[4..].trim());
                break;
            }
        }

        if let Some(act_str) = act_line {
            if act_str.starts_with("complete:") {
                let summary = act_str[9..].trim().to_string();
                return Ok(ReActAction::ToolCall {
                    name: "__complete__".to_string(),
                    arguments: serde_json::json!({ "summary": summary }),
                });
            }
            if let Ok(parsed) = serde_json::from_str::<Value>(act_str) {
                if let (Some(name), Some(args)) = (parsed.get("name"), parsed.get("arguments")) {
                    return Ok(ReActAction::ToolCall {
                        name: name.as_str().unwrap_or("").to_string(),
                        arguments: args.clone(),
                    });
                }
            }
            if act_str.starts_with("delegate:") {
                return Ok(ReActAction::Delegated {
                    description: act_str[9..].trim().to_string(),
                });
            }
            if act_str.starts_with("approve:") {
                return Ok(ReActAction::ApprovalRequired {
                    description: act_str[8..].trim().to_string(),
                });
            }
        }

        Err(())
    }

    fn parse_tool_call_from_message(&self, content: &str) -> Result<ToolCall, ()> {
        if let Ok(parsed) = serde_json::from_str::<Value>(content) {
            if let (Some(name), Some(args)) = (parsed.get("name"), parsed.get("arguments")) {
                return Ok(ToolCall {
                    id: format!("call-{}", uuid::Uuid::new_v4()),
                    name: name.as_str().unwrap_or("").to_string(),
                    arguments: args.clone(),
                });
            }
        }
        Err(())
    }

    async fn execute_tool(
        &self,
        tool_call: &ToolCall,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<ToolResult, ReActHarnessError> {
        if tool_call.name == "__complete__" {
            return Ok(ToolResult {
                success: true,
                content: "Task completed".to_string(),
                error: None,
            });
        }

        let _ = event_sink
            .send(StreamEvent::ToolCall {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            })
            .await;

        let result = self
            .tools
            .execute(&tool_call.name, tool_call.arguments.clone())
            .await
            .unwrap_or_else(|e| ToolResult {
                success: false,
                content: String::new(),
                error: Some(e.to_string()),
            });

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

    pub fn step_history(&self) -> &[ReActStep] {
        &self.step_history
    }

    pub fn clear_history(&mut self) {
        self.step_history.clear();
    }

    pub async fn triage(&self, task: &str) -> Result<TriageResult, ReActHarnessError> {
        let triage_prompt = format!(
            r#"Analyze this task and determine how to handle it.

Task: {}

Respond with ONLY a JSON object containing:
{{
  "complexity": "Simple" or "Medium" or "Complex",
  "processing_path": "SingleRoute" or "GuidedDelegate" or "StructuredOrchestration",
  "selected_mode": "Route" or "Tasks",
  "lead_member_ref": null or "member-reference-string",
  "rationale": "Brief explanation of the decision"
}}

Complexity guidelines:
- Simple: Single step, one team member, straightforward
- Medium: Multi-step, 2-3 team members, some coordination needed
- Complex: Multi-team, many dependencies, significant coordination

Processing path guidelines:
- SingleRoute: One team member handles everything
- GuidedDelegate: Supervisor delegates but coordinates closely
- StructuredOrchestration: Multiple teams with structured handoffs

Mode guidelines:
- Route: Tasks routed to appropriate members
- Tasks: Full task decomposition and management"#,
            task
        );

        let request = ChatRequest::new(
            self.llm.model().to_string(),
            vec![Message::user(triage_prompt)],
        );

        let response = self.llm.chat(request).await.map_err(|e| {
            ReActHarnessError::LlmError(format!("triage failed: {}", e))
        })?;

        let content = response.message.content.trim();

        let json_start = content.find('{');
        let json_end = content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &content[start..=end];
            let parsed: Value = serde_json::from_str(json_str).map_err(|e| {
                ReActHarnessError::LlmError(format!("failed to parse triage JSON: {} - input: {}", e, json_str))
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

            return Ok(TriageResult {
                complexity,
                processing_path,
                selected_mode,
                lead_member_ref,
                rationale,
            });
        }

        Err(ReActHarnessError::LlmError(
            format!("no JSON found in triage response: {}", content)
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReActHarnessError {
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("tool execution error: {0}")]
    ToolError(String),
    #[error("max steps exceeded")]
    MaxStepsExceeded,
}

impl From<anyhow::Error> for ReActHarnessError {
    fn from(e: anyhow::Error) -> Self {
        ReActHarnessError::ToolError(e.to_string())
    }
}
