use std::collections::HashMap;

use crate::models::v1::memory::MemoryCategory;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct QualityScore {
    pub information_density: f64,
    pub specificity: f64,
    pub timelessness: f64,
    pub reusability: f64,
    pub overall: f64,
}

impl QualityScore {
    pub fn calculate(
        information_density: f64,
        specificity: f64,
        timelessness: f64,
        reusability: f64,
    ) -> Self {
        let overall = information_density * 0.30
            + specificity * 0.30
            + timelessness * 0.20
            + reusability * 0.20;
        Self {
            information_density,
            specificity,
            timelessness,
            reusability,
            overall,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub consent_required: bool,
    pub review_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DedupAction {
    Duplicate,
    Mergeable,
    New,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupResult {
    pub similarity: f64,
    pub threshold_category: String,
    pub action: DedupAction,
    pub similar_entry_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EquivalenceResult {
    Equivalent,
    Mergeable,
    Conflict,
    Distinct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResult {
    pub has_conflict: bool,
    pub conflict_type: ConflictType,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    Overwrite,
    KeepBoth,
    MergeRequired,
    Discard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquivalenceCheckInput {
    pub candidate_content: serde_json::Value,
    pub existing_entry_id: Uuid,
    pub existing_content: serde_json::Value,
    pub time_delta_seconds: Option<i64>,
    pub same_session: bool,
    pub same_task: bool,
    pub same_agent: bool,
    pub content_similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarMemoryResult {
    pub entry_id: Uuid,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub similarity: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GateDecisionType {
    Approve,
    Review,
    Merge,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    pub decision: GateDecisionType,
    pub write_mode: Option<WriteMode>,
    pub target_entry_id: Option<Uuid>,
    pub reason: String,
    pub priority: Option<ReviewPriority>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteMode {
    Insert,
    Merge {
        target_id: Uuid,
        strategy: MergeStrategy,
    },
    Replace {
        target_id: Uuid,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MergeStrategy {
    Summarize,
    Append,
    KeepSeparate,
    WithProvenance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RejectionCategory {
    Duplicate,
    LowQuality,
    PolicyViolation,
    Conflict,
    ConsentRequired,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateGenerationConfig {
    pub enabled: bool,
    pub extraction_model: String,
    pub max_candidates_per_execution: usize,
    pub min_content_length: usize,
    pub excluded_tools: Vec<String>,
}

impl Default for CandidateGenerationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extraction_model: "gpt-4o-mini".to_string(),
            max_candidates_per_execution: 5,
            min_content_length: 20,
            excluded_tools: vec!["echo".to_string(), "ping".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub task_id: Uuid,
    pub agent_instance_id: Uuid,
    pub goal: String,
    pub output_summary: String,
    pub tool_calls: Vec<ToolCallSummary>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: Option<String>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupThresholds {
    pub duplicate: f64,
    pub merge: f64,
    pub minimum_content_length: usize,
}

impl DedupThresholds {
    pub fn for_category(category: &MemoryCategory) -> Self {
        match category {
            MemoryCategory::AgentProfileMemory | MemoryCategory::UserPreferenceMemory => {
                DedupThresholds {
                    duplicate: 0.96,
                    merge: 0.88,
                    minimum_content_length: 10,
                }
            }
            MemoryCategory::TaskOrDomainMemory => DedupThresholds {
                duplicate: 0.95,
                merge: 0.85,
                minimum_content_length: 20,
            },
            MemoryCategory::EpisodicMemory => DedupThresholds {
                duplicate: 0.94,
                merge: 0.85,
                minimum_content_length: 30,
            },
            MemoryCategory::ExternalContextMemory => DedupThresholds {
                duplicate: 0.93,
                merge: 0.80,
                minimum_content_length: 5,
            },
            MemoryCategory::Session => DedupThresholds {
                duplicate: 0.94,
                merge: 0.85,
                minimum_content_length: 30,
            },
        }
    }

    pub fn from_config(config: &GatingConfig, category: &MemoryCategory) -> Self {
        config
            .dedup_thresholds
            .get(category)
            .cloned()
            .unwrap_or_else(|| Self::for_category(category))
    }

    pub fn with_env_override(mut self, category: &MemoryCategory) -> Self {
        let prefix = format!("MEMORY_DEDUP_{}", category.to_env_suffix());
        if let Ok(v) = std::env::var(&format!("{}_DUPLICATE", prefix)) {
            if let Ok(val) = v.parse::<f64>() {
                self.duplicate = val;
            }
        }
        if let Ok(v) = std::env::var(&format!("{}_MERGE", prefix)) {
            if let Ok(val) = v.parse::<f64>() {
                self.merge = val;
            }
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatingConfig {
    pub auto_approve_quality_threshold: f64,
    pub auto_approve_confidence_threshold: f64,
    pub dedup_thresholds: HashMap<MemoryCategory, DedupThresholds>,
}

impl Default for GatingConfig {
    fn default() -> Self {
        let mut dedup_thresholds: HashMap<MemoryCategory, DedupThresholds> = HashMap::new();
        dedup_thresholds.insert(
            MemoryCategory::AgentProfileMemory,
            DedupThresholds::for_category(&MemoryCategory::AgentProfileMemory),
        );
        dedup_thresholds.insert(
            MemoryCategory::UserPreferenceMemory,
            DedupThresholds::for_category(&MemoryCategory::UserPreferenceMemory),
        );
        dedup_thresholds.insert(
            MemoryCategory::TaskOrDomainMemory,
            DedupThresholds::for_category(&MemoryCategory::TaskOrDomainMemory),
        );
        dedup_thresholds.insert(
            MemoryCategory::EpisodicMemory,
            DedupThresholds::for_category(&MemoryCategory::EpisodicMemory),
        );
        dedup_thresholds.insert(
            MemoryCategory::ExternalContextMemory,
            DedupThresholds::for_category(&MemoryCategory::ExternalContextMemory),
        );
        Self {
            auto_approve_quality_threshold: 0.88,
            auto_approve_confidence_threshold: 0.85,
            dedup_thresholds,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionFactors {
    pub quality_score: f64,
    pub confidence: f64,
    pub similarity_to_existing: Option<f64>,
    pub equivalence_result: Option<String>,
    pub risk_level: String,
    pub has_conflict: bool,
    pub consent_required: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid threshold for {category}.{field}: {value} (must be 0.0-1.0)")]
    InvalidThreshold {
        category: String,
        field: String,
        value: f64,
    },

    #[error("Invalid quality threshold: {0} (must be 0.0-1.0)")]
    InvalidQualityThreshold(f64),

    #[error("merge threshold must be <= duplicate threshold for {category}")]
    MergeGreaterThanDuplicate { category: String },
}

pub struct GatingConfigValidator;

impl GatingConfigValidator {
    pub fn validate(config: &GatingConfig) -> Result<(), ConfigError> {
        for (category, thresholds) in &config.dedup_thresholds {
            if thresholds.duplicate > 1.0 || thresholds.duplicate < 0.0 {
                return Err(ConfigError::InvalidThreshold {
                    category: format!("{:?}", category),
                    field: "duplicate".to_string(),
                    value: thresholds.duplicate,
                });
            }

            if thresholds.merge > thresholds.duplicate {
                return Err(ConfigError::MergeGreaterThanDuplicate {
                    category: format!("{:?}", category),
                });
            }

            if thresholds.merge < 0.0 || thresholds.merge > 1.0 {
                return Err(ConfigError::InvalidThreshold {
                    category: format!("{:?}", category),
                    field: "merge".to_string(),
                    value: thresholds.merge,
                });
            }
        }

        if config.auto_approve_quality_threshold > 1.0
            || config.auto_approve_quality_threshold < 0.0 {
            return Err(ConfigError::InvalidQualityThreshold(config.auto_approve_quality_threshold));
        }

        Ok(())
    }
}
