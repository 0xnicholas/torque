mod common;

use chrono::Utc;
use torque_harness::models::v1::team::{
    Blocker, Decision, DelegationStatusEntry, PublishedFact, TeamDefinitionCreate,
    TeamEvent, TeamEventType, TeamInstanceCreate, TeamTaskStatus,
};
use torque_harness::repository::{
    PostgresSharedTaskStateRepository, PostgresTeamDefinitionRepository,
    PostgresTeamEventRepository, PostgresTeamInstanceRepository, PostgresTeamMemberRepository,
    PostgresTeamTaskRepository, SharedTaskStateRepository, TeamDefinitionRepository,
    TeamEventRepository, TeamInstanceRepository, TeamMemberRepository, TeamTaskRepository,
};
use uuid::Uuid;

async fn setup_test_db() -> Option<
    (
        torque_harness::db::Database,
        PostgresTeamDefinitionRepository,
        PostgresTeamInstanceRepository,
        PostgresTeamMemberRepository,
        PostgresTeamTaskRepository,
        PostgresSharedTaskStateRepository,
        PostgresTeamEventRepository,
    ),
> {
    let db = common::setup_test_db_or_skip().await?;
    let team_def_repo = PostgresTeamDefinitionRepository::new(db.clone());
    let team_instance_repo = PostgresTeamInstanceRepository::new(db.clone());
    let team_member_repo = PostgresTeamMemberRepository::new(db.clone());
    let team_task_repo = PostgresTeamTaskRepository::new(db.clone());
    let shared_state_repo = PostgresSharedTaskStateRepository::new(db.clone());
    let team_event_repo = PostgresTeamEventRepository::new(db.clone());
    Some((
        db,
        team_def_repo,
        team_instance_repo,
        team_member_repo,
        team_task_repo,
        shared_state_repo,
        team_event_repo,
    ))
}

#[tokio::test]
async fn test_team_creation_flow() {
    let Some((_db, team_def_repo, team_instance_repo, team_member_repo, _, _, _)) =
        setup_test_db().await
    else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Test Team".to_string(),
        description: Some("A test team".to_string()),
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };

    let team_def = team_def_repo.create(&def_create).await.unwrap();
    assert_eq!(team_def.name, "Test Team");
    assert_eq!(team_def.description.as_deref(), Some("A test team"));

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();
    assert_eq!(team_instance.team_definition_id, team_def.id);
    assert_eq!(team_instance.status, "ACTIVE");

    let member1 = team_member_repo
        .create(team_instance.id, Uuid::new_v4(), "writer")
        .await
        .unwrap();
    assert_eq!(member1.role, "writer");

    let member2 = team_member_repo
        .create(team_instance.id, Uuid::new_v4(), "reviewer")
        .await
        .unwrap();
    assert_eq!(member2.role, "reviewer");

    let members = team_member_repo
        .list_by_team(team_instance.id, 10)
        .await
        .unwrap();
    assert_eq!(members.len(), 2);

    let listed_defs = team_def_repo.list(10).await.unwrap();
    assert!(!listed_defs.is_empty());
}

#[tokio::test]
async fn test_delegation_flow() {
    let Some((_db, team_def_repo, team_instance_repo, team_member_repo, team_task_repo, _, _)) =
        setup_test_db().await
    else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Delegation Test Team".to_string(),
        description: None,
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };
    let team_def = team_def_repo.create(&def_create).await.unwrap();

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();

    let _member = team_member_repo
        .create(team_instance.id, Uuid::new_v4(), "developer")
        .await
        .unwrap();

    let task = team_task_repo
        .create(
            team_instance.id,
            "Implement feature X",
            Some("Build the feature according to spec"),
            &[],
            None,
            Some("feature-x-123"),
        )
        .await
        .unwrap();
    assert_eq!(task.goal, "Implement feature X");
    assert_eq!(task.status, TeamTaskStatus::Open);

    let fetched_task = team_task_repo.get(task.id).await.unwrap();
    assert!(fetched_task.is_some());
    assert_eq!(fetched_task.unwrap().goal, "Implement feature X");

    let tasks = team_task_repo.list_by_team(team_instance.id, 10).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, TeamTaskStatus::Open);

    let open_tasks = team_task_repo.list_open(team_instance.id, 10).await.unwrap();
    assert_eq!(open_tasks.len(), 1);
}

#[tokio::test]
async fn test_task_completion_flow() {
    let Some((_db, _, _, _, team_task_repo, _, _)) = setup_test_db().await else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Completion Test Team".to_string(),
        description: None,
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };
    let team_def_repo = PostgresTeamDefinitionRepository::new(common::setup_test_db_or_skip().await.unwrap());
    let team_def = team_def_repo.create(&def_create).await.unwrap();

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance_repo = PostgresTeamInstanceRepository::new(common::setup_test_db_or_skip().await.unwrap());
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();

    let task = team_task_repo
        .create(
            team_instance.id,
            "Complete this task",
            None,
            &[],
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(task.status, TeamTaskStatus::Open);

    let updated = team_task_repo
        .update_status(task.id, TeamTaskStatus::InProgress)
        .await
        .unwrap();
    assert!(updated);

    let updated_task = team_task_repo.get(task.id).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TeamTaskStatus::InProgress);

    let completed = team_task_repo.mark_completed(task.id).await.unwrap();
    assert!(completed);

    let completed_task = team_task_repo.get(task.id).await.unwrap().unwrap();
    assert!(completed_task.completed_at.is_some());
}

#[tokio::test]
async fn test_shared_state_flow() {
    let Some((_db, team_def_repo, team_instance_repo, _, team_task_repo, shared_state_repo, _)) =
        setup_test_db().await
    else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Shared State Test Team".to_string(),
        description: None,
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };
    let team_def = team_def_repo.create(&def_create).await.unwrap();

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();

    let shared_state = shared_state_repo.get_or_create(team_instance.id).await.unwrap();
    assert_eq!(shared_state.team_instance_id, team_instance.id);
    assert!(shared_state.accepted_artifact_refs.is_empty());
    assert!(shared_state.published_facts.is_empty());

    let artifact_ref = torque_harness::models::v1::team::ArtifactRef {
        artifact_id: Uuid::new_v4(),
        scope: torque_harness::models::v1::team::PublishScope::TeamShared,
        published_by: "test-agent".to_string(),
        published_at: Utc::now(),
    };
    let added = shared_state_repo
        .add_accepted_artifact(team_instance.id, artifact_ref.clone())
        .await
        .unwrap();
    assert!(added);

    let fact = PublishedFact {
        key: "design_doc".to_string(),
        value: serde_json::json!({"title": "Architecture Design"}),
        published_by: "architect".to_string(),
        published_at: Utc::now(),
    };
    let fact_added = shared_state_repo
        .add_published_fact(team_instance.id, fact)
        .await
        .unwrap();
    assert!(fact_added);

    let delegation_entry = DelegationStatusEntry {
        delegation_id: Uuid::new_v4(),
        status: "PENDING".to_string(),
        updated_at: Utc::now(),
    };
    let delegation_updated = shared_state_repo
        .update_delegation_status(team_instance.id, delegation_entry)
        .await
        .unwrap();
    assert!(delegation_updated);

    let updated_state = shared_state_repo.get(team_instance.id).await.unwrap().unwrap();
    assert_eq!(updated_state.accepted_artifact_refs.len(), 1);
    assert_eq!(updated_state.published_facts.len(), 1);
    assert_eq!(updated_state.delegation_status.len(), 1);

    let _task = team_task_repo
        .create(
            team_instance.id,
            "Task with artifacts",
            None,
            &[artifact_ref.artifact_id],
            None,
            None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_blocker_flow() {
    let Some((_db, team_def_repo, team_instance_repo, _, _, shared_state_repo, team_event_repo)) =
        setup_test_db().await
    else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Blocker Test Team".to_string(),
        description: None,
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };
    let team_def = team_def_repo.create(&def_create).await.unwrap();

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();

    let _shared_state = shared_state_repo.get_or_create(team_instance.id).await.unwrap();

    let blocker_id = Uuid::new_v4();
    let blocker = Blocker {
        blocker_id,
        description: "Waiting for API approval".to_string(),
        source: "policy-engine".to_string(),
        created_at: Utc::now(),
    };
    let added = shared_state_repo
        .add_blocker(team_instance.id, blocker.clone())
        .await
        .unwrap();
    assert!(added);

    let state_with_blocker = shared_state_repo.get(team_instance.id).await.unwrap().unwrap();
    assert_eq!(state_with_blocker.open_blockers.len(), 1);
    assert_eq!(state_with_blocker.open_blockers[0].blocker_id, blocker_id);

    let resolved = shared_state_repo
        .resolve_blocker(team_instance.id, blocker_id)
        .await
        .unwrap();
    assert!(resolved);

    let state_without_blocker = shared_state_repo.get(team_instance.id).await.unwrap().unwrap();
    assert!(state_without_blocker.open_blockers.is_empty());

    let event = TeamEvent {
        id: Uuid::new_v4(),
        team_instance_id: team_instance.id,
        event_type: TeamEventType::BlockerAdded.to_string(),
        timestamp: Utc::now(),
        actor_ref: "test-actor".to_string(),
        team_task_ref: None,
        related_instance_refs: vec![],
        related_artifact_refs: vec![],
        payload: serde_json::json!({"blocker_id": blocker_id.to_string()}),
        causal_event_refs: vec![],
    };
    let created_event = team_event_repo.create(&event).await.unwrap();
    assert_eq!(created_event.event_type, "BLOCKER_ADDED");

    let events = team_event_repo
        .list_by_team(team_instance.id, 10)
        .await
        .unwrap();
    assert!(!events.is_empty());
}

#[tokio::test]
async fn test_update_with_lock() {
    let Some((_db, team_def_repo, team_instance_repo, _, _, shared_state_repo, _)) =
        setup_test_db().await
    else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Lock Test Team".to_string(),
        description: None,
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };
    let team_def = team_def_repo.create(&def_create).await.unwrap();

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();

    let _ = shared_state_repo.get_or_create(team_instance.id).await.unwrap();

    use torque_harness::repository::team::SharedTaskStateUpdate;

    let blocker_id = Uuid::new_v4();
    let updates = vec![
        SharedTaskStateUpdate::AddBlocker(Blocker {
            blocker_id,
            description: "Test blocker".to_string(),
            source: "test".to_string(),
            created_at: Utc::now(),
        }),
        SharedTaskStateUpdate::AddFact(PublishedFact {
            key: "test_key".to_string(),
            value: serde_json::json!({"test": true}),
            published_by: "test".to_string(),
            published_at: Utc::now(),
        }),
        SharedTaskStateUpdate::AddDecision(Decision {
            decision_id: Uuid::new_v4(),
            description: "Approved".to_string(),
            decided_by: "test".to_string(),
            decided_at: Utc::now(),
        }),
    ];

    let updated_state = shared_state_repo
        .update_with_lock(team_instance.id, updates)
        .await
        .unwrap();

    assert_eq!(updated_state.open_blockers.len(), 1);
    assert_eq!(updated_state.published_facts.len(), 1);
    assert_eq!(updated_state.decisions.len(), 1);

    let resolved_updates = vec![SharedTaskStateUpdate::ResolveBlocker(blocker_id)];
    let after_resolve = shared_state_repo
        .update_with_lock(team_instance.id, resolved_updates)
        .await
        .unwrap();
    assert!(after_resolve.open_blockers.is_empty());
}

#[tokio::test]
async fn test_idempotency_key() {
    let Some((_db, team_def_repo, team_instance_repo, _, team_task_repo, _, _)) =
        setup_test_db().await
    else {
        return;
    };

    let def_create = TeamDefinitionCreate {
        name: "Idempotency Test Team".to_string(),
        description: None,
        supervisor_agent_definition_id: Uuid::new_v4(),
        sub_agents: vec![],
        policy: serde_json::json!({}),
    };
    let team_def = team_def_repo.create(&def_create).await.unwrap();

    let instance_create = TeamInstanceCreate {
        team_definition_id: team_def.id,
    };
    let team_instance = team_instance_repo.create(&instance_create).await.unwrap();

    let task1 = team_task_repo
        .create(
            team_instance.id,
            "Idempotent task",
            None,
            &[],
            None,
            Some("idem-key-001"),
        )
        .await
        .unwrap();

    let task2 = team_task_repo
        .create(
            team_instance.id,
            "Idempotent task",
            None,
            &[],
            None,
            Some("idem-key-001"),
        )
        .await
        .unwrap();

    assert_eq!(task1.id, task2.id);

    let existing = team_task_repo
        .get_by_idempotency_key(team_instance.id, "idem-key-001")
        .await
        .unwrap();
    assert!(existing.is_some());
    assert_eq!(existing.unwrap().id, task1.id);
}