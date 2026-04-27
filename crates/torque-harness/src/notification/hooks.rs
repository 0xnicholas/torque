use crate::models::v1::memory::MemoryWriteCandidate;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum ReviewEvent {
    CandidateCreated(MemoryWriteCandidate),
    CandidateNeedsReview(MemoryWriteCandidate),
    CandidateApproved(uuid::Uuid),
    CandidateRejected(uuid::Uuid),
    CandidateMerged(uuid::Uuid),
}

impl ReviewEvent {
    pub fn candidate_id(&self) -> Option<uuid::Uuid> {
        match self {
            ReviewEvent::CandidateCreated(c) => Some(c.id),
            ReviewEvent::CandidateNeedsReview(c) => Some(c.id),
            ReviewEvent::CandidateApproved(id) => Some(*id),
            ReviewEvent::CandidateRejected(id) => Some(*id),
            ReviewEvent::CandidateMerged(id) => Some(*id),
        }
    }
}

#[async_trait::async_trait]
pub trait NotificationHook: Send + Sync {
    async fn send(&self, event: &ReviewEvent) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct WebhookHook {
    url: String,
    client: reqwest::Client,
}

impl WebhookHook {
    pub fn new(url: String) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl NotificationHook for WebhookHook {
    async fn send(&self, event: &ReviewEvent) -> anyhow::Result<()> {
        let payload = match event {
            ReviewEvent::CandidateCreated(c) => serde_json::json!({
                "type": "candidate_created",
                "candidate_id": c.id.to_string(),
                "category": c.content
            }),
            ReviewEvent::CandidateNeedsReview(c) => serde_json::json!({
                "type": "candidate_needs_review",
                "candidate_id": c.id.to_string(),
                "reasoning": c.reasoning
            }),
            ReviewEvent::CandidateApproved(id) => serde_json::json!({
                "type": "candidate_approved",
                "candidate_id": id.to_string()
            }),
            ReviewEvent::CandidateRejected(id) => serde_json::json!({
                "type": "candidate_rejected",
                "candidate_id": id.to_string()
            }),
            ReviewEvent::CandidateMerged(id) => serde_json::json!({
                "type": "candidate_merged",
                "candidate_id": id.to_string()
            }),
        };

        let response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Webhook returned error: {}", response.status());
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct SseHook {
    sender: broadcast::Sender<ReviewEvent>,
}

impl SseHook {
    pub fn new() -> (Self, broadcast::Receiver<ReviewEvent>) {
        let (tx, rx) = broadcast::channel(100);
        (Self { sender: tx }, rx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ReviewEvent> {
        self.sender.subscribe()
    }

    pub fn sender(&self) -> broadcast::Sender<ReviewEvent> {
        self.sender.clone()
    }
}

impl Default for SseHook {
    fn default() -> Self {
        Self {
            sender: broadcast::channel(100).0,
        }
    }
}

#[async_trait::async_trait]
impl NotificationHook for SseHook {
    async fn send(&self, event: &ReviewEvent) -> anyhow::Result<()> {
        let _ = self.sender.send(event.clone());
        Ok(())
    }
}
