use crate::models::v1::team::{
    ArtifactRef, Blocker, Decision, PublishScope, PublishedFact, SharedTaskState,
};
use crate::repository::team::{SharedTaskStateRepository, SharedTaskStateUpdate};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub struct SharedTaskStateManager {
    repo: Arc<dyn SharedTaskStateRepository>,
}

impl SharedTaskStateManager {
    pub fn new(repo: Arc<dyn SharedTaskStateRepository>) -> Self {
        Self { repo }
    }

    pub async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState> {
        self.repo.get_or_create(team_instance_id).await
    }

    pub async fn publish_artifact(
        &self,
        team_instance_id: Uuid,
        artifact_id: Uuid,
        scope: PublishScope,
        published_by: &str,
    ) -> anyhow::Result<bool> {
        let artifact_ref = ArtifactRef {
            artifact_id,
            scope,
            published_by: published_by.to_string(),
            published_at: Utc::now(),
        };
        self.repo
            .add_accepted_artifact(team_instance_id, artifact_ref)
            .await
    }

    pub async fn publish_fact(
        &self,
        team_instance_id: Uuid,
        key: &str,
        value: serde_json::Value,
        published_by: &str,
    ) -> anyhow::Result<bool> {
        let fact = PublishedFact {
            key: key.to_string(),
            value,
            published_by: published_by.to_string(),
            published_at: Utc::now(),
        };
        self.repo.add_published_fact(team_instance_id, fact).await
    }

    pub async fn update_delegation_status(
        &self,
        team_instance_id: Uuid,
        delegation_id: Uuid,
        status: &str,
    ) -> anyhow::Result<bool> {
        use crate::models::v1::team::DelegationStatusEntry;
        let entry = DelegationStatusEntry {
            delegation_id,
            status: status.to_string(),
            updated_at: Utc::now(),
        };
        self.repo
            .update_delegation_status(team_instance_id, entry)
            .await
    }

    pub async fn add_blocker(
        &self,
        team_instance_id: Uuid,
        description: &str,
        source: &str,
    ) -> anyhow::Result<bool> {
        let blocker = Blocker {
            blocker_id: Uuid::new_v4(),
            description: description.to_string(),
            source: source.to_string(),
            created_at: Utc::now(),
        };
        self.repo.add_blocker(team_instance_id, blocker).await
    }

    pub async fn resolve_blocker(
        &self,
        team_instance_id: Uuid,
        blocker_id: Uuid,
    ) -> anyhow::Result<bool> {
        self.repo
            .resolve_blocker(team_instance_id, blocker_id)
            .await
    }

    pub async fn add_decision(
        &self,
        team_instance_id: Uuid,
        description: &str,
        decided_by: &str,
    ) -> anyhow::Result<bool> {
        let decision = Decision {
            decision_id: Uuid::new_v4(),
            description: description.to_string(),
            decided_by: decided_by.to_string(),
            decided_at: Utc::now(),
        };
        self.repo.add_decision(team_instance_id, decision).await
    }

    pub async fn atomic_update<F>(
        &self,
        team_instance_id: Uuid,
        f: F,
    ) -> anyhow::Result<SharedTaskState>
    where
        F: FnOnce(&mut Vec<SharedTaskStateUpdate>) + Send + 'static,
    {
        let mut updates = Vec::new();
        f(&mut updates);
        self.repo.update_with_lock(team_instance_id, updates).await
    }
}
