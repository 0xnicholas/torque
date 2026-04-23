use crate::service::gating::MemoryGatingService;
use crate::service::notification::NotificationService;
use crate::models::v1::memory::MemoryWriteCandidate;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryPipelineService {
    gating: Arc<MemoryGatingService>,
    notification: Option<Arc<NotificationService>>,
}

impl MemoryPipelineService {
    pub fn new(
        gating: Arc<MemoryGatingService>,
        notification: Option<Arc<NotificationService>>,
    ) -> Self {
        Self { gating, notification }
    }

    pub async fn gate_and_notify(
        &self,
        candidate: &MemoryWriteCandidate,
    ) -> anyhow::Result<crate::models::v1::gating::GateDecision> {
        let decision = self.gating.gate_candidate(candidate).await?;

        if let Some(ref notify) = self.notification {
            match &decision.decision {
                crate::models::v1::gating::GateDecisionType::Review => {
                    let _ = notify.notify_candidate_needs_review(candidate).await;
                }
                crate::models::v1::gating::GateDecisionType::Approve => {
                    let _ = notify.notify_candidate_created(candidate).await;
                }
                crate::models::v1::gating::GateDecisionType::Merge => {
                    if let Some(target_id) = decision.target_entry_id {
                        let _ = notify.notify_candidate_merged(target_id).await;
                    }
                }
                crate::models::v1::gating::GateDecisionType::Reject => {
                    let _ = notify.notify_candidate_rejected(candidate.id).await;
                }
            }
        }

        Ok(decision)
    }
}