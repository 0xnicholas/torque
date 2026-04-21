use crate::models::{MemoryEntry, MemoryLayer, Message as ModelMessage, MessageRole};
use chrono::{DateTime, Utc};
use llm::Message as LlmMessage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_WINDOW_SIZE: usize = 10;
pub const DEFAULT_MAX_TOKENS: usize = 8192;
pub const DEFAULT_MEMORY_RECALL_LIMIT: i64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub covers_range: (usize, usize),
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum CompressionStrategy {
    KeepLastN(usize),
    SummarizeOlder { summarize_count: usize },
    ExtractiveCompression,
}

impl Default for CompressionStrategy {
    fn default() -> Self {
        CompressionStrategy::KeepLastN(DEFAULT_WINDOW_SIZE)
    }
}

pub struct ContextWindow {
    pub messages: Vec<ModelMessage>,
    pub window_size: usize,
}

impl ContextWindow {
    pub fn new(messages: Vec<ModelMessage>, window_size: usize) -> Self {
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

#[async_trait::async_trait]
pub trait Summarizer: Send + Sync {
    async fn summarize(&self, messages: &[ModelMessage]) -> anyhow::Result<String>;
}

pub struct ContextManager {
    window_size: usize,
    max_tokens: usize,
    warning_threshold: f64,
    compression_strategy: CompressionStrategy,
    summarizer: Option<Box<dyn Summarizer>>,
    full_history: Vec<ModelMessage>,
    compressed_context: Vec<ModelMessage>,
    summary_chain: Vec<Summary>,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
            max_tokens: DEFAULT_MAX_TOKENS,
            warning_threshold: 0.8,
            compression_strategy: CompressionStrategy::default(),
            summarizer: None,
            full_history: Vec::new(),
            compressed_context: Vec::new(),
            summary_chain: Vec::new(),
        }
    }

    pub fn with_window_size(mut self, window_size: usize) -> Self {
        self.window_size = window_size;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_compression_strategy(mut self, strategy: CompressionStrategy) -> Self {
        self.compression_strategy = strategy;
        self
    }

    pub fn with_summarizer(mut self, summarizer: Box<dyn Summarizer>) -> Self {
        self.summarizer = Some(summarizer);
        self
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    pub fn compression_strategy(&self) -> &CompressionStrategy {
        &self.compression_strategy
    }

    pub fn summary_chain(&self) -> &[Summary] {
        &self.summary_chain
    }

    pub fn build_context(&self, history: Vec<ModelMessage>) -> ContextWindow {
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

    pub fn add_message(&mut self, msg: ModelMessage) {
        self.full_history.push(msg.clone());
        self.compressed_context.push(msg);

        let token_count = self.estimate_tokens();
        if token_count as f64 > self.max_tokens as f64 * self.warning_threshold {
            self.compress_sync();
        }
    }

    fn compress_sync(&mut self) {
        match &self.compression_strategy {
            CompressionStrategy::KeepLastN(n) => {
                let to_keep = self.full_history.len().saturating_sub(*n);
                self.compressed_context = self.full_history[to_keep..].to_vec();
            }
            CompressionStrategy::SummarizeOlder { .. } => {
                // Async summarization requires explicit call to compress_async
                // Fall back to keeping all messages if no summarizer configured
                if self.summarizer.is_none() {
                    self.compressed_context = self.full_history.clone();
                }
            }
            CompressionStrategy::ExtractiveCompression => {
                self.compressed_context = self
                    .full_history
                    .iter()
                    .filter(|m| m.role == MessageRole::Tool || m.content.len() > 100)
                    .cloned()
                    .collect();
            }
        }
    }

    pub fn get_compressed_context(&self) -> &[ModelMessage] {
        &self.compressed_context
    }

    pub fn get_full_history(&self) -> &[ModelMessage] {
        &self.full_history
    }

    pub fn get_summary_chain(&self) -> &[Summary] {
        &self.summary_chain
    }

    pub fn current_messages(&self) -> Vec<LlmMessage> {
        self.compressed_context
            .iter()
            .map(|m| match m.role {
                MessageRole::User => LlmMessage::user(&m.content),
                MessageRole::Assistant => LlmMessage::assistant(&m.content),
                MessageRole::System => LlmMessage::system(&m.content),
                MessageRole::Tool => LlmMessage::user(&m.content),
            })
            .collect()
    }

    pub fn reset(&mut self) {
        self.full_history.clear();
        self.compressed_context.clear();
        self.summary_chain.clear();
    }

    fn estimate_tokens(&self) -> usize {
        self.full_history.iter().map(|m| m.content.len() / 4).sum()
    }

    pub async fn compress_async(&mut self) -> anyhow::Result<()> {
        if let CompressionStrategy::SummarizeOlder { summarize_count } = &self.compression_strategy
        {
            if let Some(ref summarizer) = self.summarizer {
                if *summarize_count >= self.full_history.len() {
                    self.compressed_context = self.full_history.clone();
                    return Ok(());
                }
                let to_summarize = &self.full_history[0..*summarize_count];
                match summarizer.summarize(to_summarize).await {
                    Ok(summary) => {
                        self.compressed_context = vec![ModelMessage {
                            id: Uuid::new_v4(),
                            session_id: Uuid::nil(),
                            role: MessageRole::System,
                            content: format!("Previous conversation summary: {}", summary),
                            tool_calls: None,
                            artifacts: None,
                            created_at: chrono::Utc::now(),
                        }];
                        self.compressed_context
                            .extend(self.full_history[*summarize_count..].to_vec());
                        self.summary_chain.push(Summary {
                            covers_range: (0, *summarize_count),
                            content: summary,
                            created_at: chrono::Utc::now(),
                        });
                    }
                    Err(e) => anyhow::bail!("Summarization failed: {}", e),
                }
            }
        }
        Ok(())
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
            MemoryLayer::L0 => "l0",
            MemoryLayer::L1 => "l1",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_message(role: MessageRole, content: &str) -> ModelMessage {
        ModelMessage {
            id: Uuid::new_v4(),
            session_id: Uuid::nil(),
            role,
            content: content.to_string(),
            tool_calls: None,
            artifacts: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_window_size_default() {
        let cm = ContextManager::new();
        assert_eq!(cm.window_size(), DEFAULT_WINDOW_SIZE);
        assert_eq!(cm.max_tokens(), DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn test_build_context_respects_window_size() {
        let cm = ContextManager::new().with_window_size(3);
        let history: Vec<ModelMessage> = (0..10)
            .map(|i| create_test_message(MessageRole::User, &format!("msg{}", i)))
            .collect();

        let window = cm.build_context(history);
        assert_eq!(window.messages.len(), 3);
        assert_eq!(window.messages[0].content, "msg7");
        assert_eq!(window.messages[2].content, "msg9");
    }

    #[test]
    fn test_keep_last_n_compression() {
        let mut cm = ContextManager::new()
            .with_window_size(5)
            .with_max_tokens(50)
            .with_compression_strategy(CompressionStrategy::KeepLastN(3));

        for i in 0..50 {
            cm.add_message(create_test_message(
                MessageRole::User,
                &format!("message-number-{}", i),
            ));
        }

        assert!(cm.get_full_history().len() >= 50);
        assert!(cm.get_compressed_context().len() <= 3);
    }

    #[test]
    fn test_memory_slice_message() {
        let cm = ContextManager::new();
        let entries = vec![MemoryEntry {
            id: Uuid::new_v4(),
            project_scope: "test".to_string(),
            layer: MemoryLayer::L0,
            content: "test content".to_string(),
            source_candidate_id: None,
            source_type: None,
            source_ref: None,
            proposer: None,
            status: crate::models::MemoryEntryStatus::Active,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            invalidated_at: None,
        }];

        let msg = cm.build_memory_slice_message(&entries);
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert!(msg.content.contains("Project memory"));
        assert!(msg.content.contains("test content"));
    }

    #[test]
    fn test_current_messages_converts_roles() {
        let mut cm = ContextManager::new().with_window_size(10);

        cm.add_message(create_test_message(MessageRole::System, "system prompt"));
        cm.add_message(create_test_message(MessageRole::User, "user message"));
        cm.add_message(create_test_message(
            MessageRole::Assistant,
            "assistant response",
        ));
        cm.add_message(create_test_message(MessageRole::Tool, "tool result"));

        let llm_messages = cm.current_messages();
        assert_eq!(llm_messages.len(), 4);
        assert_eq!(llm_messages[0].role, "system");
        assert_eq!(llm_messages[1].role, "user");
        assert_eq!(llm_messages[2].role, "assistant");
        assert_eq!(llm_messages[3].role, "user");
    }
}
