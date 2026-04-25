mod common;

use common::fake_llm::FakeLlm;
use common::setup_test_db_or_skip;
use serial_test::serial;
use std::sync::Arc;
use tokio::sync::mpsc;

use torque_harness::agent::stream::StreamEvent;
use torque_harness::kernel_bridge::PostgresCheckpointer;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::agent_instance::{AgentInstanceCreate, AgentInstanceStatus};
use torque_harness::models::v1::run::RunRequest;
use torque_harness::models::v1::task::TaskStatus;
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, PostgresAgentDefinitionRepository,
    PostgresAgentInstanceRepository, PostgresCheckpointRepository, PostgresEventRepository,
    PostgresMemoryRepositoryV1, PostgresTaskRepository, TaskRepository,
};
use torque_harness::models::v1::tool_policy::{ToolGovernanceConfig, ToolRiskLevel};
use torque_harness::policy::ToolGovernanceService;
use torque_harness::service::candidate_generator::NoOpCandidateGenerator;
use torque_harness::service::gating::MemoryGatingService;
use torque_harness::service::memory_pipeline::MemoryPipelineService;
use torque_harness::service::notification::NotificationService;
use torque_harness::service::{RunService, ToolService};

#[tokio::test]
#[serial]
async fn test_run_lifecycle_creates_task_and_updates_instance_status() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    // Setup repositories
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let inst_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepository::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

    // Setup Fake LLM that returns simple text
    let llm: Arc<dyn llm::LlmClient> = Arc::new(FakeLlm::single_text("Hello from test!"));

    // Setup ToolService
    let tools = Arc::new(ToolService::new());

    // Setup memory repo and gating
    let memory_repo = Arc::new(PostgresMemoryRepositoryV1::new(db.clone()));
    let candidate_gen = Arc::new(NoOpCandidateGenerator);
    let gating = Arc::new(MemoryGatingService::new(memory_repo.clone(), None, None));
    let notification_service = Arc::new(NotificationService::new());
    let memory_pipeline = Arc::new(MemoryPipelineService::new(
        gating.clone(),
        Some(notification_service),
    ));

    let tool_governance = Arc::new(ToolGovernanceService::new(ToolGovernanceConfig {
        default_risk_level: ToolRiskLevel::Medium,
        approval_required_above: ToolRiskLevel::High,
        blocked_tools: vec![],
        privileged_tools: vec![],
        side_effect_tracking: false,
    }));

    // Setup RunService
    let run_service = RunService::new(
        def_repo.clone(),
        inst_repo.clone(),
        task_repo.clone(),
        event_repo.clone(),
        checkpoint_repo.clone(),
        checkpointer,
        llm,
        tools,
        tool_governance.clone(),
        candidate_gen,
        gating,
        memory_pipeline,
        None,
    );

    // 1. Create agent definition
    let definition = def_repo
        .create(&AgentDefinitionCreate {
            name: "Test Runner Agent".into(),
            description: None,
            system_prompt: Some("You are a test agent.".into()),
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create agent definition");

    // 2. Create agent instance
    let instance = inst_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: definition.id,
            external_context_refs: vec![],
        })
        .await
        .expect("create agent instance");

    assert!(matches!(instance.status, AgentInstanceStatus::Created));

    // 3. Execute run
    let (event_tx, mut event_rx) = mpsc::channel::<StreamEvent>(32);

    let run_request = RunRequest {
        goal: "Say hello".into(),
        instructions: None,
        input_artifacts: vec![],
        external_context_refs: vec![],
        constraints: serde_json::json!({}),
        execution_mode: "sync".into(),
        expected_outputs: vec![],
        idempotency_key: None,
        webhook_url: None,
        async_execution: false,
    };

    let result = run_service
        .execute(instance.id, run_request, event_tx)
        .await;

    // 4. Verify execution succeeded
    assert!(result.is_ok(), "Run execution failed: {:?}", result.err());

    // 5. Collect SSE events
    let mut events = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        events.push(event);
    }

    // Should have at least Start + Chunk + Done events
    assert!(!events.is_empty(), "No events received");

    let has_start = events
        .iter()
        .any(|e| matches!(e, StreamEvent::Start { .. }));
    let has_done = events.iter().any(|e| matches!(e, StreamEvent::Done { .. }));
    assert!(has_start, "Missing Start event");
    assert!(has_done, "Missing Done event");

    // 6. Verify task was created and completed
    let tasks = task_repo.list(10).await.expect("list tasks");
    let task = tasks
        .iter()
        .find(|t| t.agent_instance_id == Some(instance.id));
    assert!(task.is_some(), "Task was not created for instance");

    let task = task.unwrap();
    assert_eq!(task.goal, "Say hello");
    assert!(
        matches!(task.status, TaskStatus::Completed),
        "Task should be Completed, got {:?}",
        task.status
    );

    // 7. Verify instance status updated to Ready
    let updated_instance = inst_repo
        .get(instance.id)
        .await
        .expect("get instance")
        .unwrap();
    assert!(
        matches!(updated_instance.status, AgentInstanceStatus::Ready),
        "Instance should be Ready, got {:?}",
        updated_instance.status
    );
    assert_eq!(
        updated_instance.current_task_id, None,
        "Instance should not have current task after completion"
    );

    // Cleanup
    inst_repo
        .delete(instance.id)
        .await
        .expect("delete instance");
    def_repo
        .delete(definition.id)
        .await
        .expect("delete definition");
}

#[tokio::test]
#[serial]
async fn test_run_with_nonexistent_instance_returns_error() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let inst_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepository::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

    let llm: Arc<dyn llm::LlmClient> = Arc::new(FakeLlm::single_text("test"));
    let tools = Arc::new(ToolService::new());

    let memory_repo = Arc::new(PostgresMemoryRepositoryV1::new(db.clone()));
    let candidate_gen = Arc::new(NoOpCandidateGenerator);
    let gating = Arc::new(MemoryGatingService::new(memory_repo.clone(), None, None));
    let notification_service = Arc::new(NotificationService::new());
    let memory_pipeline = Arc::new(MemoryPipelineService::new(
        gating.clone(),
        Some(notification_service),
    ));

    let tool_governance = Arc::new(ToolGovernanceService::new(ToolGovernanceConfig {
        default_risk_level: ToolRiskLevel::Medium,
        approval_required_above: ToolRiskLevel::High,
        blocked_tools: vec![],
        privileged_tools: vec![],
        side_effect_tracking: false,
    }));

    let run_service = RunService::new(
        def_repo,
        inst_repo,
        task_repo,
        event_repo,
        checkpoint_repo,
        checkpointer,
        llm,
        tools,
        tool_governance,
        candidate_gen,
        gating,
        memory_pipeline,
        None,
    );

    let (event_tx, mut event_rx) = mpsc::channel::<StreamEvent>(32);

    let run_request = RunRequest {
        goal: "Test".into(),
        instructions: None,
        input_artifacts: vec![],
        external_context_refs: vec![],
        constraints: serde_json::json!({}),
        execution_mode: "sync".into(),
        expected_outputs: vec![],
        idempotency_key: None,
        webhook_url: None,
        async_execution: false,
    };

    // Use a random UUID that doesn't exist
    let fake_id = uuid::Uuid::new_v4();
    let result = run_service.execute(fake_id, run_request, event_tx).await;

    assert!(result.is_err(), "Should fail with nonexistent instance");

    // Should receive Error event
    let mut has_error = false;
    while let Ok(event) = event_rx.try_recv() {
        if matches!(event, StreamEvent::Error { .. }) {
            has_error = true;
        }
    }
    assert!(
        has_error,
        "Should receive Error event for nonexistent instance"
    );
}

#[tokio::test]
#[serial]
async fn test_run_task_status_transitions() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let inst_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepository::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

    let llm: Arc<dyn llm::LlmClient> = Arc::new(FakeLlm::single_text("Task completed!"));
    let tools = Arc::new(ToolService::new());

    let memory_repo = Arc::new(PostgresMemoryRepositoryV1::new(db.clone()));
    let candidate_gen = Arc::new(NoOpCandidateGenerator);
    let gating = Arc::new(MemoryGatingService::new(memory_repo.clone(), None, None));
    let notification_service = Arc::new(NotificationService::new());
    let memory_pipeline = Arc::new(MemoryPipelineService::new(
        gating.clone(),
        Some(notification_service),
    ));

    let tool_governance = Arc::new(ToolGovernanceService::new(ToolGovernanceConfig {
        default_risk_level: ToolRiskLevel::Medium,
        approval_required_above: ToolRiskLevel::High,
        blocked_tools: vec![],
        privileged_tools: vec![],
        side_effect_tracking: false,
    }));

    let run_service = RunService::new(
        def_repo.clone(),
        inst_repo.clone(),
        task_repo.clone(),
        event_repo.clone(),
        checkpoint_repo.clone(),
        checkpointer,
        llm,
        tools,
        tool_governance,
        candidate_gen,
        gating,
        memory_pipeline,
        None,
    );

    // Create definition and instance
    let definition = def_repo
        .create(&AgentDefinitionCreate {
            name: "Transition Test Agent".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create definition");

    let instance = inst_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: definition.id,
            external_context_refs: vec![],
        })
        .await
        .expect("create instance");

    // Execute run
    let (event_tx, _event_rx) = mpsc::channel::<StreamEvent>(32);
    let run_request = RunRequest {
        goal: "Test transitions".into(),
        instructions: None,
        input_artifacts: vec![],
        external_context_refs: vec![],
        constraints: serde_json::json!({}),
        execution_mode: "sync".into(),
        expected_outputs: vec![],
        idempotency_key: None,
        webhook_url: None,
        async_execution: false,
    };

    run_service
        .execute(instance.id, run_request, event_tx)
        .await
        .expect("execute run");

    // Verify task lifecycle
    let tasks = task_repo.list(10).await.expect("list tasks");
    let task = tasks
        .iter()
        .find(|t| t.agent_instance_id == Some(instance.id))
        .expect("task should exist");

    // Task should end in Completed or Failed (not Created or Running)
    assert!(
        matches!(task.status, TaskStatus::Completed | TaskStatus::Failed),
        "Task should be in terminal state, got {:?}",
        task.status
    );

    // Instance should be in terminal state
    let updated_instance = inst_repo
        .get(instance.id)
        .await
        .expect("get instance")
        .unwrap();
    assert!(
        matches!(
            updated_instance.status,
            AgentInstanceStatus::Ready | AgentInstanceStatus::Failed
        ),
        "Instance should be in terminal state, got {:?}",
        updated_instance.status
    );

    // Cleanup
    inst_repo
        .delete(instance.id)
        .await
        .expect("delete instance");
    def_repo
        .delete(definition.id)
        .await
        .expect("delete definition");
}
