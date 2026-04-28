use serial_test::serial;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use torque_harness::db::Database;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::agent_instance::{AgentInstanceCreate, AgentInstanceStatus};
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, CheckpointRepositoryExt,
    PostgresAgentDefinitionRepository, PostgresAgentInstanceRepository,
    PostgresCheckpointRepositoryExt,
};
use torque_harness::runtime::checkpoint::PostgresCheckpointer;
use torque_harness::service::RecoveryService;
use torque_kernel::AgentInstanceId;
use torque_runtime::checkpoint::{Message, RuntimeCheckpointPayload};
use torque_runtime::RuntimeCheckpointSink;
use uuid::Uuid;

async fn setup_test_db() -> Option<Database> {
    let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost/torque_harness_test".to_string()
    });

    let pool = match PgPoolOptions::new().connect_lazy(&database_url) {
        Ok(pool) => pool,
        Err(_) => return None,
    };

    Some(Database::new(pool))
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct CheckpointState {
    messages: Vec<Message>,
    tool_call_count: usize,
    intermediate_results: Vec<serde_json::Value>,
    custom_state: Option<serde_json::Value>,
}

async fn save_checkpoint(
    checkpointer: &PostgresCheckpointer,
    instance_id: Uuid,
    state: CheckpointState,
) -> anyhow::Result<Uuid> {
    let state_json = serde_json::to_value(&state)?;
    let payload = RuntimeCheckpointPayload {
        instance_id: AgentInstanceId::from_uuid(instance_id),
        node_id: instance_id,
        reason: "test_checkpoint".to_string(),
        state: state_json,
    };
    let cp_ref = checkpointer.save(payload).await?;
    Ok(cp_ref.checkpoint_id)
}

#[tokio::test]
#[serial]
async fn test_checkpoint_persistence_and_retrieval() {
    let Some(db) = setup_test_db().await else {
        return;
    };

    let checkpointer: Arc<PostgresCheckpointer> = Arc::new(PostgresCheckpointer::new(db.clone()));

    let instance_id = uuid::Uuid::new_v4();
    let task_id = uuid::Uuid::new_v4();
    let state = CheckpointState {
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Running",
            "checkpoint_reason": "test_checkpoint",
            "active_task_state": "InProgress",
            "pending_approval_ids": Vec::<uuid::Uuid>::new(),
            "child_delegation_ids": Vec::<uuid::Uuid>::new(),
            "event_sequence": 1,
        })),
    };

    let checkpoint_id = save_checkpoint(checkpointer.as_ref(), instance_id, state)
        .await
        .expect("should save checkpoint");

    let loaded_state = checkpointer
        .load(checkpoint_id.clone())
        .await
        .expect("should load checkpoint");

    assert_eq!(
        loaded_state
            .get("custom_state")
            .as_ref()
            .and_then(|v| v.get("instance_state"))
            .and_then(|v| v.as_str()),
        Some("Running")
    );
    assert_eq!(
        loaded_state
            .get("custom_state")
            .as_ref()
            .and_then(|v| v.get("checkpoint_reason"))
            .and_then(|v| v.as_str()),
        Some("test_checkpoint")
    );

    checkpointer
        .delete(checkpoint_id)
        .await
        .expect("should delete checkpoint");
}

#[tokio::test]
#[serial]
async fn test_checkpoint_list_by_instance() {
    let Some(db) = setup_test_db().await else {
        return;
    };

    let checkpointer: Arc<PostgresCheckpointer> = Arc::new(PostgresCheckpointer::new(db.clone()));

    let instance_id = uuid::Uuid::new_v4();

    for i in 0..3 {
        let state = CheckpointState {
            messages: vec![],
            tool_call_count: 0,
            intermediate_results: vec![],
            custom_state: Some(serde_json::json!({
                "instance_state": format!("State{}", i),
                "checkpoint_reason": format!("reason_{}", i),
            })),
        };
        let _ = save_checkpoint(checkpointer.as_ref(), instance_id, state)
            .await
            .expect("should save checkpoint");
    }

    let checkpoints = checkpointer
        .list_run_checkpoints(instance_id)
        .await
        .expect("should list checkpoints");

    assert_eq!(checkpoints.len(), 3);

    for cp in checkpoints {
        let _ = checkpointer.delete(cp.id).await;
    }
}

#[tokio::test]
#[serial]
async fn test_checkpoint_model_query() {
    let Some(db) = setup_test_db().await else {
        return;
    };

    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));

    let def = def_repo
        .create(&AgentDefinitionCreate {
            name: "test-agent".to_string(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("should create agent definition");

    let instance = instance_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: def.id,
            external_context_refs: vec![],
        })
        .await
        .expect("should create agent instance");

    let state = CheckpointState {
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Ready",
            "checkpoint_reason": "test",
        })),
    };
    let checkpointer: Arc<PostgresCheckpointer> = Arc::new(PostgresCheckpointer::new(db.clone()));
    let _ = save_checkpoint(checkpointer.as_ref(), instance.id, state)
        .await
        .expect("should save checkpoint");

    let checkpoints = checkpoint_repo
        .list_by_instance(instance.id, 10)
        .await
        .expect("should list checkpoints for instance");

    assert!(!checkpoints.is_empty());

    let loaded = checkpoint_repo
        .get(checkpoints[0].id)
        .await
        .expect("should get checkpoint")
        .expect("checkpoint should exist");

    assert_eq!(loaded.agent_instance_id, instance.id);
}

#[tokio::test]
#[serial]
async fn test_checkpoint_state_format() {
    let state = CheckpointState {
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Running",
            "checkpoint_reason": "awaiting_tool",
            "active_task_state": "InProgress",
            "pending_approval_ids": [],
            "child_delegation_ids": [],
            "event_sequence": 42,
        })),
    };

    let serialized = serde_json::to_string(&state).expect("should serialize");
    let deserialized: CheckpointState =
        serde_json::from_str(&serialized).expect("should deserialize");

    assert_eq!(
        deserialized
            .custom_state
            .as_ref()
            .and_then(|v| v.get("instance_state"))
            .and_then(|v| v.as_str()),
        Some("Running")
    );
    assert_eq!(
        deserialized
            .custom_state
            .as_ref()
            .and_then(|v| v.get("checkpoint_reason"))
            .and_then(|v| v.as_str()),
        Some("awaiting_tool")
    );
    assert_eq!(
        deserialized
            .custom_state
            .as_ref()
            .and_then(|v| v.get("event_sequence"))
            .and_then(|v| v.as_i64()),
        Some(42)
    );
}

#[tokio::test]
#[serial]
async fn test_recovery_service_reads_checkpoint_format() {
    use torque_harness::repository::PostgresEventRepositoryExt;
    use torque_harness::service::RecoveryService;

    let Some(db) = setup_test_db().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepositoryExt::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

    let def = def_repo
        .create(&AgentDefinitionCreate {
            name: "test".to_string(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .unwrap();

    let instance = instance_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: def.id,
            external_context_refs: vec![],
        })
        .await
        .unwrap();

    instance_repo
        .update_status(instance.id, AgentInstanceStatus::Running)
        .await
        .unwrap();

    let state = CheckpointState {
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Ready",
            "checkpoint_reason": "test",
            "active_task_state": "InProgress",
            "pending_approval_ids": Vec::<uuid::Uuid>::new(),
            "child_delegation_ids": Vec::<uuid::Uuid>::new(),
            "event_sequence": 1,
        })),
    };
    let checkpoint_id = save_checkpoint(checkpointer.as_ref(), instance.id, state)
        .await
        .unwrap();

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);
    let result = recovery.restore_from_checkpoint(checkpoint_id).await;

    assert!(
        result.is_ok(),
        "RecoveryService should read checkpoint format correctly: {:?}",
        result.err()
    );

    let (restored, _messages, _rebuilt_state) = result.unwrap();
    assert_eq!(
        restored.status,
        AgentInstanceStatus::Ready,
        "Instance should be restored to Ready status"
    );
}

#[tokio::test]
#[serial]
async fn test_reconciliation_resolves_child_failure() {
    use torque_harness::repository::PostgresEventRepositoryExt;
    use torque_harness::service::RecoveryService;

    let Some(db) = setup_test_db().await else {
        return;
    };

    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepositoryExt::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

    let def = def_repo
        .create(&AgentDefinitionCreate {
            name: "test".to_string(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .unwrap();

    let parent = instance_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: def.id,
            external_context_refs: vec![],
        })
        .await
        .unwrap();

    let child = instance_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: def.id,
            external_context_refs: vec![],
        })
        .await
        .unwrap();

    let state = CheckpointState {
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "WAITING_SUBAGENT",
            "checkpoint_reason": "delegation_pending",
            "active_task_state": "InProgress",
            "pending_approval_ids": Vec::<uuid::Uuid>::new(),
            "child_delegation_ids": vec![child.id.to_string()],
            "event_sequence": 1,
        })),
    };
    let checkpoint_id = save_checkpoint(checkpointer.as_ref(), parent.id, state)
        .await
        .unwrap();

    instance_repo
        .update_status(child.id, AgentInstanceStatus::Failed)
        .await
        .unwrap();

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);
    let result = recovery.restore_from_checkpoint(checkpoint_id).await;

    assert!(result.is_ok(), "Restore should succeed: {:?}", result.err());

    let (restored, _messages, _rebuilt_state) = result.unwrap();
    assert!(
        matches!(restored.status, AgentInstanceStatus::Ready),
        "Parent should be set to Ready after detecting child failure, got {:?}",
        restored.status
    );
}

#[tokio::test]
#[serial]
async fn test_recovery_assess_recovery() {
    use torque_harness::repository::PostgresEventRepositoryExt;
    use torque_harness::service::recovery::{RecoveryAction, RecoveryDisposition};
    use torque_harness::service::RecoveryService;

    let Some(db) = setup_test_db().await else {
        return;
    };

    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepositoryExt::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

    let def = def_repo
        .create(&AgentDefinitionCreate {
            name: "test".to_string(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .unwrap();

    let instance = instance_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: def.id,
            external_context_refs: vec![],
        })
        .await
        .unwrap();

    let state = CheckpointState {
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "WAITING_SUBAGENT",
            "checkpoint_reason": "delegation_pending",
            "active_task_state": "InProgress",
            "pending_approval_ids": Vec::<uuid::Uuid>::new(),
            "child_delegation_ids": Vec::<uuid::Uuid>::new(),
            "event_sequence": 1,
        })),
    };
    let checkpoint_id = save_checkpoint(checkpointer.as_ref(), instance.id, state)
        .await
        .unwrap();

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);

    let assessment = recovery.assess_recovery(checkpoint_id).await;
    assert!(
        assessment.is_ok(),
        "assess_recovery should succeed: {:?}",
        assessment.err()
    );

    let a = assessment.unwrap();
    assert_eq!(a.instance_id, instance.id);
    assert_eq!(
        a.disposition,
        RecoveryDisposition::AwaitingDelegation,
        "disposition was {:?}, expected AwaitingDelegation",
        a.disposition
    );
    assert!(!a.terminal, "AwaitingDelegation should not be terminal");
    assert!(matches!(
        a.recommended_action,
        RecoveryAction::AwaitDelegationCompletion
    ));
}

#[tokio::test]
#[serial]
async fn test_full_recovery_flow_restore_and_resume() {
    use torque_harness::repository::PostgresEventRepositoryExt;
    use torque_harness::service::RecoveryService;

    let Some(db) = setup_test_db().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepositoryExt::new(db.clone()));

    let def = def_repo
        .create(&AgentDefinitionCreate {
            name: "test-agent".to_string(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .unwrap();

    let instance = instance_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: def.id,
            external_context_refs: vec![],
        })
        .await
        .unwrap();

    let state = CheckpointState {
        messages: vec![
            Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            Message {
                role: "assistant".to_string(),
                content: "I'll help you with that.".to_string(),
            },
        ],
        tool_call_count: 1,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "WAITING_TOOL",
            "checkpoint_reason": "awaiting_tool_completion",
            "active_task_state": "InProgress",
            "pending_approval_ids": Vec::<uuid::Uuid>::new(),
            "child_delegation_ids": Vec::<uuid::Uuid>::new(),
            "event_sequence": 10,
        })),
    };
    let checkpoint_id = save_checkpoint(checkpointer.as_ref(), instance.id, state)
        .await
        .unwrap();

    instance_repo
        .update_status(instance.id, AgentInstanceStatus::Failed)
        .await
        .unwrap();

    let recovery = RecoveryService::new(
        instance_repo.clone(),
        checkpoint_repo.clone(),
        event_repo.clone(),
    );

    let (restored, _messages, _rebuilt_state) = recovery
        .restore_from_checkpoint(checkpoint_id)
        .await
        .unwrap();

    assert!(
        matches!(
            restored.status,
            AgentInstanceStatus::Ready | AgentInstanceStatus::AwaitingTool
        ),
        "Instance should be restored, got {:?}",
        restored.status
    );

    let loaded = checkpointer.load(checkpoint_id).await.unwrap();
    assert_eq!(loaded.get("messages").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0), 2, "Should preserve message history");
    assert_eq!(loaded.get("tool_call_count").and_then(|v| v.as_u64()).unwrap_or(0), 1);

    let assessment = recovery.assess_recovery(checkpoint_id).await.unwrap();
    assert!(
        !assessment.is_terminal(),
        "Assessment should not be terminal for AwaitingTool"
    );
}
