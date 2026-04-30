use async_trait::async_trait;
use llm::Message as LlmMessage;
use serde::{Deserialize, Serialize};

/// Optional LLM-powered summarizer for context compaction.
/// When set on `ContextCompactionService`, the LLM path is tried first;
/// if it returns `None` (or is absent), the heuristic truncation path
/// is used as a fallback.
#[async_trait]
pub trait LlmSummarizer: Send + Sync {
    /// Attempt to produce a concise summary of the given messages.
    /// Return `None` if the summarizer declines (e.g. no model available).
    async fn summarize(&self, messages: &[LlmMessage]) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompactionPolicy {
    pub message_threshold: usize,
    pub estimated_token_threshold: usize,
    pub preserve_recent_messages: usize,
    pub preview_chars: usize,
}

impl Default for ContextCompactionPolicy {
    fn default() -> Self {
        Self {
            message_threshold: 12,
            estimated_token_threshold: 4_000,
            preserve_recent_messages: 4,
            preview_chars: 160,
        }
    }
}

impl ContextCompactionPolicy {
    pub fn should_compact(&self, messages: &[LlmMessage]) -> bool {
        messages.len() > self.message_threshold
            || estimate_tokens(messages) > self.estimated_token_threshold
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactSummary {
    pub compact_summary: String,
    pub key_facts: Vec<String>,
    pub preserved_tail: Vec<LlmMessage>,
}

impl CompactSummary {
    pub fn to_runtime_message(&self) -> crate::message::RuntimeMessage {
        crate::message::RuntimeMessage::user(format!(
            "[Context Compaction] {} Key facts from earlier messages:\n  {}",
            self.compact_summary,
            self.key_facts.join("\n  ")
        ))
    }

    pub fn is_compaction_message(content: &str) -> bool {
        content.starts_with("[Context Compaction]")
    }
}

#[derive(Clone)]
pub struct ContextCompactionService {
    policy: ContextCompactionPolicy,
    llm_summarizer: Option<std::sync::Arc<dyn LlmSummarizer>>,
}

impl Default for ContextCompactionService {
    fn default() -> Self {
        Self {
            policy: ContextCompactionPolicy::default(),
            llm_summarizer: None,
        }
    }
}

impl ContextCompactionService {
    pub fn new(policy: ContextCompactionPolicy) -> Self {
        Self {
            policy,
            llm_summarizer: None,
        }
    }

    /// Attach an LLM summarizer. When present, compaction will
    /// attempt LLM-driven summarization before falling back to
    /// heuristic truncation.
    pub fn with_llm_summarizer(mut self, summarizer: std::sync::Arc<dyn LlmSummarizer>) -> Self {
        self.llm_summarizer = Some(summarizer);
        self
    }

    pub fn policy(&self) -> &ContextCompactionPolicy {
        &self.policy
    }

    /// Run compaction on the given messages. If an LLM summarizer is
    /// configured, it is tried first; on failure or absence, falls back
    /// to character-level truncation.
    pub async fn compact(&self, messages: &[LlmMessage]) -> Option<CompactSummary> {
        if !self.policy.should_compact(messages) {
            return None;
        }

        if messages.last().map_or(false, |m| {
            CompactSummary::is_compaction_message(&m.content)
        }) {
            return None;
        }

        let preserve = self.policy.preserve_recent_messages.min(messages.len());
        let split_index = messages.len().saturating_sub(preserve);
        let older = &messages[..split_index];
        let preserved_tail = messages[split_index..].to_vec();

        // Try LLM-driven summarization first.
        let compact_summary = if let Some(ref summarizer) = self.llm_summarizer {
            match summarizer.summarize(older).await {
                Some(summary) => summary,
                None => {
                    tracing::warn!("LLM summarizer declined; falling back to heuristic");
                    Self::heuristic_summary(older.len())
                }
            }
        } else {
            Self::heuristic_summary(older.len())
        };

        let key_facts = older
            .iter()
            .filter_map(|message| {
                let text = message.content.trim();
                if text.is_empty() {
                    None
                } else {
                    Some(truncate(text, self.policy.preview_chars))
                }
            })
            .collect::<Vec<_>>();

        Some(CompactSummary {
            compact_summary,
            key_facts,
            preserved_tail,
        })
    }

    /// Heuristic summary: a simple template when no LLM is available.
    fn heuristic_summary(older_len: usize) -> String {
        format!(
            "Compacted {older_len} earlier messages into a derived execution summary."
        )
    }
}

fn estimate_tokens(messages: &[LlmMessage]) -> usize {
    messages.iter().map(|message| message.content.len() / 4).sum()
}

pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated = text.chars().take(max_chars).collect::<String>();
        format!("{truncated}...")
    }
}
