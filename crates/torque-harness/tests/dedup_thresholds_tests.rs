use torque_harness::models::v1::gating::{DedupThresholds, GatingConfig};
use torque_harness::models::v1::memory::MemoryCategory;

#[test]
fn test_dedup_thresholds_with_env_override_duplicate() {
    std::env::set_var("MEMORY_DEDUP_AGENT_PROFILE_DUPLICATE", "0.98");

    let thresholds = DedupThresholds::for_category(&MemoryCategory::AgentProfileMemory)
        .with_env_override(&MemoryCategory::AgentProfileMemory);

    assert_eq!(thresholds.duplicate, 0.98);
    assert_eq!(thresholds.merge, 0.88); // unchanged

    std::env::remove_var("MEMORY_DEDUP_AGENT_PROFILE_DUPLICATE");
}

#[test]
fn test_dedup_thresholds_with_env_override_merge() {
    std::env::set_var("MEMORY_DEDUP_USER_PREFERENCE_MERGE", "0.92");

    let thresholds = DedupThresholds::for_category(&MemoryCategory::UserPreferenceMemory)
        .with_env_override(&MemoryCategory::UserPreferenceMemory);

    assert_eq!(thresholds.duplicate, 0.96); // unchanged
    assert_eq!(thresholds.merge, 0.92);

    std::env::remove_var("MEMORY_DEDUP_USER_PREFERENCE_MERGE");
}

#[test]
fn test_dedup_thresholds_default_values() {
    let thresholds = DedupThresholds::for_category(&MemoryCategory::AgentProfileMemory);
    assert_eq!(thresholds.duplicate, 0.96);
    assert_eq!(thresholds.merge, 0.88);
    assert_eq!(thresholds.minimum_content_length, 10);
}

#[test]
fn test_dedup_thresholds_from_config() {
    let mut config = GatingConfig::default();
    config.dedup_thresholds.insert(
        MemoryCategory::AgentProfileMemory,
        DedupThresholds {
            duplicate: 0.99,
            merge: 0.95,
            minimum_content_length: 5,
        },
    );

    let thresholds = DedupThresholds::from_config(&config, &MemoryCategory::AgentProfileMemory);
    assert_eq!(thresholds.duplicate, 0.99);
    assert_eq!(thresholds.merge, 0.95);
    assert_eq!(thresholds.minimum_content_length, 5);
}

#[test]
fn test_dedup_thresholds_category_defaults() {
    let task = DedupThresholds::for_category(&MemoryCategory::TaskOrDomainMemory);
    assert_eq!(task.duplicate, 0.95);
    assert_eq!(task.merge, 0.85);

    let episodic = DedupThresholds::for_category(&MemoryCategory::EpisodicMemory);
    assert_eq!(episodic.duplicate, 0.94);
    assert_eq!(episodic.merge, 0.85);
}
