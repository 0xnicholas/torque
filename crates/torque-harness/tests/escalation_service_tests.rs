use async_trait::async_trait;
use chrono::Utc;
use std::sync::Mutex;
use torque_harness::models::v1::escalation::{
    Escalation, EscalationSeverity, EscalationStatus, EscalationType,
};
use torque_harness::repository::EscalationRepository;
use torque_harness::service::EscalationService;
use uuid::Uuid;

struct InMemoryEscalationRepository {
    escalations: Mutex<Vec<Escalation>>,
}

impl InMemoryEscalationRepository {
    fn new() -> Self {
        Self {
            escalations: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl EscalationRepository for InMemoryEscalationRepository {
    async fn create(
        &self,
        instance_id: Uuid,
        team_instance_id: Option<Uuid>,
        escalation_type: EscalationType,
        severity: EscalationSeverity,
        description: &str,
        context: serde_json::Value,
    ) -> anyhow::Result<Escalation> {
        let escalation = Escalation {
            id: Uuid::new_v4(),
            instance_id,
            team_instance_id,
            escalation_type,
            severity,
            status: EscalationStatus::Pending,
            description: description.to_string(),
            context,
            created_at: Utc::now(),
            resolved_at: None,
            resolved_by: None,
            resolution: None,
        };
        self.escalations.lock().unwrap().push(escalation.clone());
        Ok(escalation)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Escalation>> {
        Ok(self
            .escalations
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.id == id)
            .cloned())
    }

    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Escalation>> {
        Ok(self
            .escalations
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.instance_id == instance_id)
            .cloned()
            .collect())
    }

    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Escalation>> {
        Ok(self
            .escalations
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.team_instance_id == Some(team_instance_id))
            .cloned()
            .collect())
    }

    async fn list_pending(&self, _limit: i64) -> anyhow::Result<Vec<Escalation>> {
        Ok(self
            .escalations
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.status != EscalationStatus::Resolved && e.status != EscalationStatus::Cancelled)
            .cloned()
            .collect())
    }

    async fn update_status(&self, id: Uuid, status: EscalationStatus) -> anyhow::Result<bool> {
        let mut escalations = self.escalations.lock().unwrap();
        if let Some(e) = escalations.iter_mut().find(|e| e.id == id) {
            e.status = status;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn resolve(&self, id: Uuid, resolved_by: Uuid, resolution: &str) -> anyhow::Result<bool> {
        let mut escalations = self.escalations.lock().unwrap();
        if let Some(e) = escalations.iter_mut().find(|e| e.id == id) {
            e.status = EscalationStatus::Resolved;
            e.resolved_by = Some(resolved_by);
            e.resolution = Some(resolution.to_string());
            e.resolved_at = Some(Utc::now());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn make_service() -> (EscalationService, std::sync::Arc<InMemoryEscalationRepository>) {
    let repo = std::sync::Arc::new(InMemoryEscalationRepository::new());
    (EscalationService::new(repo.clone()), repo)
}

#[tokio::test]
async fn create_escalation_sets_pending_status() {
    let (svc, repo) = make_service();
    let instance_id = Uuid::new_v4();

    let esc = svc
        .create_escalation(
            instance_id,
            EscalationType::RecoveryFailed,
            EscalationSeverity::High,
            "Test escalation".into(),
            serde_json::json!({"key": "value"}),
        )
        .await
        .expect("create should succeed");

    assert_eq!(esc.instance_id, instance_id);
    assert_eq!(esc.status, EscalationStatus::Pending);
    assert_eq!(esc.description, "Test escalation");
    assert_eq!(esc.severity, EscalationSeverity::High);

    let stored = repo.get(esc.id).await.expect("get succeeds");
    assert!(stored.is_some());
}

#[tokio::test]
async fn list_pending_returns_created_escalation() {
    let (svc, _repo) = make_service();
    let instance_id = Uuid::new_v4();

    svc.create_escalation(
        instance_id,
        EscalationType::RecoveryFailed,
        EscalationSeverity::Critical,
        "pending".into(),
        serde_json::json!({}),
    )
    .await
    .expect("create");

    let pending = svc.list_pending_escalations(10).await.expect("list");
    assert!(!pending.is_empty());
    assert!(pending.iter().any(|e| e.description == "pending"));
}

#[tokio::test]
async fn list_pending_excludes_resolved() {
    let (svc, _repo) = make_service();
    let instance_id = Uuid::new_v4();
    let resolver = Uuid::new_v4();

    let esc = svc
        .create_escalation(
            instance_id,
            EscalationType::PolicyViolation,
            EscalationSeverity::Medium,
            "to resolve".into(),
            serde_json::json!({}),
        )
        .await
        .expect("create");

    svc.resolve_escalation(esc.id, "Resolved manually", resolver)
        .await
        .expect("resolve");

    let pending = svc.list_pending_escalations(10).await.expect("list");
    assert!(!pending.iter().any(|e| e.id == esc.id));
}

#[tokio::test]
async fn resolve_escalation_sets_resolved_fields() {
    let (svc, _repo) = make_service();
    let instance_id = Uuid::new_v4();
    let resolver = Uuid::new_v4();

    let esc = svc
        .create_escalation(
            instance_id,
            EscalationType::TeamMemberFailed,
            EscalationSeverity::Low,
            "member".into(),
            serde_json::json!({}),
        )
        .await
        .expect("create");

    let resolved = svc
        .resolve_escalation(esc.id, "Fixed by operator", resolver)
        .await
        .expect("resolve");

    assert_eq!(resolved.status, EscalationStatus::Resolved);
    assert_eq!(resolved.resolved_by, Some(resolver));
    assert_eq!(resolved.resolution.as_deref(), Some("Fixed by operator"));
    assert!(resolved.resolved_at.is_some());
}

#[tokio::test]
async fn get_escalation_returns_none_for_unknown_id() {
    let (svc, _repo) = make_service();

    let result = svc
        .get_escalation(Uuid::new_v4())
        .await
        .expect("get succeeds");
    assert!(result.is_none());
}

#[tokio::test]
async fn get_escalation_returns_created() {
    let (svc, _repo) = make_service();
    let instance_id = Uuid::new_v4();

    let esc = svc
        .create_escalation(
            instance_id,
            EscalationType::ResourceExceeded,
            EscalationSeverity::Critical,
            "resource".into(),
            serde_json::json!({}),
        )
        .await
        .expect("create");

    let found = svc.get_escalation(esc.id).await.expect("get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, esc.id);
}
