mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::Utc;
use common::setup_test_db_or_skip;
use llm::OpenAiClient;
use serde_json::{json, Value};
use serial_test::serial;
use agent_runtime_service::db::Database;
use agent_runtime_service::models::{
    MemoryCandidate, MemoryCandidateStatus, MemoryEntry, MemoryEntryStatus, MemoryLayer,
};
use std::sync::Arc;
use tower::util::ServiceExt;
use uuid::Uuid;

fn unique_project_scope() -> String {
    format!("memory-candidate-test-{}", Uuid::new_v4())
}

async fn setup_app() -> Option<(Database, axum::Router)> {
    let Some(db) = setup_test_db_or_skip().await else {
        return None;
    };

    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));
    let app = agent_runtime_service::app::build_app(Database::new(db.pool().clone()), llm);

    Some((db, app))
}

async fn read_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");

    serde_json::from_slice(&body).expect("response should be valid json")
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

    let saved = agent_runtime_service::db::memory_candidates::create(db.pool(), &candidate)
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

    agent_runtime_service::db::memory_candidates::create(db.pool(), &candidate_one)
        .await
        .expect("first candidate should save");
    agent_runtime_service::db::memory_candidates::create(db.pool(), &candidate_two)
        .await
        .expect("second candidate should save");

    let scoped =
        agent_runtime_service::db::memory_candidates::list_by_project_scope(db.pool(), &scope_one, 10, 0)
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
    let saved_candidate = agent_runtime_service::db::memory_candidates::create(db.pool(), &candidate)
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

    let saved = agent_runtime_service::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("entry should be saved");

    assert_eq!(saved.id, entry.id);
    assert_eq!(saved.project_scope, project_scope);
    assert_eq!(saved.content, entry.content);
    assert!(matches!(saved.status, MemoryEntryStatus::Active));

    let scoped =
        agent_runtime_service::db::memory_entries::list_by_project_scope(db.pool(), &project_scope, 10, 0)
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
    let saved_candidate = agent_runtime_service::db::memory_candidates::create(db.pool(), &candidate)
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

    let result = agent_runtime_service::db::memory_entries::create(db.pool(), &entry).await;

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
    let saved_candidate = agent_runtime_service::db::memory_candidates::create(db.pool(), &candidate)
        .await
        .expect("candidate should be saved");

    let accepted = agent_runtime_service::db::memory_candidates::update_status(
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

    let rejected = agent_runtime_service::db::memory_candidates::update_status(
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

    let pending = agent_runtime_service::db::memory_candidates::update_status(
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
    let saved_entry = agent_runtime_service::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("entry should be saved");

    let invalidated = agent_runtime_service::db::memory_entries::update_status(
        db.pool(),
        &project_scope,
        saved_entry.id,
        MemoryEntryStatus::Invalidated,
    )
    .await
    .expect("invalidated transition should succeed")
    .expect("entry should exist");

    assert!(invalidated.invalidated_at.is_some());

    let replaced = agent_runtime_service::db::memory_entries::update_status(
        db.pool(),
        &project_scope,
        saved_entry.id,
        MemoryEntryStatus::Replaced,
    )
    .await
    .expect("replaced transition should succeed")
    .expect("entry should exist");

    assert!(replaced.invalidated_at.is_none());

    let active = agent_runtime_service::db::memory_entries::update_status(
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

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_accept_candidate_api_creates_durable_entry() {
    let Some((db, app)) = setup_app().await else {
        return;
    };

    let api_key = "test-api-key";
    let session = agent_runtime_service::db::sessions::create(db.pool(), api_key)
        .await
        .expect("session should be created");
    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("entries cleanup should succeed");
    sqlx::query("DELETE FROM memory_candidates WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("candidates cleanup should succeed");

    let create_candidate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/sessions/{}/memory/candidates", session.id))
                .header("x-api-key", api_key)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "layer": "L1",
                        "proposed_fact": "Accepted fact should become durable entry"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(create_candidate_response.status(), StatusCode::OK);
    let candidate_json = read_json(create_candidate_response).await;
    let candidate_id = Uuid::parse_str(
        candidate_json["id"]
            .as_str()
            .expect("candidate id should be string"),
    )
    .expect("candidate id should parse");

    let accept_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/sessions/{}/memory/candidates/{}/accept",
                    session.id, candidate_id
                ))
                .header("x-api-key", api_key)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(accept_response.status(), StatusCode::OK);
    let accept_json = read_json(accept_response).await;
    assert_eq!(accept_json["candidate"]["status"], "Accepted");
    assert_eq!(
        accept_json["entry"]["source_candidate_id"],
        Value::String(candidate_id.to_string())
    );

    let entries_after = agent_runtime_service::db::memory_entries::list_by_project_scope(
        db.pool(),
        &session.project_scope,
        100,
        0,
    )
    .await
    .expect("entries should list");
    assert!(
        entries_after
            .iter()
            .any(|entry| entry.source_candidate_id == Some(candidate_id)),
        "accepted candidate should produce a durable entry linked by source_candidate_id"
    );
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_list_memory_endpoint_returns_project_entries() {
    let Some((db, app)) = setup_app().await else {
        return;
    };

    let api_key = "test-api-key";
    let session = agent_runtime_service::db::sessions::create(db.pool(), api_key)
        .await
        .expect("session should be created");
    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("entries cleanup should succeed");

    let mut entry = MemoryEntry::new(
        session.project_scope.clone(),
        MemoryLayer::L0,
        "Durable fact visible from memory endpoint".to_string(),
    );
    entry.source_type = Some("manual_seed".to_string());
    agent_runtime_service::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("entry should be created");

    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/sessions/{}/memory", session.id))
                .header("x-api-key", api_key)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json = read_json(list_response).await;
    let entries = list_json.as_array().expect("response should be an array");
    assert!(
        entries.iter().any(|entry_json| {
            entry_json["content"]
                == Value::String("Durable fact visible from memory endpoint".to_string())
        }),
        "expected seeded entry to be present in memory endpoint response"
    );
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_replace_entry_marks_old_replaced_and_creates_new_entry() {
    let Some((db, app)) = setup_app().await else {
        return;
    };

    let api_key = "test-api-key";
    let session = agent_runtime_service::db::sessions::create(db.pool(), api_key)
        .await
        .expect("session should be created");
    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("entries cleanup should succeed");

    let entry = MemoryEntry::new(
        session.project_scope.clone(),
        MemoryLayer::L0,
        "Old durable fact".to_string(),
    );
    let old_entry = agent_runtime_service::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("seed entry should save");

    let replace_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/sessions/{}/memory/{}/replace",
                    session.id, old_entry.id
                ))
                .header("x-api-key", api_key)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "New durable fact",
                        "layer": "L1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(replace_response.status(), StatusCode::OK);
    let new_entry_json = read_json(replace_response).await;
    let new_entry_id = Uuid::parse_str(
        new_entry_json["id"]
            .as_str()
            .expect("new entry id should be string"),
    )
    .expect("new entry id should parse");
    assert_ne!(new_entry_id, old_entry.id);
    assert_eq!(
        new_entry_json["content"],
        Value::String("New durable fact".to_string())
    );

    let old_after =
        agent_runtime_service::db::memory_entries::get_by_id(db.pool(), &session.project_scope, old_entry.id)
            .await
            .expect("query should succeed")
            .expect("old entry should exist");
    assert!(matches!(old_after.status, MemoryEntryStatus::Replaced));

    let all_entries = agent_runtime_service::db::memory_entries::list_by_project_scope(
        db.pool(),
        &session.project_scope,
        100,
        0,
    )
    .await
    .expect("entries should list");
    assert!(
        all_entries.iter().any(|entry_row| entry_row.id == new_entry_id),
        "new replacement entry should exist in scoped listing"
    );
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_invalidate_entry_marks_entry_invalidated() {
    let Some((db, app)) = setup_app().await else {
        return;
    };

    let api_key = "test-api-key";
    let session = agent_runtime_service::db::sessions::create(db.pool(), api_key)
        .await
        .expect("session should be created");
    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("entries cleanup should succeed");

    let entry = MemoryEntry::new(
        session.project_scope.clone(),
        MemoryLayer::L0,
        "Fact to invalidate".to_string(),
    );
    let saved = agent_runtime_service::db::memory_entries::create(db.pool(), &entry)
        .await
        .expect("entry should save");

    let invalidate_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/sessions/{}/memory/{}/invalidate",
                    session.id, saved.id
                ))
                .header("x-api-key", api_key)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(invalidate_response.status(), StatusCode::OK);
    let invalidate_json = read_json(invalidate_response).await;
    assert_eq!(invalidate_json["status"], Value::String("Invalidated".to_string()));

    let stored = agent_runtime_service::db::memory_entries::get_by_id(db.pool(), &session.project_scope, saved.id)
        .await
        .expect("query should succeed")
        .expect("entry should exist");
    assert!(matches!(stored.status, MemoryEntryStatus::Invalidated));
    assert!(stored.invalidated_at.is_some());
}

#[tokio::test]
#[serial]
async fn memory_candidate_api_tests_search_memory_endpoint_returns_ranked_active_entries() {
    let Some((db, app)) = setup_app().await else {
        return;
    };

    let api_key = "test-api-key";
    let session = agent_runtime_service::db::sessions::create(db.pool(), api_key)
        .await
        .expect("session should be created");
    sqlx::query("DELETE FROM memory_entries WHERE project_scope = $1")
        .bind(&session.project_scope)
        .execute(db.pool())
        .await
        .expect("entries cleanup should succeed");

    let best = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L1,
            "Torque context memory is project scoped and durable".to_string(),
        ),
    )
    .await
    .expect("best entry should save");

    let weaker = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L0,
            "Torque runtime note".to_string(),
        ),
    )
    .await
    .expect("weaker entry should save");

    let invalidated = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            session.project_scope.clone(),
            MemoryLayer::L0,
            "Torque context memory stale record".to_string(),
        ),
    )
    .await
    .expect("invalidated entry should save");
    let _ = agent_runtime_service::db::memory_entries::update_status(
        db.pool(),
        &session.project_scope,
        invalidated.id,
        MemoryEntryStatus::Invalidated,
    )
    .await
    .expect("invalidating test entry should succeed");

    let other_scope = unique_project_scope();
    let _ = agent_runtime_service::db::memory_entries::create(
        db.pool(),
        &MemoryEntry::new(
            other_scope,
            MemoryLayer::L1,
            "Torque context memory from another scope".to_string(),
        ),
    )
    .await
    .expect("cross scope entry should save");

    let search_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/sessions/{}/memory/search?q=torque%20context%20memory&limit=10",
                    session.id
                ))
                .header("x-api-key", api_key)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(search_response.status(), StatusCode::OK);
    let search_json = read_json(search_response).await;
    let entries = search_json.as_array().expect("response should be an array");
    assert_eq!(entries.len(), 2, "should only include active entries in scope");
    assert_eq!(
        entries[0]["id"],
        Value::String(best.id.to_string()),
        "best matching entry should be first"
    );
    assert!(
        entries
            .iter()
            .any(|entry_json| entry_json["id"] == Value::String(weaker.id.to_string())),
        "weaker but active scoped entry should be returned"
    );
    assert!(
        entries
            .iter()
            .all(|entry_json| entry_json["id"] != Value::String(invalidated.id.to_string())),
        "invalidated entries should not appear in search results"
    );
}
