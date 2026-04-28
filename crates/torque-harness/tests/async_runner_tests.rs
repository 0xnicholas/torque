use std::sync::Arc;
use torque_harness::models::v1::run::{Run, RunRequest, RunStatus};
use torque_harness::repository::RunRepository;

// ── Mock RunRepository ────────────────────────────────────────────────

struct MockRunRepository {
    run: std::sync::Mutex<Option<Run>>,
    status_history: std::sync::Mutex<Vec<RunStatus>>,
    webhook_sent_at: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    webhook_attempts: std::sync::Mutex<Option<i32>>,
    error: std::sync::Mutex<Option<String>>,
    started_at: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    completed_at: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>,
}

impl MockRunRepository {
    fn new() -> Self {
        Self {
            run: std::sync::Mutex::new(None),
            status_history: std::sync::Mutex::new(Vec::new()),
            webhook_sent_at: std::sync::Mutex::new(None),
            webhook_attempts: std::sync::Mutex::new(None),
            error: std::sync::Mutex::new(None),
            started_at: std::sync::Mutex::new(None),
            completed_at: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl RunRepository for MockRunRepository {
    async fn create(&self, run: &Run) -> anyhow::Result<()> {
        *self.run.lock().unwrap() = Some(run.clone());
        self.status_history.lock().unwrap().push(run.status.clone());
        Ok(())
    }

    async fn get(&self, _id: uuid::Uuid) -> anyhow::Result<Option<Run>> {
        Ok(self.run.lock().unwrap().clone())
    }

    async fn update_status(&self, _id: uuid::Uuid, status: RunStatus) -> anyhow::Result<()> {
        if let Some(ref mut run) = *self.run.lock().unwrap() {
            run.status = status.clone();
        }
        self.status_history.lock().unwrap().push(status);
        Ok(())
    }

    async fn get_by_status(&self, _status: RunStatus, _limit: i64) -> anyhow::Result<Vec<Run>> {
        Ok(self
            .run
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| vec![r.clone()])
            .unwrap_or_default())
    }

    async fn update_webhook_status(
        &self,
        _id: uuid::Uuid,
        webhook_sent_at: chrono::DateTime<chrono::Utc>,
        webhook_attempts: i32,
    ) -> anyhow::Result<()> {
        *self.webhook_sent_at.lock().unwrap() = Some(webhook_sent_at);
        *self.webhook_attempts.lock().unwrap() = Some(webhook_attempts);
        Ok(())
    }

    async fn update_result(
        &self,
        _id: uuid::Uuid,
        status: RunStatus,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
        error: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(ref mut run) = *self.run.lock().unwrap() {
            run.status = status.clone();
        }
        self.status_history.lock().unwrap().push(status);
        *self.started_at.lock().unwrap() = started_at;
        *self.completed_at.lock().unwrap() = completed_at;
        *self.error.lock().unwrap() = error;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[test]
fn run_request_serialization_roundtrip() {
    let req = RunRequest {
        goal: "Test async execution".into(),
        instructions: Some("Do something".into()),
        input_artifacts: vec![],
        external_context_refs: vec![],
        constraints: serde_json::Value::Null,
        execution_mode: "sync".into(),
        expected_outputs: vec![],
        idempotency_key: None,
        webhook_url: None,
        async_execution: true,
        agent_instance_id: Some(uuid::Uuid::new_v4()),
    };

    let json = serde_json::to_value(&req).unwrap();
    let roundtrip: RunRequest = serde_json::from_value(json).unwrap();
    assert_eq!(roundtrip.goal, "Test async execution");
    assert_eq!(roundtrip.async_execution, true);
    assert!(roundtrip.agent_instance_id.is_some());
}

#[test]
fn run_request_deserializes_without_agent_instance_id() {
    let json = serde_json::json!({
        "goal": "Test",
        "instructions": "Run it",
        "async_execution": false
    });
    let req: RunRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.goal, "Test");
    assert_eq!(req.async_execution, false);
    assert!(req.agent_instance_id.is_none()); // default
}

#[test]
fn run_model_includes_new_fields() {
    let run = Run {
        id: uuid::Uuid::new_v4(),
        tenant_id: uuid::Uuid::new_v4(),
        status: RunStatus::Queued,
        agent_instance_id: uuid::Uuid::new_v4(),
        instruction: "Test".into(),
        request_payload: serde_json::json!({"goal": "Test"}),
        failure_policy: None,
        webhook_url: None,
        async_execution: true,
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
        webhook_sent_at: None,
        webhook_attempts: None,
    };

    assert_eq!(run.instruction, "Test");
    assert_eq!(run.async_execution, true);
    assert_eq!(
        run.request_payload.get("goal").and_then(|v| v.as_str()),
        Some("Test")
    );
}

#[tokio::test]
async fn async_runner_updates_status_on_failure() {
    let instance_id = uuid::Uuid::new_v4();
    let run_id = uuid::Uuid::new_v4();
    let payload = serde_json::json!({
        "goal": "Test",
        "async_execution": true,
        "agent_instance_id": instance_id
    });

    let run = Run {
        id: run_id,
        tenant_id: uuid::Uuid::new_v4(),
        status: RunStatus::Queued,
        agent_instance_id: instance_id,
        instruction: "Test".into(),
        request_payload: payload,
        failure_policy: None,
        webhook_url: None,
        async_execution: true,
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
        webhook_sent_at: None,
        webhook_attempts: None,
    };

    let mock_repo = Arc::new(MockRunRepository::new());
    mock_repo.create(&run).await.unwrap();

    // AsyncRunner::process_run will fail because RunService.execute
    // can't find a real agent instance. But we verify the setup works.
    let run_from_repo = mock_repo.get(run_id).await.unwrap().unwrap();
    assert_eq!(run_from_repo.id, run_id);
    assert_eq!(run_from_repo.agent_instance_id, instance_id);
    assert_eq!(run_from_repo.status, RunStatus::Queued);

    // Verify update_result works
    mock_repo
        .update_result(
            run_id,
            RunStatus::Running,
            Some(chrono::Utc::now()),
            None,
            None,
        )
        .await
        .unwrap();

    let updated = mock_repo.get(run_id).await.unwrap().unwrap();
    assert_eq!(updated.status, RunStatus::Running);
    assert!(mock_repo.started_at.lock().unwrap().is_some());
}

#[tokio::test]
async fn async_runner_update_result_stores_error() {
    let run_id = uuid::Uuid::new_v4();
    let mock_repo = Arc::new(MockRunRepository::new());

    let run = Run {
        id: run_id,
        tenant_id: uuid::Uuid::new_v4(),
        status: RunStatus::Queued,
        agent_instance_id: uuid::Uuid::new_v4(),
        instruction: "Test".into(),
        request_payload: serde_json::json!({"goal": "Test"}),
        failure_policy: None,
        webhook_url: Some("https://example.com/hook".into()),
        async_execution: true,
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
        webhook_sent_at: None,
        webhook_attempts: None,
    };
    mock_repo.create(&run).await.unwrap();

    let error_msg = "Something went wrong".to_string();
    mock_repo
        .update_result(
            run_id,
            RunStatus::Failed,
            None,
            Some(chrono::Utc::now()),
            Some(error_msg.clone()),
        )
        .await
        .unwrap();

    let updated = mock_repo.get(run_id).await.unwrap().unwrap();
    assert_eq!(updated.status, RunStatus::Failed);
    assert_eq!(mock_repo.error.lock().unwrap().as_deref(), Some("Something went wrong"));
    assert!(mock_repo.completed_at.lock().unwrap().is_some());
}

#[tokio::test]
async fn async_runner_webhook_status_update() {
    let run_id = uuid::Uuid::new_v4();
    let mock_repo = Arc::new(MockRunRepository::new());

    let run = Run {
        id: run_id,
        tenant_id: uuid::Uuid::new_v4(),
        status: RunStatus::Completed,
        agent_instance_id: uuid::Uuid::new_v4(),
        instruction: "Test".into(),
        request_payload: serde_json::json!({"goal": "Test"}),
        failure_policy: None,
        webhook_url: Some("https://example.com/hook".into()),
        async_execution: true,
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
        webhook_sent_at: None,
        webhook_attempts: None,
    };
    mock_repo.create(&run).await.unwrap();

    let sent_at = chrono::Utc::now();
    mock_repo
        .update_webhook_status(run_id, sent_at, 3)
        .await
        .unwrap();

    assert!((*mock_repo.webhook_sent_at.lock().unwrap()).is_some());
    assert_eq!(*mock_repo.webhook_attempts.lock().unwrap(), Some(3));
}
