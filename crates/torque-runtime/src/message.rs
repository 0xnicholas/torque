use crate::context::CompactSummary;
use crate::tools::{RuntimeToolCall, RuntimeToolResult};
use llm::Message as LlmMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeMessage {
    pub role: RuntimeMessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<RuntimeToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl RuntimeMessage {
    pub fn new(role: RuntimeMessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::Assistant, content)
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self::new(RuntimeMessageRole::Tool, content)
    }
}

impl From<LlmMessage> for RuntimeMessage {
    fn from(value: LlmMessage) -> Self {
        let role = match value.role.as_str() {
            "system" => RuntimeMessageRole::System,
            "assistant" => RuntimeMessageRole::Assistant,
            "tool" => RuntimeMessageRole::Tool,
            _ => RuntimeMessageRole::User,
        };
        let tool_calls = value.tool_calls.map(|calls| {
            calls.into_iter().map(|tc| RuntimeToolCall {
                id: tc.id,
                name: tc.name,
                arguments: tc.arguments,
            }).collect()
        });
        Self {
            role,
            content: value.content,
            tool_calls,
            tool_call_id: value.tool_call_id,
            name: value.name,
        }
    }
}

impl From<RuntimeMessage> for LlmMessage {
    fn from(value: RuntimeMessage) -> Self {
        let role = match value.role {
            RuntimeMessageRole::System => "system",
            RuntimeMessageRole::User => "user",
            RuntimeMessageRole::Assistant => "assistant",
            RuntimeMessageRole::Tool => "tool",
        };
        let tool_calls = value.tool_calls.map(|calls| {
            calls.into_iter().map(|tc| llm::ToolCall {
                id: tc.id,
                name: tc.name,
                arguments: tc.arguments,
            }).collect()
        });
        LlmMessage {
            role: role.to_string(),
            content: value.content,
            tool_calls,
            tool_call_id: value.tool_call_id,
            name: value.name,
        }
    }
}

impl From<crate::checkpoint::Message> for RuntimeMessage {
    fn from(m: crate::checkpoint::Message) -> Self {
        let role = match m.role.as_str() {
            "system" => RuntimeMessageRole::System,
            "assistant" => RuntimeMessageRole::Assistant,
            "tool" => RuntimeMessageRole::Tool,
            _ => RuntimeMessageRole::User,
        };
        Self {
            role,
            content: m.content,
            tool_calls: m.tool_calls,
            tool_call_id: m.tool_call_id,
            name: m.name,
        }
    }
}

// ── StructuredMessage ───────────────────────────────────────────

/// The three context planes defined in Context Planes Design.
/// Each message variant maps to exactly one plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPlane {
    /// External references: upstream input, task packets, external context.
    ExternalContextRef,
    /// Execution results: assistant responses, tool outputs, generated content.
    Artifact,
    /// Semantic retention: system prompts, compaction markers, policies.
    Memory,
}

/// Strongly-typed message variant replacing flat `RuntimeMessage` for
/// all queue operations.  Every variant carries the exact shape needed
/// by the Layer-5 context-plane contracts and by the three delivery
/// modes (`steer`, `followUp`, `nextTurn`).
///
/// `RuntimeMessage` remains as a serialization/checkpoint bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StructuredMessage {
    /// System prompt or policy directive.
    System {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        policy_ref: Option<String>,
    },

    /// Human / upstream input.
    UserInput {
        content: String,
    },

    /// Assistant turn, possibly with requested tool calls.
    AssistantResponse {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<RuntimeToolCall>>,
    },

    /// Result of an executed tool call, keyed to the originating call.
    ToolResult {
        call_id: String,
        tool_name: String,
        result: RuntimeToolResult,
    },

    /// Compaction marker produced by `ContextCompactionService`.
    CompactionMarker {
        summary: CompactSummary,
    },

    /// Narrow mission-oriented packet (see Context State Model §10).
    TaskPacket {
        goal: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        instructions: Option<String>,
        shared_state_slice: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        constraints: Option<serde_json::Value>,
    },

    /// Supervisor-originated injection during tool turn (steer mode).
    SteerInjection {
        source: String,
        payload: Box<StructuredMessage>,
    },
}

impl StructuredMessage {
    // ── Convenience constructors ────────────────────────────

    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
            policy_ref: None,
        }
    }

    pub fn system_with_policy(content: impl Into<String>, policy_ref: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
            policy_ref: Some(policy_ref.into()),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::UserInput {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::AssistantResponse {
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn assistant_with_tools(
        content: impl Into<String>,
        tool_calls: Vec<RuntimeToolCall>,
    ) -> Self {
        Self::AssistantResponse {
            content: content.into(),
            tool_calls: Some(tool_calls),
        }
    }

    pub fn tool_result(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        result: RuntimeToolResult,
    ) -> Self {
        Self::ToolResult {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            result,
        }
    }

    pub fn task_packet(
        goal: impl Into<String>,
        shared_state_slice: serde_json::Value,
    ) -> Self {
        Self::TaskPacket {
            goal: goal.into(),
            instructions: None,
            shared_state_slice,
            constraints: None,
        }
    }

    pub fn steer(source: impl Into<String>, payload: StructuredMessage) -> Self {
        Self::SteerInjection {
            source: source.into(),
            payload: Box::new(payload),
        }
    }

    // ── Role extraction ─────────────────────────────────────

    pub fn role_name(&self) -> &'static str {
        match self {
            Self::System { .. } => "system",
            Self::UserInput { .. } => "user",
            Self::AssistantResponse { .. } => "assistant",
            Self::ToolResult { .. } => "tool",
            Self::CompactionMarker { .. } => "user",
            Self::TaskPacket { .. } => "user",
            Self::SteerInjection { .. } => "user",
        }
    }

    /// Rough content-length estimate for token budgeting.
    pub fn content_len(&self) -> usize {
        match self {
            Self::System { content, .. } => content.len(),
            Self::UserInput { content } => content.len(),
            Self::AssistantResponse { content, tool_calls } => {
                let tc_len = tool_calls
                    .as_ref()
                    .map(|tc| tc.iter().map(|t| t.arguments.to_string().len()).sum::<usize>())
                    .unwrap_or(0);
                content.len() + tc_len
            }
            Self::ToolResult { result, .. } => result.content.len(),
            Self::CompactionMarker { summary } => summary.compact_summary.len(),
            Self::TaskPacket { goal, instructions, shared_state_slice, constraints } => {
                goal.len()
                    + instructions.as_ref().map(|s| s.len()).unwrap_or(0)
                    + shared_state_slice.to_string().len()
                    + constraints.as_ref().map(|v| v.to_string().len()).unwrap_or(0)
            }
            Self::SteerInjection { payload, .. } => payload.content_len(),
        }
    }

    // ── Context plane classification ─────────────────────────

    /// Returns which context plane this message belongs to.
    /// SteerInjections delegate to the inner payload's plane.
    pub fn plane(&self) -> ContextPlane {
        match self {
            Self::System { .. } => ContextPlane::Memory,
            Self::UserInput { .. } => ContextPlane::ExternalContextRef,
            Self::AssistantResponse { .. } => ContextPlane::Artifact,
            Self::ToolResult { .. } => ContextPlane::Artifact,
            Self::CompactionMarker { .. } => ContextPlane::Memory,
            Self::TaskPacket { .. } => ContextPlane::ExternalContextRef,
            Self::SteerInjection { payload, .. } => payload.plane(),
        }
    }

    // ── Conversion to LLM-native messages ───────────────────

    /// Produce one or more `llm::Message`s for the LLM client.
    /// `ToolResult` and `AssistantResponse::tool_calls` require
    /// specific LLM-level shapes (separate message for tool call
    /// vs tool result).
    pub fn to_llm_messages(&self) -> Vec<LlmMessage> {
        match self {
            Self::System { content, .. } => {
                vec![LlmMessage {
                    role: "system".into(),
                    content: content.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }]
            }
            Self::UserInput { content } => {
                vec![LlmMessage {
                    role: "user".into(),
                    content: content.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }]
            }
            Self::AssistantResponse { content, tool_calls } => {
                let mut msgs = Vec::new();
                // Assistant text (may be empty if only tool calls)
                if !content.is_empty() {
                    msgs.push(LlmMessage {
                        role: "assistant".into(),
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });
                }
                // Assistant tool_calls as a separate message
                if let Some(tc) = tool_calls {
                    if !tc.is_empty() {
                        let llm_tc: Vec<llm::ToolCall> = tc
                            .iter()
                            .cloned()
                            .map(llm::ToolCall::from)
                            .collect();
                        msgs.push(LlmMessage {
                            role: "assistant".into(),
                            content: String::new(),
                            tool_calls: Some(llm_tc),
                            tool_call_id: None,
                            name: None,
                        });
                    }
                }
                msgs
            }
            Self::ToolResult { call_id, result, .. } => {
                let content = if result.success {
                    result.content.clone()
                } else {
                    format!(
                        "error: {}",
                        result.error.as_deref().unwrap_or("unknown")
                    )
                };
                vec![LlmMessage {
                    role: "tool".into(),
                    content,
                    tool_calls: None,
                    tool_call_id: Some(call_id.clone()),
                    name: None,
                }]
            }
            Self::CompactionMarker { summary } => {
                vec![summary.to_runtime_message().into()]
            }
            Self::TaskPacket { goal, instructions, shared_state_slice, constraints } => {
                let mut content = format!("Goal: {goal}");
                if let Some(inst) = instructions {
                    content.push_str(&format!("\nInstructions: {inst}"));
                }
                content.push_str(&format!(
                    "\nShared state: {}",
                    serde_json::to_string_pretty(shared_state_slice)
                        .unwrap_or_else(|_| "<serialization error>".into())
                ));
                if let Some(c) = constraints {
                    content.push_str(&format!(
                        "\nConstraints: {}",
                        serde_json::to_string_pretty(c)
                            .unwrap_or_else(|_| "<serialization error>".into())
                    ));
                }
                vec![LlmMessage {
                    role: "user".into(),
                    content,
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }]
            }
            Self::SteerInjection { payload, source } => {
                let mut msgs = payload.to_llm_messages();
                // Tag the first message with source for audit
                if let Some(first) = msgs.first_mut() {
                    first.content = format!("[steer from {source}] {}", first.content);
                }
                msgs
            }
        }
    }

    /// Convert from a legacy flat `RuntimeMessage`.
    pub fn from_runtime(m: &RuntimeMessage) -> Self {
        match m.role {
            RuntimeMessageRole::System => Self::System {
                content: m.content.clone(),
                policy_ref: m.name.clone(),
            },
            RuntimeMessageRole::User => Self::UserInput {
                content: m.content.clone(),
            },
            RuntimeMessageRole::Assistant => Self::AssistantResponse {
                content: m.content.clone(),
                tool_calls: m.tool_calls.clone(),
            },
            RuntimeMessageRole::Tool => {
                let result = if m.content.starts_with("error:") {
                    RuntimeToolResult::failure(m.content.clone())
                } else {
                    RuntimeToolResult::success(m.content.clone())
                };
                Self::ToolResult {
                    call_id: m.tool_call_id.clone().unwrap_or_default(),
                    tool_name: m.name.clone().unwrap_or_default(),
                    result,
                }
            }
        }
    }
}

impl From<StructuredMessage> for RuntimeMessage {
    fn from(sm: StructuredMessage) -> Self {
        match sm {
            StructuredMessage::System { content, policy_ref } => {
                let mut msg = RuntimeMessage::system(content);
                msg.name = policy_ref;
                msg
            }
            StructuredMessage::UserInput { content } => RuntimeMessage::user(content),
            StructuredMessage::AssistantResponse { content, tool_calls } => {
                let mut msg = RuntimeMessage::assistant(content);
                msg.tool_calls = tool_calls;
                msg
            }
            StructuredMessage::ToolResult { call_id, tool_name, result } => {
                let content = if result.success {
                    result.content
                } else {
                    format!("error: {}", result.error.unwrap_or_default())
                };
                let mut msg = RuntimeMessage::tool(content);
                msg.tool_call_id = Some(call_id);
                msg.name = Some(tool_name);
                msg
            }
            StructuredMessage::CompactionMarker { summary } => {
                summary.to_runtime_message()
            }
            StructuredMessage::TaskPacket { goal, instructions, shared_state_slice, constraints } => {
                let mut content = format!("[TaskPacket] Goal: {goal}");
                if let Some(inst) = instructions {
                    content.push_str(&format!("\nInstructions: {inst}"));
                }
                content.push_str(&format!(
                    "\nShared state: {}",
                    serde_json::to_string(&shared_state_slice).unwrap_or_default()
                ));
                if let Some(c) = constraints {
                    content.push_str(&format!(
                        "\nConstraints: {}",
                        serde_json::to_string(&c).unwrap_or_default()
                    ));
                }
                RuntimeMessage::user(content)
            }
            StructuredMessage::SteerInjection { source, payload } => {
                let mut inner: RuntimeMessage = (*payload).into();
                inner.content = format!("[steer from {source}] {}", inner.content);
                inner
            }
        }
    }
}
