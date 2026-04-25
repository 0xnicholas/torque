use checkpointer::r#trait::Message;
use checkpointer::Checkpointer;
use serial_test::serial;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use torque_harness::db::Database;
use torque_harness::kernel_bridge::PostgresCheckpointer;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::agent_instance::{AgentInstanceCreate, AgentInstanceStatus};
use torque_harness::repository::PostgresEventRepositoryExt;
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, CheckpointRepositoryExt,
    PostgresAgentDefinitionRepository, PostgresAgentInstanceRepository,
    PostgresCheckpointRepositoryExt,
};
use torque_harness::service::RecoveryService;
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

#[tokio::test]
#[serial]
async fn test_get_checkpoint_messages_returns_messages() {
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

    let test_messages = vec![
        Message {
            role: "user".to_string(),
            content: "Hello, how are you?".to_string(),
        },
        Message {
            role: "assistant".to_string(),
            content: "I'm doing well, thank you!".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: "Can you help me with something?".to_string(),
        },
    ];

    let state = checkpointer::CheckpointState {
        messages: test_messages.clone(),
        tool_call_count: 2,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Running",
            "checkpoint_reason": "test_messages",
        })),
    };

    let checkpoint_id = checkpointer
        .save(instance.id, instance.id, state)
        .await
        .expect("should save checkpoint");

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);

    let messages = recovery
        .get_checkpoint_messages(checkpoint_id.0)
        .await
        .expect("should get checkpoint messages");

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello, how are you?");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "I'm doing well, thank you!");
    assert_eq!(messages[2].role, "user");
    assert_eq!(messages[2].content, "Can you help me with something?");

    let _ = checkpointer.delete(checkpoint_id).await;
}

#[tokio::test]
#[serial]
async fn test_get_checkpoint_messages_empty_for_checkpoint_without_messages() {
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
        messages: vec![],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Ready",
            "checkpoint_reason": "test_empty_messages",
        })),
    };

    let checkpoint_id = checkpointer
        .save(instance.id, instance.id, state)
        .await
        .expect("should save checkpoint");

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);

    let messages = recovery
        .get_checkpoint_messages(checkpoint_id.0)
        .await
        .expect("should get checkpoint messages");

    assert!(messages.is_empty());

    let _ = checkpointer.delete(checkpoint_id).await;
}

#[tokio::test]
#[serial]
async fn test_get_checkpoint_messages_nonexistent_checkpoint() {
    let Some(db) = setup_test_db().await else {
        return;
    };

    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepositoryExt::new(db.clone()));

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);

    let fake_id = Uuid::new_v4();
    let messages = recovery
        .get_checkpoint_messages(fake_id)
        .await
        .expect("should handle nonexistent checkpoint gracefully");

    assert!(messages.is_empty());
}

#[tokio::test]
#[serial]
async fn test_restore_from_checkpoint_returns_messages() {
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

    instance_repo
        .update_status(instance.id, AgentInstanceStatus::Running)
        .await
        .unwrap();

    let test_messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: "Tell me a joke.".to_string(),
        },
    ];

    let state = checkpointer::CheckpointState {
        messages: test_messages.clone(),
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Ready",
            "checkpoint_reason": "test_restore_messages",
        })),
    };

    let checkpoint_id = checkpointer
        .save(instance.id, instance.id, state)
        .await
        .expect("should save checkpoint");

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);

    let (restored_instance, messages, _rebuilt_state) = recovery
        .restore_from_checkpoint(checkpoint_id.0)
        .await
        .expect("should restore from checkpoint");

    assert_eq!(restored_instance.id, instance.id);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "system");
    assert_eq!(messages[0].content, "You are a helpful assistant.");
    assert_eq!(messages[1].role, "user");
    assert_eq!(messages[1].content, "Tell me a joke.");

    let _ = checkpointer.delete(checkpoint_id).await;
}

#[tokio::test]
#[serial]
async fn test_checkpoint_repository_get_messages() {
    let Some(db) = setup_test_db().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let instance_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepositoryExt::new(db.clone()));
    let checkpointer = Arc::new(PostgresCheckpointer::new(db.clone()));

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
        messages: vec![Message {
            role: "assistant".to_string(),
            content: "This is a test message.".to_string(),
        }],
        tool_call_count: 1,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Running",
        })),
    };

    let checkpoint_id = checkpointer
        .save(instance.id, instance.id, state)
        .await
        .expect("should save checkpoint");

    let messages = checkpoint_repo
        .get_messages(checkpoint_id.0)
        .await
        .expect("should get messages from repository");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(messages[0].content, "This is a test message.");

    let _ = checkpointer.delete(checkpoint_id).await;
}

#[tokio::test]
#[serial]
async fn test_resume_instance_returns_messages() {
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

    let test_messages = vec![
        Message {
            role: "user".to_string(),
            content: "First message".to_string(),
        },
        Message {
            role: "assistant".to_string(),
            content: "Second message".to_string(),
        },
    ];

    let state = checkpointer::CheckpointState {
        messages: test_messages.clone(),
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: Some(serde_json::json!({
            "instance_state": "Running",
            "checkpoint_reason": "test_resume",
        })),
    };

    let _checkpoint_id = checkpointer
        .save(instance.id, instance.id, state)
        .await
        .expect("should save checkpoint");

    let recovery = RecoveryService::new(instance_repo.clone(), checkpoint_repo.clone(), event_repo);

    let (restored_instance, messages, _rebuilt_state) = recovery
        .resume_instance(instance.id)
        .await
        .expect("should resume instance");

    assert_eq!(restored_instance.id, instance.id);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "First message");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Second message");
}
