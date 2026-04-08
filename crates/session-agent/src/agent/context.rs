use crate::models::{Message, MessageRole};
use llm::Message as LlmMessage;

pub const DEFAULT_WINDOW_SIZE: usize = 10;

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

    pub fn with_window_size(mut self, size: usize) -> Self {
        self.window_size = size;
        self
    }

    pub fn build_context(&self, history: Vec<Message>) -> ContextWindow {
        let start = history.len().saturating_sub(self.window_size);
        let window_messages = history[start..].to_vec();

        ContextWindow::new(window_messages, self.window_size)
    }

    pub fn add_system_prompt(&self, mut window: ContextWindow, prompt: &str) -> ContextWindow {
        let system_msg = Message {
            id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::nil(),
            role: MessageRole::System,
            content: prompt.to_string(),
            tool_calls: None,
            artifacts: None,
            created_at: chrono::Utc::now(),
        };

        window.messages.insert(0, system_msg);
        window
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}
