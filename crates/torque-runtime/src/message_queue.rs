use async_trait::async_trait;
use std::collections::VecDeque;

use crate::context::{CompactSummary, ContextCompactionPolicy, CancellationToken};
use crate::message::StructuredMessage;

/// Three mutually exclusive delivery semantics for a message handed to
/// the queue.  The mode controls *when* and *how* a message becomes
/// visible to the agent's execution loop.
///
/// ┌──────────┬─────────────────────────────────────────────────┐
/// │ Mode     │ Behaviour                                        │
/// ├──────────┼─────────────────────────────────────────────────┤
/// │ Steer    │ Injected *during* the current tool-execution    │
/// │          │ phase, before the next LLM turn.  Highest        │
/// │          │ priority.  Supervisor / policy use.              │
/// ├──────────┼─────────────────────────────────────────────────┤
/// │ FollowUp │ Queued until the agent becomes `Ready`, then     │
/// │          │ triggers a chained execution cycle.              │
/// ├──────────┼─────────────────────────────────────────────────┤
/// │ NextTurn │ Passive background — merged into the initial     │
/// │          │ prompt list next time `execute_v1` is called.    │
/// │          │ Never triggers execution on its own.            │
/// └──────────┴─────────────────────────────────────────────────┘
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryMode {
    Steer,
    FollowUp,
    NextTurn,
}

/// The kernel-visible contract for a structured message queue that
/// supports three delivery modes.  Implementations are free to use
/// in-memory VecDeques, persistent storage, or hybrid approaches.
#[async_trait]
pub trait MessageQueue: Send + Sync {
    /// Enqueue a message with an explicit delivery mode.
    ///
    /// - `Steer` messages are pushed onto a LIFO channel and consumed
    ///   by `poll_steer()`.
    /// - `FollowUp` messages are appended FIFO and consumed by
    ///   `drain_followups()`.
    /// - `NextTurn` messages are stored passively and returned by
    ///   `next_turn_messages()`.
    async fn enqueue(&mut self, msg: StructuredMessage, mode: DeliveryMode);

    /// Push a message directly into the active conversation list.
    /// Used by the agent loop to accumulate assistant responses,
    /// tool results, and compaction markers as the conversation
    /// progresses.
    fn push_conversation_message(&mut self, msg: StructuredMessage);

    /// Return an immutable view of the active conversation messages.
    /// Used for checkpoint serialization and debugging.
    fn active_messages(&self) -> &[StructuredMessage];

    /// Poll the steer channel for the next supervisor injection.
    /// Returns `None` when the channel is empty.
    ///
    /// Called by `RuntimeHost` **after** tool execution and **before**
    /// the next LLM turn.
    fn poll_steer(&mut self) -> Option<StructuredMessage>;

    /// Drain all pending follow-up messages, emptying the follow-up
    /// channel.  Called by `RunService::execute_inner` after an
    /// instance transitions to `Ready`.
    fn drain_followups(&mut self) -> Vec<StructuredMessage>;

    /// Return an immutable view of the passive `NextTurn` messages.
    /// Called by `RuntimeHost::execute_v1` when building the initial
    /// prompt list.
    fn next_turn_messages(&self) -> Vec<&StructuredMessage>;

    /// Move all `NextTurn` messages into the active conversation list
    /// and clear the passive channel.  Called by `RuntimeHost::execute_v1`
    /// before the conversation loop begins.
    fn merge_next_turn(&mut self);

    /// Convert all *active* messages (already-accumulated turns plus
    /// any injected steers) into `llm::Message`s suitable for an LLM
    /// call.  FollowUp and NextTurn messages are **excluded**.
    fn to_llm_messages(&self) -> Vec<llm::Message>;

    /// Approximate total token count across the active conversation.
    fn token_count(&self) -> usize;

    /// Run compaction on the active message list.  Returns a summary
    /// if compaction was performed, or `None` if thresholds were not
    /// met or compaction was suppressed (already-compacted tail).
    async fn compact(&mut self, policy: &ContextCompactionPolicy) -> Option<CompactSummary>;

    /// Abort any in-flight compaction operation.
    /// Implementations should cancel the active `CancellationToken`
    /// if one exists.  No-op if no compaction is in progress.
    fn abort_compaction(&mut self);

    /// Derive a TaskPacket from the current conversation state.
    /// Used for delegation and team handoff (see Context State Model §10).
    ///
    /// The default implementation extracts the most recent `TaskPacket`,
    /// or synthesizes one from the goal of the first user message if
    /// no explicit packet exists.
    fn derive_task_packet(&self, goal: &str) -> crate::message::StructuredMessage {
        // Search for an existing TaskPacket in the active messages (most recent first)
        for msg in self.active_messages().iter().rev() {
            if let crate::message::StructuredMessage::TaskPacket { .. } = msg {
                return msg.clone();
            }
        }
        // Fallback: synthesize a minimal packet from the goal
        crate::message::StructuredMessage::task_packet(goal, serde_json::json!({}))
    }
}

// ── InMemoryMessageQueue ───────────────────────────────────────

/// Default in-memory implementation of `MessageQueue` using three
/// independent channels:
///
/// - `steer_ch`: `VecDeque` drained LIFO
/// - `followup_ch`: `VecDeque` drained FIFO
/// - `nextturn_ch`: `Vec` — passive, never drained automatically
///
/// Capacity limits are configurable to prevent unbounded growth.
#[derive(Debug, Clone)]
pub struct InMemoryMessageQueue {
    /// Accumulated conversation messages (the "active" list).
    messages: Vec<StructuredMessage>,

    /// Steer channel — consumed LIFO by poll_steer().
    steer_ch: VecDeque<StructuredMessage>,

    /// Follow-up channel — consumed FIFO by drain_followups().
    followup_ch: VecDeque<StructuredMessage>,

    /// NextTurn passive channel.
    nextturn_ch: Vec<StructuredMessage>,

    /// Maximum total tokens across the active conversation.
    /// When exceeded, compaction is triggered.
    pub max_total_tokens: usize,

    /// Maximum number of steer messages that can be pending.
    /// Additional steers are dropped (with a warning log).
    pub max_steer_pending: usize,

    /// Maximum depth for followUp chain execution.
    pub max_followup_depth: usize,

    /// Cancellation token for in-flight compaction.
    /// Set when compaction starts, cleared when it completes.
    /// When `abort_compaction()` is called, this token is cancelled.
    pub abort_token: Option<CancellationToken>,
}

impl InMemoryMessageQueue {
    /// Create a queue pre-populated with initial messages.
    pub fn new(initial: Vec<StructuredMessage>) -> Self {
        Self {
            messages: initial,
            steer_ch: VecDeque::new(),
            followup_ch: VecDeque::new(),
            nextturn_ch: Vec::new(),
            max_total_tokens: 128_000, // generous default
            max_steer_pending: 8,
            max_followup_depth: 3,
            abort_token: None,
        }
    }

    /// Create an empty queue.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Approximate token count for a single message (characters / 4).
    fn estimate_msg_tokens(msg: &StructuredMessage) -> usize {
        msg.content_len() / 4
    }

    /// Total token estimate for the entire active message list.
    fn estimate_total_tokens(messages: &[StructuredMessage]) -> usize {
        messages.iter().map(Self::estimate_msg_tokens).sum()
    }
}

#[async_trait]
impl MessageQueue for InMemoryMessageQueue {
    async fn enqueue(&mut self, msg: StructuredMessage, mode: DeliveryMode) {
        match mode {
            DeliveryMode::Steer => {
                if self.steer_ch.len() >= self.max_steer_pending {
                    tracing::warn!(
                        steer_pending = self.steer_ch.len(),
                        max = self.max_steer_pending,
                        "steer channel full; dropping oldest steer"
                    );
                    self.steer_ch.pop_back(); // drop oldest (LIFO: back = oldest)
                }
                self.steer_ch.push_front(msg); // LIFO: newest at front
            }
            DeliveryMode::FollowUp => {
                self.followup_ch.push_back(msg); // FIFO
            }
            DeliveryMode::NextTurn => {
                self.nextturn_ch.push(msg);
            }
        }
    }

    fn push_conversation_message(&mut self, msg: StructuredMessage) {
        self.messages.push(msg);
    }

    fn active_messages(&self) -> &[StructuredMessage] {
        &self.messages
    }

    fn poll_steer(&mut self) -> Option<StructuredMessage> {
        // LIFO: pop from front (newest injected first)
        let msg = self.steer_ch.pop_front()?;
        // Push the injected message into the active conversation list
        // so it becomes visible to the LLM.
        self.messages.push(msg.clone());
        Some(msg)
    }

    fn drain_followups(&mut self) -> Vec<StructuredMessage> {
        std::mem::take(&mut self.followup_ch).into_iter().collect()
    }

    fn next_turn_messages(&self) -> Vec<&StructuredMessage> {
        self.nextturn_ch.iter().collect()
    }

    fn merge_next_turn(&mut self) {
        let drained: Vec<StructuredMessage> = std::mem::take(&mut self.nextturn_ch);
        self.messages.extend(drained);
    }

    fn to_llm_messages(&self) -> Vec<llm::Message> {
        self.messages
            .iter()
            .flat_map(|msg| msg.to_llm_messages())
            .collect()
    }

    fn token_count(&self) -> usize {
        Self::estimate_total_tokens(&self.messages)
    }

    async fn compact(&mut self, policy: &ContextCompactionPolicy) -> Option<CompactSummary> {
        // Convert to flat llm::Messages for the existing compaction service
        let llm_msgs: Vec<llm::Message> = self.to_llm_messages();

        if !policy.should_compact(&llm_msgs) {
            return None;
        }

        // Prevent double-compaction
        if llm_msgs.last().map_or(false, |m| CompactSummary::is_compaction_message(&m.content)) {
            return None;
        }

        let preserve = policy.preserve_recent_messages.min(llm_msgs.len());
        let split_index = llm_msgs.len().saturating_sub(preserve);
        let older = &llm_msgs[..split_index];
        let preserved_tail = llm_msgs[split_index..].to_vec();

        let key_facts: Vec<String> = older
            .iter()
            .filter_map(|message| {
                let text = message.content.trim();
                if text.is_empty() {
                    None
                } else {
                    Some(crate::context::truncate(text, policy.preview_chars))
                }
            })
            .collect();

        let summary = CompactSummary {
            compact_summary: format!(
                "Compacted {} earlier messages into a derived execution summary.",
                older.len()
            ),
            key_facts,
            preserved_tail,
        };

        // Replace active messages with the compaction marker
        let cm = StructuredMessage::CompactionMarker {
            summary: summary.clone(),
        };
        self.messages = vec![cm];

        Some(summary)
    }

    fn abort_compaction(&mut self) {
        if let Some(ref token) = self.abort_token {
            token.cancel();
            tracing::info!("In-flight compaction aborted via abort_token");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextCompactionPolicy;
    use crate::message::StructuredMessage;
    use serde_json::json;

    fn make_msg(content: &str) -> StructuredMessage {
        StructuredMessage::user(content.to_string())
    }

    #[tokio::test]
    async fn enqueue_steer_lifo_order() {
        let mut q = InMemoryMessageQueue::empty();
        q.enqueue(make_msg("steer-1"), DeliveryMode::Steer).await;
        q.enqueue(make_msg("steer-2"), DeliveryMode::Steer).await;

        // LIFO: last in, first out
        let first = q.poll_steer().unwrap();
        assert!(first.to_llm_messages()[0].content.contains("steer-2"),
                "expected steer-2 (LIFO)");

        let second = q.poll_steer().unwrap();
        assert!(second.to_llm_messages()[0].content.contains("steer-1"),
                "expected steer-1");
    }

    #[tokio::test]
    async fn enqueue_followup_fifo_order() {
        let mut q = InMemoryMessageQueue::empty();
        q.enqueue(make_msg("follow-1"), DeliveryMode::FollowUp).await;
        q.enqueue(make_msg("follow-2"), DeliveryMode::FollowUp).await;

        let drained = q.drain_followups();
        assert_eq!(drained.len(), 2);
        assert!(drained[0].to_llm_messages()[0].content.contains("follow-1"));
        assert!(drained[1].to_llm_messages()[0].content.contains("follow-2"));

        // After drain, channel is empty
        assert!(q.drain_followups().is_empty());
    }

    #[tokio::test]
    async fn nextturn_passive() {
        let mut q = InMemoryMessageQueue::empty();
        q.enqueue(make_msg("bg-1"), DeliveryMode::NextTurn).await;
        q.enqueue(make_msg("bg-2"), DeliveryMode::NextTurn).await;

        let passive = q.next_turn_messages();
        assert_eq!(passive.len(), 2);

        // to_llm_messages should NOT include nextTurn messages
        let llm_msgs = q.to_llm_messages();
        assert!(llm_msgs.is_empty(),
                "nextTurn messages should not appear in active LLM messages");
    }

    #[tokio::test]
    async fn steer_injected_into_active_messages() {
        let mut q = InMemoryMessageQueue::new(vec![make_msg("user-1")]);
        q.enqueue(make_msg("steer-1"), DeliveryMode::Steer).await;

        // Before poll, steer is NOT in active messages
        let before = q.to_llm_messages();
        assert_eq!(before.len(), 1);

        // After poll, steer IS in active messages
        q.poll_steer();
        let after = q.to_llm_messages();
        assert_eq!(after.len(), 2);
        assert!(after[1].content.contains("steer-1"));
    }

    #[tokio::test]
    async fn compaction_replaces_messages_and_skips_double() {
        let mut msgs = vec![make_msg("hello")];
        // Fill with enough messages to exceed the default threshold of 12
        for i in 0..15 {
            msgs.push(make_msg(&format!("msg-{}", i)));
        }
        let mut q = InMemoryMessageQueue::new(msgs);
        let policy = ContextCompactionPolicy::default();

        let first = q.compact(&policy).await;
        assert!(first.is_some(), "first compaction should succeed");

        // After compaction, messages should only contain the compaction marker
        let llm_msgs = q.to_llm_messages();
        assert!(llm_msgs.len() >= 1);
        assert!(llm_msgs[0].content.contains("[Context Compaction]"));

        // Second compaction should be suppressed (already compacted)
        let second = q.compact(&policy).await;
        assert!(second.is_none(), "double compaction should be suppressed");
    }

    #[tokio::test]
    async fn steer_channel_overflow_drops_oldest() {
        let mut q = InMemoryMessageQueue::empty();
        q.max_steer_pending = 2;

        q.enqueue(make_msg("steer-1"), DeliveryMode::Steer).await;
        q.enqueue(make_msg("steer-2"), DeliveryMode::Steer).await;
        q.enqueue(make_msg("steer-3"), DeliveryMode::Steer).await; // steer-1 should be dropped

        // LIFO: newest first. steer-3 at front, steer-2 at back.
        let first = q.poll_steer().unwrap();
        assert!(first.to_llm_messages()[0].content.contains("steer-3"),
                "newest steer should be first (LIFO)");

        let second = q.poll_steer().unwrap();
        assert!(second.to_llm_messages()[0].content.contains("steer-2"),
                "steer-2 should survive (steer-1 was dropped)");

        assert!(q.poll_steer().is_none(), "only 2 steers remaining");
    }
}
