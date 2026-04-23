mod common;

use common::fake_llm::FakeLlm;
use common::setup_test_db_or_skip;
use serial_test::serial;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_harness::agent::StreamEvent;
use torque_harness::models::{MemoryEntry, MemoryEntryStatus, MemoryLayer};
use torque_harness::repository::{
    MemoryRepository, PostgresCheckpointRepository, PostgresEventRepository,
    PostgresMemoryRepository, PostgresMessageRepository, PostgresSessionRepository,
};
use torque_harness::service::{MemoryService, SessionService, ToolService};

async fn build_session_service(
    db: torque_harness::db::Database,
    llm: Arc<dyn llm::LlmClient>,
) -> SessionService {
    let session_repo = Arc::new(PostgresSessionRepository::new(db.clone()));
    let message_repo = Arc::new(PostgresMessageRepository::new(db.clone()));
    let event_repo = Arc::new(PostgresEventRepository::new(db.clone()));
    let checkpoint_repo = Arc::new(PostgresCheckpointRepository::new(db.clone()));
    let memory_repo = Arc::new(PostgresMemoryRepository::new(db.clone()));

    let tool = Arc::new(ToolService::new());
    let memory_v1_repo = Arc::new(torque_harness::repository::PostgresMemoryRepositoryV1::new(
        db.clone(),
    ));
    let memory = Arc::new(MemoryService::new(memory_repo, memory_v1_repo, None));

    let checkpointer = Arc::new(torque_harness::kernel_bridge::PostgresCheckpointer::new(
        db.clone(),
    ));

    SessionService::new(
        session_repo,
        message_repo,
        event_repo,
        checkpoint_repo,
        checkpointer,
        llm,
        tool,
        memory,
    )
}

#[tokio::test]
#[serial]
async fn memory_recall_tests_recall_for_prompt_filters_status_and_prefers_match() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresMemoryRepository::new(db.clone());
    let project_scope = format!("memory-recall-test-{}", uuid::Uuid::new_v4());

    let matching = repo
        .create_entry(&MemoryEntry::new(
            project_scope.clone(),
            MemoryLayer::L1,
            "Torque runtime stores durable project memory".to_string(),
        ))
        .await
        .expect("matching active entry should be created");

    let stale = repo
        .create_entry(&MemoryEntry::new(
            project_scope.clone(),
            MemoryLayer::L1,
            "Old stale memory about torque runtime".to_string(),
        ))
        .await
        .expect("stale entry should be created");

    repo.update_entry_status(&project_scope, stale.id, MemoryEntryStatus::Invalidated)
        .await
        .expect("stale memory should be invalidated");

    let non_matching = repo
        .create_entry(&MemoryEntry::new(
            project_scope.clone(),
            MemoryLayer::L0,
            "Unrelated note about deployment windows".to_string(),
        ))
        .await
        .expect("non matching active entry should be created");

    let recalled = repo
        .search_entries(&project_scope, "torque runtime memory", 10)
        .await
        .expect("recall query should succeed");

    assert_eq!(recalled.len(), 2);
    assert_eq!(
        recalled[0].id, matching.id,
        "matching entry should be ranked first"
    );
    assert!(recalled
        .iter()
        .all(|entry| entry.status == MemoryEntryStatus::Active));
    assert!(recalled.iter().any(|entry| entry.id == non_matching.id));
    assert!(recalled.iter().all(|entry| entry.id != stale.id));
}

#[tokio::test]
#[serial]
async fn memory_recall_tests_chat_injects_memory_slice_into_prompt() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let llm = Arc::new(FakeLlm::single_text("ack"));
    let svc = build_session_service(db.clone(), llm.clone()).await;

    let session = svc
        .create("runner-memory-key", "memory-chat-scope")
        .await
        .expect("session should be created");

    let memory_repo = PostgresMemoryRepository::new(db.clone());

    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("project memory entries should be cleaned");

    let active = memory_repo
        .create_entry(&MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L1,
            "Torque is instance-centric and context should stay narrow".to_string(),
        ))
        .await
        .expect("active memory entry should be created");

    let invalidated = memory_repo
        .create_entry(&MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L1,
            "This old rule should not be recalled".to_string(),
        ))
        .await
        .expect("second memory entry should be created");

    memory_repo
        .update_entry_status(
            &session.project_scope,
            invalidated.id,
            MemoryEntryStatus::Invalidated,
        )
        .await
        .expect("entry should be invalidated");

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(16);

    svc.chat(
        session.id,
        "runner-memory-key",
        "remind me what Torque says about context".to_string(),
        tx,
    )
    .await
    .expect("chat should complete");
    while rx.recv().await.is_some() {}

    let requests = llm.recorded_requests();
    let first = requests
        .first()
        .expect("chat should send at least one llm request");
    let user_messages: Vec<&str> = first
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .collect();

    assert!(
        user_messages
            .iter()
            .any(|text| text.contains("Project memory")),
        "prompt should contain a project memory section"
    );
    assert!(
        user_messages
            .iter()
            .any(|text| text.contains(&active.content)),
        "active memory should be injected into prompt"
    );
    assert!(
        user_messages
            .iter()
            .all(|text| !text.contains("old rule should not be recalled")),
        "invalidated memory must not be injected"
    );
}
