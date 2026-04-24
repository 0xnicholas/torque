mod common;

use common::setup_test_db_or_skip;
use serial_test::serial;
use std::sync::Arc;
use torque_harness::models::v1::memory::{CompactionStrategy, CompactionRecommendation, MemoryCategory, MemoryEntry};
use torque_harness::repository::{MemoryRepositoryV1, PostgresMemoryRepositoryV1};
use torque_harness::jobs::memory_compaction::MemoryCompactionJob;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn test_get_entries_by_ids() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = Arc::new(PostgresMemoryRepositoryV1::new(db.clone()));

    let entry1 = MemoryEntry {
        id: Uuid::new_v4(),
        agent_instance_id: Some(Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::EpisodicMemory,
        key: "test_key_1".to_string(),
        value: serde_json::json!("test_value_1"),
        source_candidate_id: None,
        superseded_by: None,
        embedding_model: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let entry2 = MemoryEntry {
        id: Uuid::new_v4(),
        agent_instance_id: Some(Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::TaskOrDomainMemory,
        key: "test_key_2".to_string(),
        value: serde_json::json!("test_value_2"),
        source_candidate_id: None,
        superseded_by: None,
        embedding_model: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let created1 = repo.create_entry(&entry1).await.expect("entry1 should be created");
    let created2 = repo.create_entry(&entry2).await.expect("entry2 should be created");

    let ids = vec![created1.id, created2.id];
    let fetched = repo.get_entries_by_ids(ids).await.expect("should fetch entries by ids");

    assert_eq!(fetched.len(), 2);
    assert!(fetched.iter().any(|e| e.id == created1.id));
    assert!(fetched.iter().any(|e| e.id == created2.id));
}

#[tokio::test]
#[serial]
async fn test_compaction_job_processes_entries() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = Arc::new(PostgresMemoryRepositoryV1::new(db.clone()));

    let old_date = chrono::Utc::now() - chrono::Duration::days(60);
    let entry1 = MemoryEntry {
        id: Uuid::new_v4(),
        agent_instance_id: Some(Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::EpisodicMemory,
        key: "old_key_1".to_string(),
        value: serde_json::json!("old_value_1"),
        source_candidate_id: None,
        superseded_by: None,
        embedding_model: None,
        access_count: 5,
        last_accessed_at: Some(old_date),
        created_at: old_date,
        updated_at: old_date,
    };

    let entry2 = MemoryEntry {
        id: Uuid::new_v4(),
        agent_instance_id: Some(Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::EpisodicMemory,
        key: "old_key_2".to_string(),
        value: serde_json::json!("old_value_2"),
        source_candidate_id: None,
        superseded_by: None,
        embedding_model: None,
        access_count: 3,
        last_accessed_at: Some(old_date),
        created_at: old_date,
        updated_at: old_date,
    };

    repo.create_entry(&entry1).await.expect("entry1 should be created");
    repo.create_entry(&entry2).await.expect("entry2 should be created");

    let job = MemoryCompactionJob::new(repo.clone(), None)
        .with_batch_size(10)
        .with_max_age_days(30);

    let recommendations = job.evaluate().await.expect("evaluate should work");
    assert_eq!(recommendations.len(), 1);

    let recommendation = &recommendations[0];
    assert_eq!(recommendation.entry_ids.len(), 2);
    assert!(recommendation.entry_ids.contains(&entry1.id));
    assert!(recommendation.entry_ids.contains(&entry2.id));
    assert!(recommendation.entry_id == entry1.id || recommendation.entry_id == entry2.id);
    assert_eq!(recommendation.strategy, CompactionStrategy::Archive);
    assert!(recommendation.supersedes.is_none());

    let result = job.run().await.expect("compaction job should run");

    assert_eq!(result.entries_processed, 1);
    assert_eq!(result.candidates_created, 1);
    assert_eq!(result.errors, 0);
}

#[test]
fn test_compaction_strategy_serialization() {
    let summarize = CompactionStrategy::Summarize;
    let merge = CompactionStrategy::Merge;
    let archive = CompactionStrategy::Archive;
    let drop = CompactionStrategy::Drop;

    let json_summarize = serde_json::to_string(&summarize).unwrap();
    let json_merge = serde_json::to_string(&merge).unwrap();
    let json_archive = serde_json::to_string(&archive).unwrap();
    let json_drop = serde_json::to_string(&drop).unwrap();

    assert!(json_summarize.contains("Summarize"));
    assert!(json_merge.contains("Merge"));
    assert!(json_archive.contains("Archive"));
    assert!(json_drop.contains("Drop"));
}

#[tokio::test]
async fn test_compaction_recommendation_struct() {
    let entry_id = Uuid::new_v4();
    let recommendation = CompactionRecommendation {
        entry_id,
        entry_ids: vec![entry_id],
        strategy: CompactionStrategy::Summarize,
        reason: "Test reason".to_string(),
        supersedes: Some(Uuid::new_v4()),
    };

    let json = serde_json::to_string(&recommendation).unwrap();
    assert!(json.contains("Summarize"));
    assert!(json.contains("Test reason"));
    assert!(json.contains("supersedes"));
}