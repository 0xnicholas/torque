use axum::Router;
use llm::{LlmClient, ProviderConfig};
use std::sync::Arc;

use crate::api;
use crate::db::Database;
use crate::embedding::OpenAIEmbeddingGenerator;
use crate::repository::RepositoryContainer;
use crate::service::ServiceContainer;

pub fn build_app(db: Database, llm: Arc<dyn LlmClient>) -> Router {
    // ─────────────────────
    // Build a best-effort ProviderConfig from env for embedding init
    let provider_config = ProviderConfig::from_env();
    let repos = RepositoryContainer {
        memory: Arc::new(crate::repository::PostgresMemoryRepository::new(db.clone())),
        event: Arc::new(crate::repository::PostgresEventRepository::new(db.clone())),
        checkpoint: Arc::new(crate::repository::PostgresCheckpointRepository::new(
            db.clone(),
        )),
        agent_definition: Arc::new(crate::repository::PostgresAgentDefinitionRepository::new(
            db.clone(),
        )),
        agent_instance: Arc::new(crate::repository::PostgresAgentInstanceRepository::new(
            db.clone(),
        )),
        task: Arc::new(crate::repository::PostgresTaskRepository::new(db.clone())),
        artifact: Arc::new(crate::repository::PostgresArtifactRepository::new(
            db.clone(),
        )),
        capability_profile: Arc::new(crate::repository::PostgresCapabilityProfileRepository::new(
            db.clone(),
        )),
        capability_binding: Arc::new(
            crate::repository::PostgresCapabilityRegistryBindingRepository::new(db.clone()),
        ),
        team_definition: Arc::new(crate::repository::PostgresTeamDefinitionRepository::new(
            db.clone(),
        )),
        team_instance: Arc::new(crate::repository::PostgresTeamInstanceRepository::new(
            db.clone(),
        )),
        team_member: Arc::new(crate::repository::PostgresTeamMemberRepository::new(
            db.clone(),
        )),
        team_task: Arc::new(crate::repository::PostgresTeamTaskRepository::new(
            db.clone(),
        )),
        team_shared_state: Arc::new(crate::repository::PostgresSharedTaskStateRepository::new(
            db.clone(),
        )),
        team_event: Arc::new(crate::repository::PostgresTeamEventRepository::new(
            db.clone(),
        )),
        delegation: Arc::new(crate::repository::PostgresDelegationRepository::new(
            db.clone(),
        )),
        approval: Arc::new(crate::repository::PostgresApprovalRepository::new(
            db.clone(),
        )),
        checkpoint_ext: Arc::new(crate::repository::PostgresCheckpointRepositoryExt::new(
            db.clone(),
        )),
        event_ext: Arc::new(crate::repository::PostgresEventRepositoryExt::new(
            db.clone(),
        )),
        ephemeral_log: Arc::new(crate::repository::PostgresEphemeralLogRepository::new(
            db.clone(),
        )),
        rule: Arc::new(crate::repository::PostgresRuleRepository::new(db.clone())),
        escalation: Arc::new(crate::repository::PostgresEscalationRepository::new(
            db.clone(),
        )),
        run: Arc::new(crate::repository::PostgresRunRepository::new(db.clone())),
        tool_policy: Arc::new(crate::repository::PostgresToolPolicyRepository::new(
            db.clone(),
        )),
    };

    let memory_v1 = Arc::new(crate::repository::PostgresMemoryRepositoryV1::new(
        db.clone(),
    ));

    let embedding = {
        let gen = OpenAIEmbeddingGenerator::from_provider_config(&provider_config);
        Some(Arc::new(gen) as Arc<dyn crate::embedding::EmbeddingGenerator>)
    };

    let checkpointer = Arc::new(crate::runtime::checkpoint::PostgresCheckpointer::new(db.clone()));
    let idempotency = Arc::new(crate::v1_guards::IdempotencyStore::new());
    let run_gate = Arc::new(crate::v1_guards::RunGate::new());
    let services = ServiceContainer::new(
        repos,
        memory_v1,
        checkpointer,
        llm.clone(),
        embedding,
        idempotency,
        run_gate,
    );

    api::router(db, llm, Arc::new(services))
}
