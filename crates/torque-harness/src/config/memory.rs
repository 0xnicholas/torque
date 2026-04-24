use std::collections::HashMap;

use crate::models::v1::gating::{CandidateGenerationConfig, DedupThresholds, GatingConfig};
use crate::models::v1::memory::MemoryCategory;

pub fn candidate_generation_config() -> CandidateGenerationConfig {
    CandidateGenerationConfig {
        enabled: std::env::var("MEMORY_CANDIDATE_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(true),
        extraction_model: std::env::var("MEMORY_EXTRACTION_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".to_string()),
        max_candidates_per_execution: std::env::var("MEMORY_MAX_CANDIDATES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
        min_content_length: std::env::var("MEMORY_MIN_CONTENT_LENGTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20),
        excluded_tools: std::env::var("MEMORY_EXCLUDED_TOOLS")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| vec!["echo".to_string(), "ping".to_string()]),
    }
}

pub fn gating_config() -> GatingConfig {
    let mut thresholds: HashMap<MemoryCategory, DedupThresholds> = HashMap::new();

    thresholds.insert(
        MemoryCategory::AgentProfileMemory,
        DedupThresholds {
            duplicate: 0.96,
            merge: 0.88,
            minimum_content_length: 10,
        },
    );
    thresholds.insert(
        MemoryCategory::UserPreferenceMemory,
        DedupThresholds {
            duplicate: 0.96,
            merge: 0.88,
            minimum_content_length: 10,
        },
    );
    thresholds.insert(
        MemoryCategory::TaskOrDomainMemory,
        DedupThresholds {
            duplicate: 0.95,
            merge: 0.85,
            minimum_content_length: 20,
        },
    );
    thresholds.insert(
        MemoryCategory::EpisodicMemory,
        DedupThresholds {
            duplicate: 0.94,
            merge: 0.85,
            minimum_content_length: 30,
        },
    );
    thresholds.insert(
        MemoryCategory::ExternalContextMemory,
        DedupThresholds {
            duplicate: 0.93,
            merge: 0.80,
            minimum_content_length: 5,
        },
    );

    for (category, thresholds) in thresholds.iter_mut() {
        *thresholds = thresholds.clone().with_env_override(category);
    }

    GatingConfig {
        auto_approve_quality_threshold: std::env::var("MEMORY_QUALITY_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.88),
        auto_approve_confidence_threshold: std::env::var("MEMORY_CONFIDENCE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.85),
        dedup_thresholds: thresholds,
    }
}

pub fn extraction_model() -> String {
    std::env::var("MEMORY_EXTRACTION_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string())
}

pub fn extraction_api_base() -> String {
    std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
}

pub fn extraction_api_key() -> Option<String> {
    std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("LLM_API_KEY"))
        .ok()
}
