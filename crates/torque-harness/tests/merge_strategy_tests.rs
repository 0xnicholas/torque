use serde_json::json;
use torque_harness::models::v1::gating::MergeStrategy;
use torque_harness::models::v1::memory::{MemoryCategory, MemoryContent, MemoryEntry};
use torque_harness::service::merge_strategy::{
    AppendStrategy, KeepSeparateStrategy, MergeStrategyExecutor, MergeStrategyHandler,
    MergedMemoryEntry, WithProvenanceStrategy,
};

fn create_test_existing() -> MemoryEntry {
    MemoryEntry {
        id: uuid::Uuid::new_v4(),
        agent_instance_id: Some(uuid::Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::AgentProfileMemory,
        key: "test_key".to_string(),
        value: json!("original value"),
        source_candidate_id: None,
        superseded_by: None,
        embedding_model: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn create_test_candidate() -> MemoryContent {
    MemoryContent {
        category: MemoryCategory::AgentProfileMemory,
        key: "test_key".to_string(),
        value: json!("new value"),
    }
}

#[tokio::test]
async fn test_append_strategy_creates_array() {
    let strategy = AppendStrategy;
    let existing = create_test_existing();
    let candidate = create_test_candidate();

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    assert!(matches!(result.value, serde_json::Value::Array(arr) if arr.len() == 2));
    assert_eq!(result.key, "test_key");
}

#[tokio::test]
async fn test_append_strategy_deduplicates() {
    let strategy = AppendStrategy;
    let mut existing = create_test_existing();
    existing.value = json!(["value1", "value2"]);
    let mut candidate = create_test_candidate();
    candidate.value = json!("value1"); // duplicate

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    if let serde_json::Value::Array(arr) = result.value {
        assert_eq!(arr.len(), 2); // Should not add duplicate
    } else {
        panic!("Expected array");
    }
}

#[tokio::test]
async fn test_keep_separate_strategy() {
    let strategy = KeepSeparateStrategy;
    let existing = create_test_existing();
    let candidate = create_test_candidate();

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    assert!(result.value.get("_type").is_some());
    assert_eq!(result.value["_type"], "separate_entries");
}

#[tokio::test]
async fn test_with_provenance_strategy() {
    let strategy = WithProvenanceStrategy;
    let existing = create_test_existing();
    let candidate = create_test_candidate();

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    assert_eq!(result.provenance.len(), 1);
    assert_eq!(result.provenance[0].method, "merged");
}
