use crate::models::v1::escalation::{Escalation, EscalationSeverity, EscalationType};
use crate::repository::escalation::EscalationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct EscalationService {
    escalation_repo: Arc<dyn EscalationRepository>,
}

impl EscalationService {
    pub fn new(escalation_repo: Arc<dyn EscalationRepository>) -> Self {
        Self { escalation_repo }
    }

    pub async fn create_escalation(
        &self,
        instance_id: Uuid,
        escalation_type: EscalationType,
        severity: EscalationSeverity,
        description: String,
        context: serde_json::Value,
    ) -> anyhow::Result<Escalation> {
        self.escalation_repo
            .create(
                instance_id,
                None,
                escalation_type,
                severity,
                &description,
                context,
            )
            .await
    }

    pub async fn list_pending_escalations(&self, limit: i64) -> anyhow::Result<Vec<Escalation>> {
        self.escalation_repo.list_pending(limit).await
    }

    pub async fn get_escalation(&self, id: Uuid) -> anyhow::Result<Option<Escalation>> {
        self.escalation_repo.get(id).await
    }

    pub async fn resolve_escalation(
        &self,
        id: Uuid,
        resolution: &str,
        resolved_by: Uuid,
    ) -> anyhow::Result<Escalation> {
        self.escalation_repo
            .resolve(id, resolved_by, resolution)
            .await?;
        self.escalation_repo
            .get(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Escalation not found"))
    }
}