use async_trait::async_trait;
use llm::Message as LlmMessage;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Optional LLM-powered summarizer for context compaction.
/// When set on `ContextCompactionService`, the LLM path is tried first;
/// if it returns `None` (or is absent), the heuristic truncation path
/// is used as a fallback.
#[async_trait]
pub trait LlmSummarizer: Send + Sync {
    /// Attempt to produce a concise summary of the given messages.
    /// Return `None` if the summarizer declines (e.g. no model available).
    async fn summarize(&self, messages: &[LlmMessage]) -> Option<String>;

    /// Same as `summarize` but also accepts custom summarization
    /// instructions and a cancellation token.  When `cancel.is_cancelled()`
    /// returns `true`, the summarizer should abort and return `None`.
    ///
    /// The default implementation ignores `instructions` and `cancel`
    /// and delegates to `summarize()` for backward compatibility.
    async fn summarize_with_options(
        &self,
        messages: &[LlmMessage],
        instructions: Option<&str>,
        cancel: &CancellationToken,
    ) -> Option<String> {
        let _ = (instructions, cancel);
        self.summarize(messages).await
    }
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

    /// Run compaction with default options (no custom instructions, no cancellation).
    /// Delegates to `compact_with_options` for unified logic.
    pub async fn compact(&self, messages: &[LlmMessage]) -> Option<CompactSummary> {
        let cancel = CancellationToken::new();
        self.compact_with_options(messages, None, &cancel).await
    }

    /// Run compaction on the given messages with optional custom
    /// summarization instructions and a cancellation token.
    ///
    /// - `instructions`: passed to the LLM summarizer if configured.
    /// - `cancel`: checked before and after summarizer invocation.
    ///   When cancelled, compaction returns `None`.
    pub async fn compact_with_options(
        &self,
        messages: &[LlmMessage],
        instructions: Option<String>,
        cancel: &CancellationToken,
    ) -> Option<CompactSummary> {
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

        // Check cancellation before summarizer.
        if cancel.is_cancelled() {
            tracing::info!("Compaction cancelled before summarizer");
            return None;
        }

        // Try LLM-driven summarization first.
        let compact_summary = if let Some(ref summarizer) = self.llm_summarizer {
            match summarizer
                .summarize_with_options(older, instructions.as_deref(), cancel)
                .await
            {
                Some(summary) => summary,
                None => {
                    if cancel.is_cancelled() {
                        tracing::info!("Compaction cancelled during/after summarizer");
                        return None;
                    }
                    tracing::warn!("LLM summarizer declined; falling back to heuristic");
                    Self::heuristic_summary(older.len())
                }
            }
        } else {
            Self::heuristic_summary(older.len())
        };

        // Check cancellation after summarizer, before key facts extraction.
        if cancel.is_cancelled() {
            tracing::info!("Compaction cancelled after summarizer");
            return None;
        }

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

// ── CancellationToken ────────────────────────────────────────────

/// A lightweight cancellation token backed by `Arc<AtomicBool>`.
/// Used to signal that an in-flight operation (e.g. compaction)
/// should abort.
///
/// This is intentionally simpler than `tokio_util::CancellationToken`
/// to avoid adding a dependency.  It matches the pattern used by
/// `AbortSignal` in the extension system.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    inner: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a new, non-cancelled token.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signal cancellation.  Idempotent — subsequent calls are no-ops.
    pub fn cancel(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    /// Returns `true` if `cancel()` has been called.
    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

// ── CompactionJob ────────────────────────────────────────────────

/// Status of an in-flight compaction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionJobStatus {
    Running,
    Completed,
    Aborted,
}

/// A handle for an in-flight compaction operation.
/// Created by `SessionService::compact()` and used by
/// `SessionService::abort_compaction()` to cancel the operation.
#[derive(Debug, Clone)]
pub struct CompactionJob {
    pub id: Uuid,
    pub cancel: CancellationToken,
    pub status: CompactionJobStatus,
}

fn estimate_tokens(messages: &[LlmMessage]) -> usize {
    messages.iter().map(|message| message.content.len() / 4).sum()
}

/// A no-op cancellation token that never cancels.
/// Used internally when no cancellation is desired.
pub fn never_cancel_token() -> CancellationToken {
    CancellationToken::new()
}

pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated = text.chars().take(max_chars).collect::<String>();
        format!("{truncated}...")
    }
}
