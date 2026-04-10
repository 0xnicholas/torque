mod common;

use chrono::Utc;
use common::setup_test_db_or_skip;
use serial_test::serial;
use session_agent::models::{
    MemoryCandidate, MemoryCandidateStatus, MemoryEntry, MemoryEntryStatus, MemoryLayer,
};
use uuid::Uuid;

fn unique_project_scope() -> String {
    format!("memory-candidate-test-{}", Uuid::new_v4())
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_create_memory_candidate_persists_project_scope_and_status() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let project_scope = unique_project_scope();
    let candidate = MemoryCandidate {
        id: Uuid::new_v4(),
        project_scope: project_scope.clone(),
        layer: MemoryLayer::L0,
        proposed_fact: "Project uses project-scoped durable memory".to_string(),
        source_type: Some("user_statement".to_string()),
        source_ref: Some("session://abc123".to_string()),
        proposer: Some("user".to_string()),
        confidence: Some(0.95),
        status: MemoryCandidateStatus::Pending,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        accepted_at: None,
        rejected_at: None,
    };

    let saved = session_agent::db::memory_candidates::create(db.pool(), &candidate)
        .await
        .expect("candidate should be saved");

    assert_eq!(saved.id, candidate.id);
    assert_eq!(saved.project_scope, project_scope);
    assert_eq!(saved.proposed_fact, candidate.proposed_fact);
    assert!(matches!(saved.status, MemoryCandidateStatus::Pending));
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_list_memory_candidates_is_project_scoped() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let scope_one = unique_project_scope();
    let scope_two = unique_project_scope();

    let candidate_one = MemoryCandidate {
        id: Uuid::new_v4(),
        project_scope: scope_one.clone(),
        layer: MemoryLayer::L1,
        proposed_fact: "Scope one fact".to_string(),
        source_type: Some("artifact_summary".to_string()),
        source_ref: None,
        proposer: Some("runtime".to_string()),
        confidence: Some(0.8),
        status: MemoryCandidateStatus::Pending,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        accepted_at: None,
        rejected_at: None,
    };

    let candidate_two = MemoryCandidate {
        id: Uuid::new_v4(),
        project_scope: scope_two.clone(),
        layer: MemoryLayer::L1,
        proposed_fact: "Scope two fact".to_string(),
        source_type: Some("artifact_summary".to_string()),
        source_ref: None,
        proposer: Some("runtime".to_string()),
        confidence: Some(0.8),
        status: MemoryCandidateStatus::Pending,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        accepted_at: None,
        rejected_at: None,
    };

    session_agent::db::memory_candidates::create(db.pool(), &candidate_one)
        .await
        .expect("first candidate should save");
    session_agent::db::memory_candidates::create(db.pool(), &candidate_two)
        .await
        .expect("second candidate should save");

    let scoped =
        session_agent::db::memory_candidates::list_by_project_scope(db.pool(), &scope_one, 10, 0)
            .await
            .expect("candidates should list");

    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, candidate_one.id);
    assert_eq!(scoped[0].project_scope, scope_one);
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_create_memory_entry_persists_project_scope_and_status() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let project_scope = unique_project_scope();
    let candidate = MemoryCandidate::new(
        project_scope.clone(),
        MemoryLayer::L0,
        "Candidate for durable entry".to_string(),
    );
    let saved_candidate = session_agent::db::memory_candidates::create(db.pool(), &candidate)
        .await
        .expect("candidate should be saved");

    let entry = MemoryEntry {
        id: Uuid::new_v4(),
        project_scope: project_scope.clone(),
        layer: MemoryLayer::L0,
        content: "Durable project fact".to_string(),
        source_candidate_id: Some(saved_candidate.id),
        source_type: Some("candidate_acceptance".to_string()),
        source_ref: Some(format!("memory-candidate://{}", saved_candidate.id)),
        proposer: Some("supervisor".to_string()),
        status: MemoryEntryStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        invalidated_at: None,
    };

    let saved = session_agent::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("entry should be saved");

    assert_eq!(saved.id, entry.id);
    assert_eq!(saved.project_scope, project_scope);
    assert_eq!(saved.content, entry.content);
    assert!(matches!(saved.status, MemoryEntryStatus::Active));

    let scoped =
        session_agent::db::memory_entries::list_by_project_scope(db.pool(), &project_scope, 10, 0)
            .await
            .expect("entries should list");

    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, entry.id);
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_reject_cross_project_candidate_link_on_entry_create() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let candidate_scope = unique_project_scope();
    let entry_scope = unique_project_scope();

    let candidate = MemoryCandidate::new(
        candidate_scope,
        MemoryLayer::L0,
        "Candidate scoped to another project".to_string(),
    );
    let saved_candidate = session_agent::db::memory_candidates::create(db.pool(), &candidate)
        .await
        .expect("candidate should be saved");

    let entry = MemoryEntry {
        id: Uuid::new_v4(),
        project_scope: entry_scope,
        layer: MemoryLayer::L0,
        content: "Entry should not link across projects".to_string(),
        source_candidate_id: Some(saved_candidate.id),
        source_type: Some("candidate_acceptance".to_string()),
        source_ref: Some(format!("memory-candidate://{}", saved_candidate.id)),
        proposer: Some("supervisor".to_string()),
        status: MemoryEntryStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        invalidated_at: None,
    };

    let result = session_agent::db::memory_entries::create(db.pool(), &entry).await;

    assert!(result.is_err(), "cross-project candidate linkage should be rejected");
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_candidate_status_transition_clears_stale_timestamps() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let project_scope = unique_project_scope();
    let candidate = MemoryCandidate::new(
        project_scope.clone(),
        MemoryLayer::L1,
        "Candidate status timestamps should normalize".to_string(),
    );
    let saved_candidate = session_agent::db::memory_candidates::create(db.pool(), &candidate)
        .await
        .expect("candidate should be saved");

    let accepted = session_agent::db::memory_candidates::update_status(
        db.pool(),
        &project_scope,
        saved_candidate.id,
        MemoryCandidateStatus::Accepted,
    )
    .await
    .expect("accepted transition should succeed")
    .expect("candidate should exist");

    assert!(accepted.accepted_at.is_some());
    assert!(accepted.rejected_at.is_none());

    let rejected = session_agent::db::memory_candidates::update_status(
        db.pool(),
        &project_scope,
        saved_candidate.id,
        MemoryCandidateStatus::Rejected,
    )
    .await
    .expect("rejected transition should succeed")
    .expect("candidate should exist");

    assert!(rejected.accepted_at.is_none());
    assert!(rejected.rejected_at.is_some());

    let pending = session_agent::db::memory_candidates::update_status(
        db.pool(),
        &project_scope,
        saved_candidate.id,
        MemoryCandidateStatus::Pending,
    )
    .await
    .expect("pending transition should succeed")
    .expect("candidate should exist");

    assert!(pending.accepted_at.is_none());
    assert!(pending.rejected_at.is_none());
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_entry_status_transition_clears_invalidated_timestamp() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let project_scope = unique_project_scope();
    let entry = MemoryEntry::new(
        project_scope.clone(),
        MemoryLayer::L0,
        "Entry status timestamps should normalize".to_string(),
    );
    let saved_entry = session_agent::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("entry should be saved");

    let invalidated = session_agent::db::memory_entries::update_status(
        db.pool(),
        &project_scope,
        saved_entry.id,
        MemoryEntryStatus::Invalidated,
    )
    .await
    .expect("invalidated transition should succeed")
    .expect("entry should exist");

    assert!(invalidated.invalidated_at.is_some());

    let replaced = session_agent::db::memory_entries::update_status(
        db.pool(),
        &project_scope,
        saved_entry.id,
        MemoryEntryStatus::Replaced,
    )
    .await
    .expect("replaced transition should succeed")
    .expect("entry should exist");

    assert!(replaced.invalidated_at.is_none());

    let active = session_agent::db::memory_entries::update_status(
        db.pool(),
        &project_scope,
        saved_entry.id,
        MemoryEntryStatus::Active,
    )
    .await
    .expect("active transition should succeed")
    .expect("entry should exist");

    assert!(active.invalidated_at.is_none());
}
