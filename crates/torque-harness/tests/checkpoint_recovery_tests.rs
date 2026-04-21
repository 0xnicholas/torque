use checkpointer::Checkpointer;
use serial_test::serial;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use torque_harness::db::Database;
use torque_harness::kernel_bridge::PostgresCheckpointer;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::agent_instance::AgentInstanceCreate;
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, CheckpointRepositoryExt,
    PostgresAgentDefinitionRepository, PostgresAgentInstanceRepository,
    PostgresCheckpointRepositoryExt,
};

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

#[tokio::test]
#[serial]
async fn test_checkpoint_persistence_and_retrieval() {
    let Some(db) = setup_test_db().await else {
        return;
    };

    let checkpointer: Arc<PostgresCheckpointer> = Arc::new(PostgresCheckpointer::new(db.clone()));

    let instance_id = uuid::Uuid::new_v4();
    let task_id = uuid::Uuid::new_v4();
    let state = checkpointer::CheckpointState {
        data: serde_json::json!({
            "instance_state": "Running",
            "checkpoint_reason": "test_checkpoint",
            "active_task_state": "InProgress",
            "pending_approval_ids": Vec::<uuid::Uuid>::new(),
            "child_delegation_ids": Vec::<uuid::Uuid>::new(),
            "event_sequence": 1,
        }),
    };

    let checkpoint_id = checkpointer
        .save(instance_id, task_id, state)
        .await
        .expect("should save checkpoint");

    let loaded_state = checkpointer
        .load(checkpoint_id.clone())
        .await
        .expect("should load checkpoint");

    assert_eq!(
        loaded_state
            .data
            .get("instance_state")
            .and_then(|v| v.as_str()),
        Some("Running")
    );
    assert_eq!(
        loaded_state
            .data
            .get("checkpoint_reason")
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
        let state = checkpointer::CheckpointState {
            data: serde_json::json!({
                "instance_state": format!("State{}", i),
                "checkpoint_reason": format!("reason_{}", i),
            }),
        };
        let _ = checkpointer
            .save(instance_id, uuid::Uuid::new_v4(), state)
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

    let state = checkpointer::CheckpointState {
        data: serde_json::json!({
            "instance_state": "Ready",
            "checkpoint_reason": "test",
        }),
    };
    let checkpointer: Arc<PostgresCheckpointer> = Arc::new(PostgresCheckpointer::new(db.clone()));
    let _ = checkpointer
        .save(instance.id, instance.id, state)
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
    let state = checkpointer::CheckpointState {
        data: serde_json::json!({
            "instance_state": "Running",
            "checkpoint_reason": "awaiting_tool",
            "active_task_state": "InProgress",
            "pending_approval_ids": [],
            "child_delegation_ids": [],
            "event_sequence": 42,
        }),
    };

    let serialized = serde_json::to_string(&state).expect("should serialize");
    let deserialized: checkpointer::CheckpointState =
        serde_json::from_str(&serialized).expect("should deserialize");

    assert_eq!(
        deserialized
            .data
            .get("instance_state")
            .and_then(|v| v.as_str()),
        Some("Running")
    );
    assert_eq!(
        deserialized
            .data
            .get("checkpoint_reason")
            .and_then(|v| v.as_str()),
        Some("awaiting_tool")
    );
    assert_eq!(
        deserialized
            .data
            .get("event_sequence")
            .and_then(|v| v.as_i64()),
        Some(42)
    );
}
