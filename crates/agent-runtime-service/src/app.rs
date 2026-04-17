use axum::Router;
use llm::OpenAiClient;
use std::sync::Arc;

use crate::api;
use crate::db::Database;
use crate::repository::RepositoryContainer;
use crate::service::ServiceContainer;

pub fn build_app(db: Database, llm: Arc<OpenAiClient>) -> Router {
    let repos = RepositoryContainer {
        session: Arc::new(crate::repository::PostgresSessionRepository::new(
            db.clone(),
        )),
        message: Arc::new(crate::repository::PostgresMessageRepository::new(
            db.clone(),
        )),
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
    };

    let checkpointer = Arc::new(crate::kernel_bridge::PostgresCheckpointer::new(db.clone()));
    let idempotency = Arc::new(crate::v1_guards::IdempotencyStore::new());
    let run_gate = Arc::new(crate::v1_guards::RunGate::new());
    let llm_dyn: Arc<dyn llm::LlmClient> = llm.clone();

    let services = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(ServiceContainer::new(
            repos,
            checkpointer,
            llm_dyn,
            idempotency,
            run_gate,
        ))
    });

    api::router(db, llm, Arc::new(services))
}
