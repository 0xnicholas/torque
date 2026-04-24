use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::Type, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryCategory {
    AgentProfileMemory,
    UserPreferenceMemory,
    TaskOrDomainMemory,
    EpisodicMemory,
    ExternalContextMemory,
    Session,
}

impl MemoryCategory {
    pub fn to_env_suffix(&self) -> String {
        match self {
            MemoryCategory::AgentProfileMemory => "AGENT_PROFILE".to_string(),
            MemoryCategory::UserPreferenceMemory => "USER_PREFERENCE".to_string(),
            MemoryCategory::TaskOrDomainMemory => "TASK_DOMAIN".to_string(),
            MemoryCategory::EpisodicMemory => "EPISODIC".to_string(),
            MemoryCategory::ExternalContextMemory => "EXTERNAL_CONTEXT".to_string(),
            MemoryCategory::Session => "SESSION".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub source_type: String,
    pub source_ref: Option<Uuid>,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, sqlx::Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryWriteCandidateStatus {
    Pending,
    ReviewRequired,
    AutoApproved,
    Approved,
    Rejected,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryWriteCandidate {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub team_instance_id: Option<Uuid>,
    pub content: serde_json::Value,
    pub reasoning: Option<String>,
    pub status: MemoryWriteCandidateStatus,
    pub memory_entry_id: Option<Uuid>,
    pub reviewed_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryWriteCandidateCreate {
    pub agent_instance_id: Uuid,
    pub team_instance_id: Option<Uuid>,
    pub content: MemoryContent,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RejectCandidateRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeCandidateRequest {
    pub target_id: Uuid,
    pub strategy: String,
}

// Internal row structure for database queries (includes embedding)
#[derive(Debug, Clone)]
pub struct MemoryEntryRow {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub source_candidate_id: Option<Uuid>,
    pub superseded_by: Option<Uuid>,
    pub embedding: Option<crate::vector_type::Vector>,
    pub embedding_model: Option<String>,
    pub access_count: i32,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for MemoryEntryRow {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            agent_instance_id: row.try_get("agent_instance_id")?,
            team_instance_id: row.try_get("team_instance_id")?,
            category: row.try_get("category")?,
            key: row.try_get("key")?,
            value: row.try_get("value")?,
            source_candidate_id: row.try_get("source_candidate_id")?,
            superseded_by: row.try_get("superseded_by")?,
            embedding: row.try_get("embedding")?,
            embedding_model: row.try_get("embedding_model")?,
            access_count: row.try_get("access_count")?,
            last_accessed_at: row.try_get("last_accessed_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl From<MemoryEntryRow> for MemoryEntry {
    fn from(row: MemoryEntryRow) -> Self {
        Self {
            id: row.id,
            agent_instance_id: row.agent_instance_id,
            team_instance_id: row.team_instance_id,
            category: row.category,
            key: row.key,
            value: row.value,
            source_candidate_id: row.source_candidate_id,
            superseded_by: row.superseded_by,
            embedding_model: row.embedding_model,
            access_count: row.access_count,
            last_accessed_at: row.last_accessed_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// Public API model (no embedding field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub source_candidate_id: Option<Uuid>,
    pub superseded_by: Option<Uuid>,
    pub embedding_model: Option<String>,
    pub access_count: i32,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SemanticSearchRow {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub source_candidate_id: Option<Uuid>,
    pub superseded_by: Option<Uuid>,
    pub embedding: Option<crate::vector_type::Vector>,
    pub embedding_model: Option<String>,
    pub access_count: i32,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub similarity: f64,
}

impl From<SemanticSearchRow> for SemanticSearchResult {
    fn from(row: SemanticSearchRow) -> Self {
        Self {
            entry: MemoryEntry {
                id: row.id,
                agent_instance_id: row.agent_instance_id,
                team_instance_id: row.team_instance_id,
                category: row.category,
                key: row.key,
                value: row.value,
                source_candidate_id: row.source_candidate_id,
                superseded_by: row.superseded_by,
                embedding_model: row.embedding_model,
                access_count: row.access_count,
                last_accessed_at: row.last_accessed_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            similarity_score: row.similarity,
            search_method: "semantic".to_string(),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HybridSearchRow {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub source_candidate_id: Option<Uuid>,
    pub superseded_by: Option<Uuid>,
    pub embedding: Option<crate::vector_type::Vector>,
    pub embedding_model: Option<String>,
    pub access_count: i32,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub score: f64,
}

impl From<HybridSearchRow> for SemanticSearchResult {
    fn from(row: HybridSearchRow) -> Self {
        Self {
            entry: MemoryEntry {
                id: row.id,
                agent_instance_id: row.agent_instance_id,
                team_instance_id: row.team_instance_id,
                category: row.category,
                key: row.key,
                value: row.value,
                source_candidate_id: row.source_candidate_id,
                superseded_by: row.superseded_by,
                embedding_model: row.embedding_model,
                access_count: row.access_count,
                last_accessed_at: row.last_accessed_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            similarity_score: row.score,
            search_method: "hybrid".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSearchResult {
    #[serde(flatten)]
    pub entry: MemoryEntry,
    pub similarity_score: f64,
    pub search_method: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SemanticSearchQuery {
    pub query: String,
    pub category: Option<MemoryCategory>,
    pub limit: Option<i64>,
    pub hybrid: Option<bool>,
    pub vector_weight: Option<f64>,
    pub keyword_weight: Option<f64>,
}

impl Default for SemanticSearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            category: None,
            limit: Some(10),
            hybrid: Some(true),
            vector_weight: Some(0.7),
            keyword_weight: Some(0.3),
        }
    }
}

// Session Memory
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionMemoryEntry {
    pub id: Uuid,
    pub session_id: Uuid,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionMemorySet {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl_seconds: Option<i64>,
}

// Compaction Job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionJob {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub status: CompactionJobStatus,
    pub categories_processed: Vec<MemoryCategory>,
    pub entries_compacted: i64,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompactionJobStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

// Decision Log
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryDecisionLog {
    pub id: Uuid,
    pub candidate_id: Option<Uuid>,
    pub entry_id: Option<Uuid>,
    pub decision_type: String,
    pub decision_reason: Option<String>,
    pub factors: serde_json::Value,
    pub processed_by: String,
    pub processed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// Decision Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStats {
    pub total_decisions: i64,
    pub approved: i64,
    pub rejected: i64,
    pub merged: i64,
    pub review: i64,
    pub approval_rate: f64,
    pub rejection_rate: f64,
    pub avg_quality_score: Option<f64>,
    pub top_rejection_reasons: Vec<RejectionReasonCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionReasonCount {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompactionStrategy {
    Summarize,
    Merge,
    Archive,
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRecommendation {
    pub entry_id: Uuid,
    pub strategy: CompactionStrategy,
    pub reason: String,
    pub supersedes: Option<Uuid>,
}
