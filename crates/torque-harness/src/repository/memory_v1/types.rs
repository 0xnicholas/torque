use crate::db::Database;
use crate::models::v1::artifact::Artifact;
use crate::models::v1::external_context::ExternalContextRef;
use crate::models::v1::gating::SimilarMemoryResult;
use crate::models::v1::memory::{
    DecisionStats, HybridSearchRow, MemoryCategory, MemoryDecisionLog, MemoryEntry, MemoryEntryRow,
    MemoryWriteCandidate, MemoryWriteCandidateStatus, RejectionReasonCount, SemanticSearchResult,
    SemanticSearchRow, SessionMemoryEntry,
};
use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SimilarMemoryRow {
    pub id: Uuid,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub similarity: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<SimilarMemoryRow> for SimilarMemoryResult {
    fn from(row: SimilarMemoryRow) -> Self {
        Self {
            entry_id: row.id,
            category: row.category,
            key: row.key,
            value: row.value,
            similarity: row.similarity,
            created_at: row.created_at,
        }
    }
}

#[async_trait]
pub trait MemoryRepositoryV1: Send + Sync {
    // Context Anchors
    async fn get_external_context_refs(
        &self,
        agent_instance_id: Uuid,
    ) -> anyhow::Result<Vec<ExternalContextRef>>;

    async fn get_team_for_agent(&self, agent_instance_id: Uuid) -> anyhow::Result<Option<Uuid>>;

    async fn get_last_event_id(&self, agent_instance_id: Uuid) -> anyhow::Result<Option<Uuid>>;

    async fn get_artifacts_by_instance(
        &self,
        agent_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Artifact>>;

    // Memory Entries
    async fn create_entry(&self, entry: &MemoryEntry) -> anyhow::Result<MemoryEntry>;

    async fn create_entry_with_embedding(
        &self,
        agent_instance_id: Option<Uuid>,
        team_instance_id: Option<Uuid>,
        category: MemoryCategory,
        key: &str,
        value: serde_json::Value,
        source_candidate_id: Option<Uuid>,
        embedding: Option<crate::vector_type::Vector>,
        embedding_model: Option<String>,
    ) -> anyhow::Result<MemoryEntry>;

    async fn list_entries(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<MemoryEntry>>;

    async fn list_entries_by_agent(
        &self,
        agent_instance_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>>;

    async fn get_entry_by_id(&self, id: Uuid) -> anyhow::Result<Option<MemoryEntry>>;

    async fn get_entries_by_ids(&self, ids: Vec<Uuid>) -> anyhow::Result<Vec<MemoryEntry>>;

    async fn update_entry_access(&self, id: Uuid) -> anyhow::Result<Option<MemoryEntry>>;

    async fn update_entries_superseded_by(
        &self,
        entry_ids: &[Uuid],
        superseded_by: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>>;

    // Semantic Search
    async fn semantic_search(
        &self,
        query_embedding: &crate::vector_type::Vector,
        category: Option<&MemoryCategory>,
        limit: i64,
    ) -> anyhow::Result<Vec<SemanticSearchResult>>;

    async fn hybrid_search(
        &self,
        query_embedding: &crate::vector_type::Vector,
        keyword_query: &str,
        category: Option<&MemoryCategory>,
        limit: i64,
        vector_weight: f64,
        keyword_weight: f64,
    ) -> anyhow::Result<Vec<SemanticSearchResult>>;

    async fn find_similar_entries(
        &self,
        query_embedding: &crate::vector_type::Vector,
        category: Option<&MemoryCategory>,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::models::v1::gating::SimilarMemoryResult>>;

    // Candidates
    async fn create_candidate(
        &self,
        candidate: &MemoryWriteCandidate,
    ) -> anyhow::Result<MemoryWriteCandidate>;

    async fn list_candidates(
        &self,
        status: Option<MemoryWriteCandidateStatus>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>>;

    async fn get_candidate_by_id(&self, id: Uuid) -> anyhow::Result<Option<MemoryWriteCandidate>>;

    async fn count_candidates_by_status(
        &self,
        agent_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<(String, i64)>>;

    async fn update_candidate_status(
        &self,
        id: Uuid,
        status: MemoryWriteCandidateStatus,
        reviewed_by: Option<String>,
        memory_entry_id: Option<Uuid>,
    ) -> anyhow::Result<Option<MemoryWriteCandidate>>;

    // Session Memory
    async fn session_memory_get(
        &self,
        session_id: Uuid,
        key: &str,
    ) -> anyhow::Result<Option<SessionMemoryEntry>>;

    async fn session_memory_set(
        &self,
        session_id: Uuid,
        key: &str,
        value: serde_json::Value,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<SessionMemoryEntry>;

    async fn session_memory_delete(&self, session_id: Uuid, key: &str) -> anyhow::Result<bool>;

    async fn session_memory_list(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Vec<SessionMemoryEntry>>;

    async fn session_memory_cleanup_expired(&self, batch_size: i64) -> anyhow::Result<u64>;

    // Decision Log
    async fn log_decision(
        &self,
        candidate_id: Option<Uuid>,
        entry_id: Option<Uuid>,
        decision_type: &str,
        decision_reason: Option<&str>,
        factors: serde_json::Value,
        processed_by: &str,
    ) -> anyhow::Result<MemoryDecisionLog>;

    async fn list_decisions(
        &self,
        agent_instance_id: Option<Uuid>,
        decision_type: Option<&str>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryDecisionLog>>;

    async fn get_decision_stats(
        &self,
        agent_instance_id: Option<Uuid>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<DecisionStats>;

    // Backfill
    async fn get_entries_without_embedding(&self, limit: i64) -> anyhow::Result<Vec<MemoryEntry>>;

    async fn update_entry_embedding(
        &self,
        id: Uuid,
        embedding: &crate::vector_type::Vector,
        model: &str,
    ) -> anyhow::Result<Option<MemoryEntry>>;
}

pub struct PostgresMemoryRepositoryV1 {
    pub(crate) db: Database,
}

impl PostgresMemoryRepositoryV1 {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

