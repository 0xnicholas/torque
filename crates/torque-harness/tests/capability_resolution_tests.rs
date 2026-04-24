use serial_test::serial;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use torque_harness::db::Database;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::capability::{
    CapabilityProfileCreate, CapabilityRegistryBindingCreate,
    QualityTier, RiskLevel,
};
use torque_harness::repository::{
    AgentDefinitionRepository, PostgresAgentDefinitionRepository,
    PostgresCapabilityProfileRepository, PostgresCapabilityRegistryBindingRepository,
};
use torque_harness::service::CapabilityService;
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
async fn test_capability_resolve_by_ref() {
    let Some(db) = setup_test_db().await else { return; };

    let profile_repo = Arc::new(PostgresCapabilityProfileRepository::new(db.clone()));
    let binding_repo = Arc::new(PostgresCapabilityRegistryBindingRepository::new(db.clone()));
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let service = CapabilityService::new(profile_repo.clone(), binding_repo.clone());

    let unique_name = format!("test.resolution.{}", Uuid::new_v4());
    let agent_name = format!("test-agent.{}", Uuid::new_v4());

    let def = def_repo.create(&AgentDefinitionCreate {
        name: agent_name,
        description: None,
        system_prompt: None,
        tool_policy: serde_json::json!({}),
        memory_policy: serde_json::json!({}),
        delegation_policy: serde_json::json!({}),
        limits: serde_json::json!({}),
        default_model_policy: serde_json::json!({}),
    }).await.unwrap();

    let profile = service.create_profile(CapabilityProfileCreate {
        name: unique_name.clone(),
        description: Some("Test capability".to_string()),
        input_contract: None,
        output_contract: None,
        risk_level: RiskLevel::Low,
        default_agent_definition_id: None,
    }).await.unwrap();

    service.create_binding(CapabilityRegistryBindingCreate {
        capability_profile_id: profile.id,
        agent_definition_id: def.id,
        compatibility_score: Some(0.9),
        quality_tier: QualityTier::Production,
        metadata: None,
    }).await.unwrap();

    let resolution = service.resolve_by_ref(&unique_name, None).await.unwrap();

    assert_eq!(resolution.capability_ref, unique_name);
    assert_eq!(resolution.capability_profile_id, profile.id);
    assert_eq!(resolution.candidates.len(), 1);
    assert_eq!(resolution.candidates[0].agent_definition_id, def.id);
    assert_eq!(resolution.candidates[0].compatibility_score, Some(0.9));
}

#[tokio::test]
#[serial]
async fn test_capability_resolve_not_found() {
    let Some(db) = setup_test_db().await else { return; };

    let profile_repo = Arc::new(PostgresCapabilityProfileRepository::new(db.clone()));
    let binding_repo = Arc::new(PostgresCapabilityRegistryBindingRepository::new(db.clone()));
    let service = CapabilityService::new(profile_repo.clone(), binding_repo.clone());

    let result = service.resolve_by_ref("nonexistent.capability", None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
#[serial]
async fn test_capability_resolve_multiple_bindings() {
    let Some(db) = setup_test_db().await else { return; };

    let profile_repo = Arc::new(PostgresCapabilityProfileRepository::new(db.clone()));
    let binding_repo = Arc::new(PostgresCapabilityRegistryBindingRepository::new(db.clone()));
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let service = CapabilityService::new(profile_repo.clone(), binding_repo.clone());

    let unique_agent1 = format!("test-agent-1.{}", Uuid::new_v4());
    let unique_agent2 = format!("test-agent-2.{}", Uuid::new_v4());
    let unique_profile = format!("multi.binding.{}", Uuid::new_v4());

    let def1 = def_repo.create(&AgentDefinitionCreate {
        name: unique_agent1,
        description: None,
        system_prompt: None,
        tool_policy: serde_json::json!({}),
        memory_policy: serde_json::json!({}),
        delegation_policy: serde_json::json!({}),
        limits: serde_json::json!({}),
        default_model_policy: serde_json::json!({}),
    }).await.unwrap();

    let def2 = def_repo.create(&AgentDefinitionCreate {
        name: unique_agent2,
        description: None,
        system_prompt: None,
        tool_policy: serde_json::json!({}),
        memory_policy: serde_json::json!({}),
        delegation_policy: serde_json::json!({}),
        limits: serde_json::json!({}),
        default_model_policy: serde_json::json!({}),
    }).await.unwrap();

    let profile = service.create_profile(CapabilityProfileCreate {
        name: unique_profile.clone(),
        description: Some("Multiple bindings".to_string()),
        input_contract: None,
        output_contract: None,
        risk_level: RiskLevel::Medium,
        default_agent_definition_id: None,
    }).await.unwrap();

    service.create_binding(CapabilityRegistryBindingCreate {
        capability_profile_id: profile.id,
        agent_definition_id: def1.id,
        compatibility_score: Some(0.8),
        quality_tier: QualityTier::Beta,
        metadata: None,
    }).await.unwrap();

    service.create_binding(CapabilityRegistryBindingCreate {
        capability_profile_id: profile.id,
        agent_definition_id: def2.id,
        compatibility_score: Some(0.95),
        quality_tier: QualityTier::Production,
        metadata: None,
    }).await.unwrap();

    let resolution = service.resolve_by_ref(&unique_profile, None).await.unwrap();

    assert_eq!(resolution.candidates.len(), 2);
    assert_eq!(resolution.candidates[0].agent_definition_id, def2.id);
    assert_eq!(resolution.candidates[0].compatibility_score, Some(0.95));
    assert_eq!(resolution.candidates[1].agent_definition_id, def1.id);
    assert_eq!(resolution.candidates[1].compatibility_score, Some(0.8));
}