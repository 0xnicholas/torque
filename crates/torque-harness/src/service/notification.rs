use crate::notification::{NotificationHook, ReviewEvent, SseHook};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct NotificationService {
    hooks: Vec<Arc<dyn NotificationHook>>,
    sse_hook: Option<SseHook>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            sse_hook: None,
        }
    }

    pub fn with_webhook_hook(mut self, url: String) -> Self {
        self.hooks
            .push(Arc::new(crate::notification::WebhookHook::new(url)));
        self
    }

    pub fn with_sse_hook(mut self) -> Self {
        let (sse, _rx) = SseHook::new();
        self.sse_hook = Some(sse.clone());
        self.hooks.push(Arc::new(sse));
        self
    }

    pub fn subscribe(&self) -> Option<broadcast::Receiver<ReviewEvent>> {
        self.sse_hook.as_ref().map(|h| h.subscribe())
    }

    pub async fn notify(&self, event: &ReviewEvent) -> anyhow::Result<()> {
        for hook in &self.hooks {
            if let Err(e) = hook.send(event).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
        }
        Ok(())
    }

    pub async fn notify_candidate_needs_review(
        &self,
        candidate: &crate::models::v1::memory::MemoryWriteCandidate,
    ) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateNeedsReview(candidate.clone()))
            .await
    }

    pub async fn notify_candidate_created(
        &self,
        candidate: &crate::models::v1::memory::MemoryWriteCandidate,
    ) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateCreated(candidate.clone()))
            .await
    }

    pub async fn notify_candidate_approved(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateApproved(id)).await
    }

    pub async fn notify_candidate_rejected(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateRejected(id)).await
    }

    pub async fn notify_candidate_merged(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateMerged(id)).await
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for NotificationService {
    fn clone(&self) -> Self {
        Self {
            hooks: self.hooks.clone(),
            sse_hook: self.sse_hook.clone(),
        }
    }
}