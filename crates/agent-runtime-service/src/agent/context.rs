use crate::models::{MemoryEntry, Message, MessageRole};
use llm::Message as LlmMessage;

pub const DEFAULT_WINDOW_SIZE: usize = 10;
pub const DEFAULT_MEMORY_RECALL_LIMIT: i64 = 5;

pub struct ContextWindow {
    pub messages: Vec<Message>,
    pub window_size: usize,
}

impl ContextWindow {
    pub fn new(messages: Vec<Message>, window_size: usize) -> Self {
        Self {
            messages,
            window_size,
        }
    }

    pub fn to_llm_messages(&self) -> Vec<LlmMessage> {
        self.messages
            .iter()
            .map(|m| match m.role {
                MessageRole::User => LlmMessage::user(&m.content),
                MessageRole::Assistant => LlmMessage::assistant(&m.content),
                MessageRole::System => LlmMessage::system(&m.content),
                MessageRole::Tool => LlmMessage::user(&m.content),
            })
            .collect()
    }
}

pub struct ContextManager {
    window_size: usize,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
        }
    }

    pub fn build_context(&self, history: Vec<Message>) -> ContextWindow {
        let start = history.len().saturating_sub(self.window_size);
        let window_messages = history[start..].to_vec();

        ContextWindow::new(window_messages, self.window_size)
    }

    pub fn build_memory_slice_message(&self, entries: &[MemoryEntry]) -> Option<LlmMessage> {
        if entries.is_empty() {
            return None;
        }

        let mut lines = Vec::with_capacity(entries.len() + 2);
        lines.push("Project memory (durable facts, may be stale):".to_string());
        lines.push("Use these only when relevant to the user request.".to_string());

        for entry in entries {
            lines.push(format!("- [{}] {}", entry.layer_string(), entry.content));
        }

        Some(LlmMessage::system(lines.join("\n")))
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

trait MemoryLayerString {
    fn layer_string(&self) -> &'static str;
}

impl MemoryLayerString for MemoryEntry {
    fn layer_string(&self) -> &'static str {
        match &self.layer {
            crate::models::MemoryLayer::L0 => "l0",
            crate::models::MemoryLayer::L1 => "l1",
        }
    }
}
