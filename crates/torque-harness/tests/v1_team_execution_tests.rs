mod common;

use common::setup_test_db_or_skip;
use serial_test::serial;

use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::task::{TaskStatus, TaskType};
use torque_harness::models::v1::team::{TeamDefinitionCreate, TeamInstanceCreate};
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, PostgresAgentDefinitionRepository,
    PostgresAgentInstanceRepository, PostgresTaskRepository, PostgresTeamDefinitionRepository,
    PostgresTeamInstanceRepository, PostgresTeamMemberRepository, TaskRepository,
    TeamDefinitionRepository, TeamInstanceRepository, TeamMemberRepository,
};
use torque_harness::service::TeamService;
use std::sync::Arc;

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
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));

    // Setup TeamService
    let team_service = TeamService::new(
        team_def_repo.clone(),
        team_inst_repo.clone(),
        team_member_repo.clone(),
        task_repo.clone(),
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
            "Complete team objective",
            Some("Work together to achieve the goal"),
        )
        .await
        .expect("create team task");

    assert_eq!(task.goal, "Complete team objective");
    assert!(matches!(task.task_type, TaskType::TeamTask));
    assert!(matches!(task.status, TaskStatus::Created));

    // 5. List team tasks
    let tasks = team_service
        .list_team_tasks(team_instance.id, 10)
        .await
        .expect("list team tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, task.id);

    // Cleanup
    task_repo.cancel(task.id).await.expect("cancel task");
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
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));

    let team_service = TeamService::new(
        team_def_repo.clone(),
        team_inst_repo.clone(),
        team_member_repo.clone(),
        task_repo.clone(),
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
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));

    let team_service = TeamService::new(team_def_repo, team_inst_repo, team_member_repo, task_repo);

    let fake_team_id = uuid::Uuid::new_v4();
    let result = team_service
        .create_team_task(fake_team_id, "Test", None)
        .await;

    assert!(
        result.is_err(),
        "Should fail with nonexistent team instance"
    );
}
