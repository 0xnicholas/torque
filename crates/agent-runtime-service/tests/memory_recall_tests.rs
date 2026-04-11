mod common;

use common::fake_llm::FakeLlm;
use common::setup_test_db_or_skip;
use serial_test::serial;
use agent_runtime_service::agent::{AgentRunner, StreamEvent};
use agent_runtime_service::models::{MemoryEntry, MemoryEntryStatus, MemoryLayer, Message};
use agent_runtime_service::tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
#[serial]
async fn memory_recall_tests_recall_for_prompt_filters_status_and_prefers_match() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let project_scope = format!("memory-recall-test-{}", uuid::Uuid::new_v4());

    let matching = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            project_scope.clone(),
            MemoryLayer::L1,
            "Torque runtime stores durable project memory".to_string(),
        ),
    )
    .await
    .expect("matching active entry should be created");

    let stale = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            project_scope.clone(),
            MemoryLayer::L1,
            "Old stale memory about torque runtime".to_string(),
        ),
    )
    .await
    .expect("stale entry should be created");

    let _ = agent_runtime_service::db::memory_entries::update_status(
        db.pool(),
        &project_scope,
        stale.id,
        MemoryEntryStatus::Invalidated,
    )
    .await
    .expect("stale memory should be invalidated");

    let non_matching = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            project_scope.clone(),
            MemoryLayer::L0,
            "Unrelated note about deployment windows".to_string(),
        ),
    )
    .await
    .expect("non matching active entry should be created");

    let recalled = agent_runtime_service::db::memory_entries::recall_for_prompt(
        db.pool(),
        &project_scope,
        "torque runtime memory",
        10,
    )
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
async fn memory_recall_tests_runner_injects_memory_slice_into_prompt() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let session = agent_runtime_service::db::sessions::create(db.pool(), "runner-memory-key")
        .await
        .expect("session should be created");

    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("project memory entries should be cleaned");

    let active = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L1,
            "Torque is instance-centric and context should stay narrow".to_string(),
        ),
    )
    .await
    .expect("active memory entry should be created");

    let invalidated = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L1,
            "This old rule should not be recalled".to_string(),
        ),
    )
    .await
    .expect("second memory entry should be created");

    let _ = agent_runtime_service::db::memory_entries::update_status(
        db.pool(),
        &session.project_scope,
        invalidated.id,
        MemoryEntryStatus::Invalidated,
    )
    .await
    .expect("entry should be invalidated");

    let llm = Arc::new(FakeLlm::single_text("ack"));
    let tools = Arc::new(ToolRegistry::new());
    let runner = AgentRunner::new(llm.clone(), db.clone(), tools);
    let user_message = Message::user(
        session.id,
        "remind me what Torque says about context".to_string(),
    );
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(16);

    let _ = runner
        .run(&session, &user_message, tx)
        .await
        .expect("runner should complete");
    while rx.recv().await.is_some() {}

    let requests = llm.recorded_requests();
    let first = requests
        .first()
        .expect("runner should send at least one llm request");
    let system_messages: Vec<&str> = first
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect();

    assert!(
        system_messages
            .iter()
            .any(|text| text.contains("Project memory")),
        "prompt should contain a project memory section"
    );
    assert!(
        system_messages
            .iter()
            .any(|text| text.contains(&active.content)),
        "active memory should be injected into prompt"
    );
    assert!(
        system_messages
            .iter()
            .all(|text| !text.contains("old rule should not be recalled")),
        "invalidated memory must not be injected"
    );
}
