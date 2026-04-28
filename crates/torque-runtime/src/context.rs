use llm::Message as LlmMessage;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Default)]
pub struct ContextCompactionService {
    policy: ContextCompactionPolicy,
}

impl ContextCompactionService {
    pub fn new(policy: ContextCompactionPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &ContextCompactionPolicy {
        &self.policy
    }

    pub fn compact(&self, messages: &[LlmMessage]) -> Option<CompactSummary> {
        if !self.policy.should_compact(messages) {
            return None;
        }

        let preserve = self.policy.preserve_recent_messages.min(messages.len());
        let split_index = messages.len().saturating_sub(preserve);
        let older = &messages[..split_index];
        let preserved_tail = messages[split_index..].to_vec();

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
            compact_summary: format!(
                "Compacted {} earlier messages into a derived execution summary.",
                older.len()
            ),
            key_facts,
            preserved_tail,
        })
    }
}

fn estimate_tokens(messages: &[LlmMessage]) -> usize {
    messages.iter().map(|message| message.content.len() / 4).sum()
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated = text.chars().take(max_chars).collect::<String>();
        format!("{truncated}...")
    }
}
