mod common;

use common::setup_test_db_or_skip;
use serial_test::serial;

use std::sync::Arc;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::task::{TaskStatus, TaskType};
use torque_harness::models::v1::team::{
    TeamDefinitionCreate, TeamInstanceCreate, TeamTaskCreate, TeamTaskStatus,
};
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, DelegationRepository,
    PostgresAgentDefinitionRepository, PostgresAgentInstanceRepository,
    PostgresCapabilityProfileRepository, PostgresCapabilityRegistryBindingRepository,
    PostgresDelegationRepository, PostgresSharedTaskStateRepository,
    PostgresTeamDefinitionRepository, PostgresTeamEventRepository, PostgresTeamInstanceRepository,
    PostgresTeamMemberRepository, PostgresTeamTaskRepository, SharedTaskStateRepository,
    TeamDefinitionRepository, TeamEventRepository, TeamInstanceRepository, TeamMemberRepository,
    TeamTaskRepository,
};
use torque_harness::service::team::{
    SelectorResolver, SharedTaskStateManager, TeamEventEmitter, TeamSupervisor,
};
use torque_harness::service::TeamService;

#[tokio::test]
#[serial]
async fn test_team_task_lifecycle() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    // Setup repositories
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let team_def_repo = Arc::new(PostgresTeamDefinitionRepository::new(db.clone()));
    let team_inst_repo = Arc::new(PostgresTeamInstanceRepository::new(db.clone()));
    let team_member_repo = Arc::new(PostgresTeamMemberRepository::new(db.clone()));
    let team_task_repo = Arc::new(PostgresTeamTaskRepository::new(db.clone()));
    let shared_state_repo = Arc::new(PostgresSharedTaskStateRepository::new(db.clone()));
    let team_event_repo = Arc::new(PostgresTeamEventRepository::new(db.clone()));

    // Setup TeamService
    let team_service = TeamService::new(
        team_def_repo.clone(),
        team_inst_repo.clone(),
        team_member_repo.clone(),
        team_task_repo.clone(),
        shared_state_repo.clone(),
        team_event_repo.clone(),
    );

    // 1. Create supervisor agent definition
    let supervisor_def = def_repo
        .create(&AgentDefinitionCreate {
            name: "Supervisor Agent".into(),
            description: None,
            system_prompt: Some("You are a team supervisor.".into()),
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create supervisor definition");

    // 2. Create team definition
    let team_def = team_def_repo
        .create(&TeamDefinitionCreate {
            name: "Test Team".into(),
            description: Some("A test team".into()),
            supervisor_agent_definition_id: supervisor_def.id,
            sub_agents: vec![],
            policy: serde_json::json!({}),
        })
        .await
        .expect("create team definition");

    // 3. Create team instance
    let team_instance = team_inst_repo
        .create(&TeamInstanceCreate {
            team_definition_id: team_def.id,
        })
        .await
        .expect("create team instance");

    // 4. Create team task
    let task = team_service
        .create_team_task(
            team_instance.id,
            &TeamTaskCreate {
                goal: "Complete team objective".to_string(),
                instructions: Some("Work together to achieve the goal".to_string()),
                idempotency_key: None,
                input_artifacts: vec![],
                parent_task_id: None,
            },
        )
        .await
        .expect("create team task");

    assert_eq!(task.goal, "Complete team objective");
    assert!(matches!(
        task.status,
        torque_harness::models::v1::team::TeamTaskStatus::Open
    ));

    // 5. List team tasks
    let tasks = team_service
        .list_team_tasks(team_instance.id, 10)
        .await
        .expect("list team tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, task.id);

    // Cleanup
    team_task_repo
        .mark_completed(task.id)
        .await
        .expect("mark completed");
    team_inst_repo
        .delete(team_instance.id)
        .await
        .expect("delete team instance");
    team_def_repo
        .delete(team_def.id)
        .await
        .expect("delete team definition");
    def_repo
        .delete(supervisor_def.id)
        .await
        .expect("delete supervisor definition");
}

#[tokio::test]
#[serial]
async fn test_team_member_management() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let team_def_repo = Arc::new(PostgresTeamDefinitionRepository::new(db.clone()));
    let team_inst_repo = Arc::new(PostgresTeamInstanceRepository::new(db.clone()));
    let team_member_repo = Arc::new(PostgresTeamMemberRepository::new(db.clone()));
    let team_task_repo = Arc::new(PostgresTeamTaskRepository::new(db.clone()));
    let shared_state_repo = Arc::new(PostgresSharedTaskStateRepository::new(db.clone()));
    let team_event_repo = Arc::new(PostgresTeamEventRepository::new(db.clone()));

    let team_service = TeamService::new(
        team_def_repo.clone(),
        team_inst_repo.clone(),
        team_member_repo.clone(),
        team_task_repo.clone(),
        shared_state_repo.clone(),
        team_event_repo.clone(),
    );

    // Create definitions and instances
    let supervisor_def = def_repo
        .create(&AgentDefinitionCreate {
            name: "Supervisor".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create supervisor");

    let member_def = def_repo
        .create(&AgentDefinitionCreate {
            name: "Member Agent".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create member definition");

    let team_def = team_def_repo
        .create(&TeamDefinitionCreate {
            name: "Member Test Team".into(),
            description: None,
            supervisor_agent_definition_id: supervisor_def.id,
            sub_agents: vec![],
            policy: serde_json::json!({}),
        })
        .await
        .expect("create team definition");

    let team_instance = team_inst_repo
        .create(&TeamInstanceCreate {
            team_definition_id: team_def.id,
        })
        .await
        .expect("create team instance");

    let member_instance =
        torque_harness::repository::PostgresAgentInstanceRepository::new(db.clone())
            .create(
                &torque_harness::models::v1::agent_instance::AgentInstanceCreate {
                    agent_definition_id: member_def.id,
                    external_context_refs: vec![],
                },
            )
            .await
            .expect("create member instance");

    // Add member
    let member = team_service
        .add_member(team_instance.id, member_instance.id, "subagent")
        .await
        .expect("add member");

    assert_eq!(member.team_instance_id, team_instance.id);
    assert_eq!(member.agent_instance_id, member_instance.id);
    assert_eq!(member.role, "subagent");

    // List members
    let members = team_service
        .list_members(team_instance.id, 10)
        .await
        .expect("list members");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].id, member.id);

    // Remove member
    let removed = team_service
        .remove_member(team_instance.id, member_instance.id)
        .await
        .expect("remove member");
    assert!(removed);

    let members_after = team_service
        .list_members(team_instance.id, 10)
        .await
        .expect("list members after");
    assert!(members_after.is_empty());

    // Cleanup
    team_inst_repo
        .delete(team_instance.id)
        .await
        .expect("delete team instance");
    team_def_repo
        .delete(team_def.id)
        .await
        .expect("delete team definition");
    def_repo
        .delete(supervisor_def.id)
        .await
        .expect("delete supervisor");
    def_repo
        .delete(member_def.id)
        .await
        .expect("delete member def");
}

#[tokio::test]
#[serial]
async fn test_team_task_for_nonexistent_team_fails() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let team_def_repo = Arc::new(PostgresTeamDefinitionRepository::new(db.clone()));
    let team_inst_repo = Arc::new(PostgresTeamInstanceRepository::new(db.clone()));
    let team_member_repo = Arc::new(PostgresTeamMemberRepository::new(db.clone()));
    let team_task_repo = Arc::new(PostgresTeamTaskRepository::new(db.clone()));
    let shared_state_repo = Arc::new(PostgresSharedTaskStateRepository::new(db.clone()));
    let team_event_repo = Arc::new(PostgresTeamEventRepository::new(db.clone()));

    let team_service = TeamService::new(
        team_def_repo,
        team_inst_repo,
        team_member_repo,
        team_task_repo,
        shared_state_repo,
        team_event_repo,
    );

    let fake_team_id = uuid::Uuid::new_v4();
    let result = team_service
        .create_team_task(
            fake_team_id,
            &TeamTaskCreate {
                goal: "Test".to_string(),
                instructions: None,
                idempotency_key: None,
                input_artifacts: vec![],
                parent_task_id: None,
            },
        )
        .await;

    assert!(
        result.is_err(),
        "Should fail with nonexistent team instance"
    );
}

#[tokio::test]
#[serial]
async fn test_supervisor_poll_and_execute_with_route_mode() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let agent_inst_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let team_def_repo = Arc::new(PostgresTeamDefinitionRepository::new(db.clone()));
    let team_inst_repo = Arc::new(PostgresTeamInstanceRepository::new(db.clone()));
    let team_member_repo = Arc::new(PostgresTeamMemberRepository::new(db.clone()));
    let team_task_repo = Arc::new(PostgresTeamTaskRepository::new(db.clone()));
    let shared_state_repo = Arc::new(PostgresSharedTaskStateRepository::new(db.clone()));
    let team_event_repo = Arc::new(PostgresTeamEventRepository::new(db.clone()));
    let delegation_repo = Arc::new(PostgresDelegationRepository::new(db.clone()));
    let capability_profile_repo = Arc::new(PostgresCapabilityProfileRepository::new(db.clone()));
    let capability_binding_repo =
        Arc::new(PostgresCapabilityRegistryBindingRepository::new(db.clone()));

    let selector_resolver = SelectorResolver::new(
        team_member_repo.clone(),
        agent_inst_repo.clone(),
        capability_profile_repo,
        capability_binding_repo,
    );
    let shared_state_manager = SharedTaskStateManager::new(shared_state_repo.clone());
    let event_emitter = TeamEventEmitter::new(team_event_repo.clone());

    let supervisor = TeamSupervisor::new(
        team_task_repo.clone(),
        delegation_repo.clone(),
        Arc::new(selector_resolver),
        Arc::new(shared_state_manager),
        Arc::new(event_emitter),
    );

    let supervisor_def = def_repo
        .create(&AgentDefinitionCreate {
            name: "Supervisor Agent".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create supervisor definition");

    let member_def = def_repo
        .create(&AgentDefinitionCreate {
            name: "Member Agent".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create member definition");

    let team_def = team_def_repo
        .create(&TeamDefinitionCreate {
            name: "Supervisor Test Team".into(),
            description: None,
            supervisor_agent_definition_id: supervisor_def.id,
            sub_agents: vec![],
            policy: serde_json::json!({}),
        })
        .await
        .expect("create team definition");

    let team_instance = team_inst_repo
        .create(&TeamInstanceCreate {
            team_definition_id: team_def.id,
        })
        .await
        .expect("create team instance");

    let member_instance = agent_inst_repo
        .create(
            &torque_harness::models::v1::agent_instance::AgentInstanceCreate {
                agent_definition_id: member_def.id,
                external_context_refs: vec![],
            },
        )
        .await
        .expect("create member instance");

    team_member_repo
        .create(team_instance.id, member_instance.id, "worker")
        .await
        .expect("add team member");

    let task = team_task_repo
        .create(team_instance.id, "Simple task goal", None, &[], None, None)
        .await
        .expect("create team task");

    assert_eq!(task.status, TeamTaskStatus::Open);

    let result = supervisor.poll_and_execute(team_instance.id).await;
    assert!(result.is_ok(), "poll_and_execute should succeed");

    let result = result.unwrap();
    assert!(result.is_some(), "Should have executed a task");
    let exec_result = result.unwrap();
    assert!(
        exec_result.success,
        "Execution should succeed: {}",
        exec_result.summary
    );
    assert_eq!(exec_result.task_id, task.id);

    let updated_task = team_task_repo.get(task.id).await.unwrap().unwrap();
    assert!(
        matches!(
            updated_task.status,
            TeamTaskStatus::Completed | TeamTaskStatus::InProgress
        ),
        "Task should be completed or in progress, got {:?}",
        updated_task.status
    );

    let delegations = delegation_repo.list_by_task(task.id, 10).await.unwrap();
    assert!(
        !delegations.is_empty(),
        "Should have created at least one delegation"
    );

    team_task_repo
        .mark_completed(task.id)
        .await
        .expect("cleanup");
    team_member_repo
        .remove(team_instance.id, member_instance.id)
        .await
        .expect("cleanup");
    team_inst_repo
        .delete(team_instance.id)
        .await
        .expect("cleanup");
    team_def_repo.delete(team_def.id).await.expect("cleanup");
    def_repo.delete(supervisor_def.id).await.expect("cleanup");
    def_repo.delete(member_def.id).await.expect("cleanup");
}
