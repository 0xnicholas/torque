mod common;

use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::agent_instance::AgentInstanceCreate;
use torque_harness::models::v1::artifact::ArtifactScope;
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, ArtifactRepository,
    PostgresAgentDefinitionRepository, PostgresAgentInstanceRepository, PostgresArtifactRepository,
};
use common::setup_test_db_or_skip;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_v1_agent_definition_lifecycle() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresAgentDefinitionRepository::new(db);

    let created = repo
        .create(&AgentDefinitionCreate {
            name: "Test Agent".into(),
            description: Some("A test agent".into()),
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create agent definition");

    assert_eq!(created.name, "Test Agent");

    let fetched = repo.get(created.id).await.expect("get agent definition");
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().name, "Test Agent");

    let deleted = repo
        .delete(created.id)
        .await
        .expect("delete agent definition");
    assert!(deleted);

    let not_found = repo.get(created.id).await.expect("get after delete");
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_v1_agent_instance_lifecycle() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let def_repo = PostgresAgentDefinitionRepository::new(db.clone());
    let inst_repo = PostgresAgentInstanceRepository::new(db);

    let definition = def_repo
        .create(&AgentDefinitionCreate {
            name: "Test Agent".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create agent definition");

    let instance = inst_repo
        .create(&AgentInstanceCreate {
            agent_definition_id: definition.id,
            external_context_refs: vec![],
        })
        .await
        .expect("create agent instance");

    assert_eq!(instance.agent_definition_id, definition.id);

    let fetched = inst_repo
        .get(instance.id)
        .await
        .expect("get agent instance");
    assert!(fetched.is_some());

    inst_repo
        .delete(instance.id)
        .await
        .expect("delete agent instance");
    def_repo
        .delete(definition.id)
        .await
        .expect("delete agent definition");
}

#[tokio::test]
#[serial]
async fn test_v1_artifact_lifecycle() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresArtifactRepository::new(db);

    let artifact = repo
        .create(
            "text",
            ArtifactScope::Private,
            "text/plain",
            serde_json::json!({"text": "Hello, world!"}),
        )
        .await
        .expect("create artifact");

    assert_eq!(artifact.kind, "text");
    assert_eq!(artifact.mime_type, "text/plain");

    let fetched = repo.get(artifact.id).await.expect("get artifact");
    assert!(fetched.is_some());

    let published = repo
        .update_scope(artifact.id, ArtifactScope::TeamShared)
        .await
        .expect("publish artifact");
    assert!(published);

    repo.delete(artifact.id).await.expect("delete artifact");
}

#[tokio::test]
#[serial]
async fn test_v1_list_pagination() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresAgentDefinitionRepository::new(db);

    for i in 0..5 {
        repo.create(&AgentDefinitionCreate {
            name: format!("Agent {}", i),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create agent definition");
    }

    let list = repo
        .list(3, None, None)
        .await
        .expect("list agent definitions");
    assert_eq!(list.len(), 3);

    let all = repo
        .list(10, None, None)
        .await
        .expect("list all agent definitions");
    assert_eq!(all.len(), 5);
}
