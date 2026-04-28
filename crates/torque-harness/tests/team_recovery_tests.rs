use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use torque_harness::models::v1::agent_instance::{
    AgentInstance, AgentInstanceCreate, AgentInstanceStatus,
};
use torque_harness::models::v1::checkpoint::Checkpoint;
use torque_harness::models::v1::event::Event;
use torque_harness::models::v1::team::{
    TeamMember, TeamRecoveryAction, TeamRecoveryDisposition, TeamTask, TeamTaskStatus,
};
use torque_harness::repository::{
    AgentInstanceRepository, CheckpointRepositoryExt, EventRepositoryExt, TeamMemberRepository,
    TeamTaskRepository,
};
use torque_harness::service::recovery::RecoveryService;
use torque_runtime::checkpoint::Message;
use uuid::Uuid;

struct MockAgentInstanceRepository;

impl MockAgentInstanceRepository {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentInstanceRepository for MockAgentInstanceRepository {
    async fn create(&self, _req: &AgentInstanceCreate) -> anyhow::Result<AgentInstance> {
        Ok(AgentInstance {
            id: Uuid::new_v4(),
            agent_definition_id: Uuid::new_v4(),
            status: AgentInstanceStatus::Created,
            external_context_refs: serde_json::Value::Array(vec![]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            checkpoint_id: None,
            current_task_id: None,
        })
    }

    async fn list(&self, _limit: i64) -> anyhow::Result<Vec<AgentInstance>> {
        Ok(vec![])
    }

    async fn get(&self, _id: Uuid) -> anyhow::Result<Option<AgentInstance>> {
        Ok(None)
    }

    async fn get_many(&self, _ids: &[Uuid]) -> anyhow::Result<Vec<AgentInstance>> {
        Ok(vec![])
    }

    async fn delete(&self, _id: Uuid) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn update_status(&self, _id: Uuid, _status: AgentInstanceStatus) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn update_current_task(&self, _id: Uuid, _task_id: Option<Uuid>) -> anyhow::Result<bool> {
        Ok(true)
    }
}

struct MockCheckpointRepository;

impl MockCheckpointRepository {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CheckpointRepositoryExt for MockCheckpointRepository {
    async fn list(&self, _limit: i64) -> anyhow::Result<Vec<Checkpoint>> {
        Ok(vec![])
    }

    async fn get(&self, _id: Uuid) -> anyhow::Result<Option<Checkpoint>> {
        Ok(None)
    }

    async fn list_by_instance(
        &self,
        _instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Checkpoint>> {
        Ok(vec![])
    }

    async fn get_messages(
        &self,
        _checkpoint_id: Uuid,
    ) -> anyhow::Result<Vec<Message>> {
        Ok(vec![])
    }
}

struct MockEventRepository;

impl MockEventRepository {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventRepositoryExt for MockEventRepository {
    async fn list_by_types(
        &self,
        _entity_type: &str,
        _entity_id: Uuid,
        _event_types: &[String],
        _limit: i64,
    ) -> anyhow::Result<Vec<Event>> {
        Ok(vec![])
    }

    async fn list(&self, _limit: i64) -> anyhow::Result<Vec<Event>> {
        Ok(vec![])
    }

    async fn list_after(&self, _event_id: Uuid) -> anyhow::Result<Vec<Event>> {
        Ok(vec![])
    }
}

struct MockTeamMemberRepository {
    members: Mutex<Vec<TeamMember>>,
}

impl MockTeamMemberRepository {
    fn new(members: Vec<TeamMember>) -> Self {
        Self {
            members: Mutex::new(members),
        }
    }
}

#[async_trait]
impl TeamMemberRepository for MockTeamMemberRepository {
    async fn create(
        &self,
        _team_instance_id: Uuid,
        _agent_instance_id: Uuid,
        _role: &str,
    ) -> anyhow::Result<TeamMember> {
        unimplemented!()
    }

    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<TeamMember>> {
        let members = self.members.lock().unwrap();
        Ok(members
            .iter()
            .filter(|m| m.team_instance_id == team_instance_id)
            .cloned()
            .collect())
    }

    async fn remove(
        &self,
        _team_instance_id: Uuid,
        _agent_instance_id: Uuid,
    ) -> anyhow::Result<bool> {
        unimplemented!()
    }
}

struct MockTeamTaskRepository {
    tasks: Mutex<std::collections::HashMap<Uuid, TeamTask>>,
}

impl MockTeamTaskRepository {
    fn new() -> Self {
        Self {
            tasks: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn add_task(&self, task: TeamTask) {
        self.tasks.lock().unwrap().insert(task.id, task);
    }
}

#[async_trait]
impl TeamTaskRepository for MockTeamTaskRepository {
    async fn create(
        &self,
        _team_instance_id: Uuid,
        _goal: &str,
        _instructions: Option<&str>,
        _input_artifacts: &[Uuid],
        _parent_task_id: Option<Uuid>,
        _idempotency_key: Option<&str>,
    ) -> anyhow::Result<TeamTask> {
        unimplemented!()
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamTask>> {
        Ok(self.tasks.lock().unwrap().get(&id).cloned())
    }

    async fn get_by_idempotency_key(
        &self,
        _team_instance_id: Uuid,
        _idempotency_key: &str,
    ) -> anyhow::Result<Option<TeamTask>> {
        unimplemented!()
    }

    async fn list_by_team(
        &self,
        _team_instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<TeamTask>> {
        Ok(self.tasks.lock().unwrap().values().cloned().collect())
    }

    async fn list_open(
        &self,
        _team_instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<TeamTask>> {
        Ok(self
            .tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| matches!(t.status, TeamTaskStatus::Open | TeamTaskStatus::InProgress))
            .cloned()
            .collect())
    }

    async fn update_status(&self, id: Uuid, status: TeamTaskStatus) -> anyhow::Result<bool> {
        if let Some(task) = self.tasks.lock().unwrap().get_mut(&id) {
            task.status = status;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn update_triage_result(
        &self,
        _id: Uuid,
        _triage: &torque_harness::models::v1::team::TriageResult,
    ) -> anyhow::Result<bool> {
        unimplemented!()
    }

    async fn update_mode(&self, _id: Uuid, _mode: &str) -> anyhow::Result<bool> {
        unimplemented!()
    }

    async fn mark_completed(&self, _id: Uuid) -> anyhow::Result<bool> {
        unimplemented!()
    }

    async fn update_retry_count(&self, id: Uuid, retry_count: u32) -> anyhow::Result<bool> {
        if let Some(task) = self.tasks.lock().unwrap().get_mut(&id) {
            task.retry_count = retry_count;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn create_test_member(id: Uuid, team_instance_id: Uuid, status: &str) -> TeamMember {
    TeamMember {
        id,
        team_instance_id,
        agent_instance_id: Uuid::new_v4(),
        role: "member".to_string(),
        status: status.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_team_recovery_assessment_healthy() {
    let team_id = Uuid::new_v4();
    let members = vec![
        create_test_member(Uuid::new_v4(), team_id, "Active"),
        create_test_member(Uuid::new_v4(), team_id, "Active"),
    ];
    let member_repo = Arc::new(MockTeamMemberRepository::new(members));
    let task_repo = Arc::new(MockTeamTaskRepository::new());

    let service = RecoveryService::new(
        Arc::new(MockAgentInstanceRepository::new()),
        Arc::new(MockCheckpointRepository::new()),
        Arc::new(MockEventRepository::new()),
    )
    .with_team_repos(member_repo.clone(), task_repo.clone());

    let assessment = service.assess_team_recovery(team_id).await.unwrap();

    assert!(matches!(
        assessment.disposition,
        TeamRecoveryDisposition::TeamHealthy
    ));
    assert!(assessment.failed_member_ids.is_empty());
}

#[tokio::test]
async fn test_team_recovery_assessment_degraded() {
    let team_id = Uuid::new_v4();
    let failed_member_id = Uuid::new_v4();
    let members = vec![
        create_test_member(failed_member_id, team_id, "Failed"),
        create_test_member(Uuid::new_v4(), team_id, "Active"),
    ];
    let member_repo = Arc::new(MockTeamMemberRepository::new(members));
    let task_repo = Arc::new(MockTeamTaskRepository::new());

    let service = RecoveryService::new(
        Arc::new(MockAgentInstanceRepository::new()),
        Arc::new(MockCheckpointRepository::new()),
        Arc::new(MockEventRepository::new()),
    )
    .with_team_repos(member_repo.clone(), task_repo.clone());

    let assessment = service.assess_team_recovery(team_id).await.unwrap();

    assert!(matches!(
        assessment.disposition,
        TeamRecoveryDisposition::TeamDegraded
    ));
    assert_eq!(assessment.failed_member_ids.len(), 1);
    assert_eq!(assessment.failed_member_ids[0], failed_member_id);
}

#[tokio::test]
async fn test_team_recovery_retry_on_failure() {
    let task_id = Uuid::new_v4();
    let team_id = Uuid::new_v4();

    let task = TeamTask {
        id: task_id,
        team_instance_id: team_id,
        goal: "Test task".to_string(),
        instructions: None,
        status: TeamTaskStatus::Failed,
        triage_result: None,
        mode_selected: None,
        input_artifacts: vec![],
        parent_task_id: None,
        idempotency_key: None,
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
        retry_count: 0,
    };

    let task_repo = Arc::new(MockTeamTaskRepository::new());
    task_repo.add_task(task);

    let member_repo = Arc::new(MockTeamMemberRepository::new(vec![]));

    let service = RecoveryService::new(
        Arc::new(MockAgentInstanceRepository::new()),
        Arc::new(MockCheckpointRepository::new()),
        Arc::new(MockEventRepository::new()),
    )
    .with_team_repos(member_repo.clone(), task_repo.clone());

    let result = service.recover_team_task(task_id).await.unwrap();

    assert!(matches!(result.action_taken, TeamRecoveryAction::Retry));
    assert!(matches!(result.new_status, TeamTaskStatus::Open));
}
