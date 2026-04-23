use crate::message_bus::stream_bus::{StreamBus, StreamMessage};
use crate::models::v1::delegation::Delegation;
use crate::repository::DelegationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct DelegationService {
    repo: Arc<dyn DelegationRepository>,
    stream_bus: Option<Arc<dyn StreamBus>>,
}

impl DelegationService {
    pub fn new(repo: Arc<dyn DelegationRepository>) -> Self {
        Self {
            repo,
            stream_bus: None,
        }
    }

    pub fn with_stream_bus(mut self, stream_bus: Arc<dyn StreamBus>) -> Self {
        self.stream_bus = Some(stream_bus);
        self
    }

    pub async fn create(
        &self,
        task_id: Uuid,
        parent_instance_id: Uuid,
        selector: serde_json::Value,
    ) -> anyhow::Result<Delegation> {
        self.repo
            .create(task_id, parent_instance_id, selector)
            .await
    }

    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        self.repo.list(limit).await
    }

    pub async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Delegation>> {
        self.repo.list_by_instance(instance_id, limit).await
    }

    pub async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        self.repo.list_by_task(task_id, limit).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>> {
        self.repo.get(id).await
    }

    pub async fn accept(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.update_status(id, "ACCEPTED").await
    }

    pub async fn reject(&self, id: Uuid, reason: &str) -> anyhow::Result<bool> {
        let delegation = self.repo.get(id).await?;
        if delegation.is_none() {
            return Ok(false);
        }
        let delegation = delegation.unwrap();

        let result = self.repo.reject(id, reason).await?;

        if result {
            self.publish_event(
                &delegation,
                "rejected",
                serde_json::json!({
                    "reason": reason
                }),
            )
            .await;
        }

        Ok(result)
    }

    pub async fn complete(&self, id: Uuid, artifact_id: Uuid) -> anyhow::Result<bool> {
        let delegation = self.repo.get(id).await?;
        if delegation.is_none() {
            return Ok(false);
        }
        let delegation = delegation.unwrap();

        let result = self.repo.complete(id, artifact_id).await?;

        if result {
            self.publish_event(
                &delegation,
                "completed",
                serde_json::json!({
                    "artifact_id": artifact_id.to_string()
                }),
            )
            .await;
        }

        Ok(result)
    }

    pub async fn fail(&self, id: Uuid, error: &str) -> anyhow::Result<bool> {
        let delegation = self.repo.get(id).await?;
        if delegation.is_none() {
            return Ok(false);
        }
        let delegation = delegation.unwrap();

        let result = self.repo.fail(id, error).await?;

        if result {
            self.publish_event(
                &delegation,
                "failed",
                serde_json::json!({
                    "error": error
                }),
            )
            .await;
        }

        Ok(result)
    }

    async fn publish_event(
        &self,
        delegation: &Delegation,
        event_type: &str,
        event_data: serde_json::Value,
    ) {
        if let Some(bus) = &self.stream_bus {
            let stream_key = format!("delegation:{}:status", delegation.id);
            let message = StreamMessage {
                id: None,
                data: serde_json::json!({
                    "type": event_type,
                    "data": {
                        "delegation_id": delegation.id.to_string(),
                        "member_id": delegation.parent_agent_instance_id.to_string(),
                    }
                }),
                timestamp: chrono::Utc::now(),
            };
            if let Err(e) = bus.xadd(&stream_key, &message).await {
                tracing::warn!("Failed to publish delegation event: {}", e);
            }
        }
    }
}
