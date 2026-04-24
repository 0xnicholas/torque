use torque_harness::models::v1::delegation::DelegationStatus;
use torque_harness::models::v1::team::{
    Blocker, Decision, DelegationStatusEntry, PolicyCheckSummary, PublishedFact,
    SharedTaskState, TeamTaskStatus,
};
use torque_harness::service::team::SharedTaskStateManager;
use torque_harness::repository::SharedTaskStateRepository;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

struct MockSharedTaskStateRepository {
    state: SharedTaskState,
}

impl MockSharedTaskStateRepository {
    fn new() -> Self {
        Self {
            state: SharedTaskState {
                id: Uuid::new_v4(),
                team_instance_id: Uuid::new_v4(),
                accepted_artifact_refs: vec![],
                published_facts: vec![],
                delegation_status: vec![],
                open_blockers: vec![],
                decisions: vec![],
                updated_at: chrono::Utc::now(),
            },
        }
    }
}

#[async_trait]
impl SharedTaskStateRepository for MockSharedTaskStateRepository {
    async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState> {
        let mut state = self.state.clone();
        state.team_instance_id = team_instance_id;
        Ok(state)
    }

    async fn get(&self, team_instance_id: Uuid) -> anyhow::Result<Option<SharedTaskState>> {
        let mut state = self.state.clone();
        state.team_instance_id = team_instance_id;
        Ok(Some(state))
    }

    async fn add_accepted_artifact(
        &self,
        _team_instance_id: Uuid,
        _artifact_ref: torque_harness::models::v1::team::ArtifactRef,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn add_published_fact(
        &self,
        _team_instance_id: Uuid,
        _fact: PublishedFact,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn update_delegation_status(
        &self,
        _team_instance_id: Uuid,
        _entry: DelegationStatusEntry,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn add_blocker(&self, _team_instance_id: Uuid, _blocker: Blocker) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn resolve_blocker(&self, _team_instance_id: Uuid, _blocker_id: Uuid) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn add_decision(&self, _team_instance_id: Uuid, _decision: Decision) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn update_with_lock(
        &self,
        _team_instance_id: Uuid,
        _updates: Vec<torque_harness::repository::team::SharedTaskStateUpdate>,
    ) -> anyhow::Result<SharedTaskState> {
        Ok(self.state.clone())
    }
}

#[tokio::test]
async fn test_shared_task_state_manager_get_or_create() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();

    let state = manager.get_or_create(team_instance_id).await.unwrap();
    assert_eq!(state.team_instance_id, team_instance_id);
}

#[tokio::test]
async fn test_shared_task_state_manager_publish_artifact() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();
    let artifact_id = Uuid::new_v4();

    let result = manager
        .publish_artifact(
            team_instance_id,
            artifact_id,
            torque_harness::models::v1::team::PublishScope::TeamShared,
            "test_member",
        )
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_shared_task_state_manager_publish_fact() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();

    let result = manager
        .publish_fact(team_instance_id, "key", serde_json::json!("value"), "test_member")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_shared_task_state_manager_update_delegation_status() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();
    let delegation_id = Uuid::new_v4();

    let result = manager
        .update_delegation_status(team_instance_id, delegation_id, "ACCEPTED")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_shared_task_state_manager_add_blocker() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();

    let result = manager
        .add_blocker(team_instance_id, "Test blocker", "test_source")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_shared_task_state_manager_resolve_blocker() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();
    let blocker_id = Uuid::new_v4();

    let result = manager.resolve_blocker(team_instance_id, blocker_id).await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_shared_task_state_manager_add_decision() {
    let repo = Arc::new(MockSharedTaskStateRepository::new());
    let manager = SharedTaskStateManager::new(repo.clone());
    let team_instance_id = Uuid::new_v4();

    let result = manager
        .add_decision(team_instance_id, "Test decision", "test_decider")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_delegation_status_entry_creation() {
    let delegation_id = Uuid::new_v4();
    let entry = DelegationStatusEntry {
        delegation_id,
        status: "ACCEPTED".to_string(),
        updated_at: chrono::Utc::now(),
    };

    assert_eq!(entry.delegation_id, delegation_id);
    assert_eq!(entry.status, "ACCEPTED");
}

#[tokio::test]
async fn test_blocker_creation() {
    let blocker = Blocker {
        blocker_id: Uuid::new_v4(),
        description: "Test blocker".to_string(),
        source: "test_source".to_string(),
        created_at: chrono::Utc::now(),
    };

    assert_eq!(blocker.description, "Test blocker");
    assert_eq!(blocker.source, "test_source");
}

#[tokio::test]
async fn test_decision_creation() {
    let decision = Decision {
        decision_id: Uuid::new_v4(),
        description: "Test decision".to_string(),
        decided_by: "test_decider".to_string(),
        decided_at: chrono::Utc::now(),
    };

    assert_eq!(decision.description, "Test decision");
    assert_eq!(decision.decided_by, "test_decider");
}

#[tokio::test]
async fn test_published_fact_creation() {
    let fact = PublishedFact {
        key: "test_key".to_string(),
        value: serde_json::json!("test_value"),
        published_by: "test_publisher".to_string(),
        published_at: chrono::Utc::now(),
    };

    assert_eq!(fact.key, "test_key");
    assert_eq!(fact.published_by, "test_publisher");
}

#[tokio::test]
async fn test_policy_check_summary() {
    let summary = PolicyCheckSummary {
        resource_available: true,
        approval_required: false,
        risk_level: "low".to_string(),
    };

    assert!(summary.resource_available);
    assert!(!summary.approval_required);
    assert_eq!(summary.risk_level, "low");
}

#[tokio::test]
async fn test_team_task_status_variants() {
    assert_eq!(TeamTaskStatus::Open.to_string(), "OPEN");
    assert_eq!(TeamTaskStatus::Triaged.to_string(), "TRIAGED");
    assert_eq!(TeamTaskStatus::InProgress.to_string(), "IN_PROGRESS");
    assert_eq!(TeamTaskStatus::WaitingMembers.to_string(), "WAITING_MEMBERS");
    assert_eq!(TeamTaskStatus::Completed.to_string(), "COMPLETED");
    assert_eq!(TeamTaskStatus::Failed.to_string(), "FAILED");
}

#[tokio::test]
async fn test_delegation_status_constants() {
    assert_eq!(DelegationStatus::Pending.to_string(), "PENDING");
    assert_eq!(DelegationStatus::Accepted.to_string(), "ACCEPTED");
    assert_eq!(DelegationStatus::Completed.to_string(), "COMPLETED");
    assert_eq!(DelegationStatus::Rejected.to_string(), "REJECTED");
    assert_eq!(DelegationStatus::Failed.to_string(), "FAILED");
}