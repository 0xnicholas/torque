use crate::embedding::{memory_to_embedding_text, EmbeddingGenerator};
use crate::models::v1::memory::{
    CompactionJob, CompactionJobStatus, MemoryCategory, MemoryDecisionLog,
    MemoryEntry as V1MemoryEntry, MemoryWriteCandidate, MemoryWriteCandidateStatus,
    SemanticSearchQuery, SemanticSearchResult, SessionMemoryEntry, SessionMemorySet,
};
use crate::models::{MemoryCandidate, MemoryCandidateStatus, MemoryEntry, MemoryEntryStatus};
use crate::repository::{MemoryRepository, MemoryRepositoryV1};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct MemoryService {
    repo: Arc<dyn MemoryRepository>,
    repo_v1: Arc<dyn MemoryRepositoryV1>,
    embedding: Option<Arc<dyn EmbeddingGenerator>>,
}

impl MemoryService {
    pub fn new(
        repo: Arc<dyn MemoryRepository>,
        repo_v1: Arc<dyn MemoryRepositoryV1>,
        embedding: Option<Arc<dyn EmbeddingGenerator>>,
    ) -> Self {
        Self {
            repo,
            repo_v1,
            embedding,
        }
    }

    // Legacy methods (keep for backward compatibility)
    pub fn repo(&self) -> &Arc<dyn MemoryRepository> {
        &self.repo
    }

    pub async fn create_candidate(
        &self,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryCandidate> {
        self.repo.create_candidate(candidate).await
    }

    pub async fn accept_candidate(
        &self,
        project_scope: &str,
        candidate_id: Uuid,
    ) -> anyhow::Result<Option<(MemoryCandidate, MemoryEntry)>> {
        self.repo
            .accept_candidate_to_entry(project_scope, candidate_id)
            .await
    }

    pub async fn create_entry(&self, entry: &MemoryEntry) -> anyhow::Result<MemoryEntry> {
        self.repo.create_entry(entry).await
    }

    pub async fn list_candidates(
        &self,
        project_scope: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryCandidate>> {
        self.repo
            .list_candidates(project_scope, limit, offset)
            .await
    }

    pub async fn list_entries(
        &self,
        project_scope: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.repo.list_entries(project_scope, limit, offset).await
    }

    pub async fn search_entries(
        &self,
        project_scope: &str,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.repo.search_entries(project_scope, query, limit).await
    }

    pub async fn get_entry_by_id(
        &self,
        project_scope: &str,
        id: Uuid,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        self.repo.get_entry_by_id(project_scope, id).await
    }

    pub async fn get_candidate_by_id(
        &self,
        project_scope: &str,
        id: Uuid,
    ) -> anyhow::Result<Option<MemoryCandidate>> {
        self.repo.get_candidate_by_id(project_scope, id).await
    }

    pub async fn update_candidate_status(
        &self,
        project_scope: &str,
        id: Uuid,
        status: MemoryCandidateStatus,
    ) -> anyhow::Result<Option<MemoryCandidate>> {
        self.repo
            .update_candidate_status(project_scope, id, status)
            .await
    }

    pub async fn update_entry_status(
        &self,
        project_scope: &str,
        id: Uuid,
        status: MemoryEntryStatus,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        self.repo
            .update_entry_status(project_scope, id, status)
            .await
    }

    // V1 Methods
    pub async fn v1_create_entry(
        &self,
        agent_instance_id: Option<Uuid>,
        team_instance_id: Option<Uuid>,
        category: MemoryCategory,
        key: &str,
        value: serde_json::Value,
        source_candidate_id: Option<Uuid>,
    ) -> anyhow::Result<V1MemoryEntry> {
        let embedding = if let Some(embedding_gen) = &self.embedding {
            let text = memory_to_embedding_text(&crate::models::v1::memory::MemoryContent {
                category: category.clone(),
                key: key.to_string(),
                value: value.clone(),
            });
            let emb = embedding_gen.generate(&text).await?;
            Some(crate::vector_type::Vector::from(emb))
        } else {
            None
        };

        let model = self.embedding.as_ref().map(|e| e.model_name().to_string());

        self.repo_v1
            .create_entry_with_embedding(
                agent_instance_id,
                team_instance_id,
                category,
                key,
                value,
                source_candidate_id,
                embedding,
                model,
            )
            .await
    }

    pub async fn v1_list_entries(
        &self,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<V1MemoryEntry>> {
        self.repo_v1.list_entries(limit, offset).await
    }

    pub async fn v1_get_entry(&self, id: Uuid) -> anyhow::Result<Option<V1MemoryEntry>> {
        self.repo_v1.get_entry_by_id(id).await
    }

    pub async fn v1_semantic_search(
        &self,
        query: &SemanticSearchQuery,
    ) -> anyhow::Result<Vec<SemanticSearchResult>> {
        let embedding_gen = self
            .embedding
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding generator not configured"))?;

        let query_embedding_vec = embedding_gen.generate(&query.query).await?;
        let query_embedding = crate::vector_type::Vector::from(query_embedding_vec);

        let use_hybrid = query.hybrid.unwrap_or(true);

        if use_hybrid {
            let vector_weight = query.vector_weight.unwrap_or(0.7);
            let keyword_weight = query.keyword_weight.unwrap_or(0.3);
            self.repo_v1
                .hybrid_search(
                    &query_embedding,
                    &query.query,
                    query.category.as_ref(),
                    query.limit.unwrap_or(10),
                    vector_weight,
                    keyword_weight,
                )
                .await
        } else {
            self.repo_v1
                .semantic_search(
                    &query_embedding,
                    query.category.as_ref(),
                    query.limit.unwrap_or(10),
                )
                .await
        }
    }

    pub async fn v1_create_candidate(
        &self,
        candidate: &MemoryWriteCandidate,
    ) -> anyhow::Result<MemoryWriteCandidate> {
        self.repo_v1.create_candidate(candidate).await
    }

    pub async fn v1_list_candidates(
        &self,
        status: Option<MemoryWriteCandidateStatus>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>> {
        self.repo_v1.list_candidates(status, limit, offset).await
    }

    pub async fn v1_count_candidates_by_status(
        &self,
        agent_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Option<Vec<(String, i64)>>> {
        let counts = self
            .repo_v1
            .count_candidates_by_status(agent_instance_id)
            .await?;
        if counts.is_empty() {
            Ok(None)
        } else {
            Ok(Some(counts))
        }
    }

    pub async fn v1_get_candidate(&self, id: Uuid) -> anyhow::Result<Option<MemoryWriteCandidate>> {
        self.repo_v1.get_candidate_by_id(id).await
    }

    pub async fn v1_update_candidate_status(
        &self,
        id: Uuid,
        status: MemoryWriteCandidateStatus,
        reviewed_by: Option<String>,
        memory_entry_id: Option<Uuid>,
    ) -> anyhow::Result<Option<MemoryWriteCandidate>> {
        self.repo_v1
            .update_candidate_status(id, status, reviewed_by, memory_entry_id)
            .await
    }

    // Session Memory
    pub async fn session_memory_get(
        &self,
        session_id: Uuid,
        key: &str,
    ) -> anyhow::Result<Option<SessionMemoryEntry>> {
        self.repo_v1.session_memory_get(session_id, key).await
    }

    pub async fn session_memory_set(
        &self,
        session_id: Uuid,
        req: &SessionMemorySet,
    ) -> anyhow::Result<SessionMemoryEntry> {
        let expires_at = req
            .ttl_seconds
            .map(|ttl| chrono::Utc::now() + chrono::Duration::seconds(ttl));
        self.repo_v1
            .session_memory_set(session_id, &req.key, req.value.clone(), expires_at)
            .await
    }

    pub async fn session_memory_delete(&self, session_id: Uuid, key: &str) -> anyhow::Result<bool> {
        self.repo_v1.session_memory_delete(session_id, key).await
    }

    pub async fn session_memory_list(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Vec<SessionMemoryEntry>> {
        self.repo_v1.session_memory_list(session_id).await
    }

    pub async fn session_memory_cleanup(&self, batch_size: i64) -> anyhow::Result<u64> {
        self.repo_v1
            .session_memory_cleanup_expired(batch_size)
            .await
    }

    // Decision Log
    pub async fn log_decision(
        &self,
        candidate_id: Option<Uuid>,
        entry_id: Option<Uuid>,
        decision_type: &str,
        decision_reason: Option<&str>,
        factors: serde_json::Value,
        processed_by: &str,
    ) -> anyhow::Result<MemoryDecisionLog> {
        self.repo_v1
            .log_decision(
                candidate_id,
                entry_id,
                decision_type,
                decision_reason,
                factors,
                processed_by,
            )
            .await
    }

    pub async fn list_decisions(
        &self,
        agent_instance_id: Option<Uuid>,
        decision_type: Option<&str>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryDecisionLog>> {
        self.repo_v1
            .list_decisions(
                agent_instance_id,
                decision_type,
                start_date,
                end_date,
                limit,
                offset,
            )
            .await
    }

    // Backfill
    pub async fn get_entries_without_embedding(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<V1MemoryEntry>> {
        self.repo_v1.get_entries_without_embedding(limit).await
    }

    pub async fn backfill_embedding(
        &self,
        entry_id: Uuid,
        category: MemoryCategory,
        key: &str,
        value: &serde_json::Value,
    ) -> anyhow::Result<Option<V1MemoryEntry>> {
        let embedding_gen = self
            .embedding
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding generator not configured"))?;

        let text = memory_to_embedding_text(&crate::models::v1::memory::MemoryContent {
            category,
            key: key.to_string(),
            value: value.clone(),
        });
        let embedding_vec = embedding_gen.generate(&text).await?;
        let embedding = crate::vector_type::Vector::from(embedding_vec);
        let model = embedding_gen.model_name().to_string();

        self.repo_v1
            .update_entry_embedding(entry_id, &embedding, &model)
            .await
    }

    pub async fn trigger_compaction(
        &self,
        agent_instance_id: Option<Uuid>,
        team_instance_id: Option<Uuid>,
        categories: Option<Vec<MemoryCategory>>,
    ) -> anyhow::Result<CompactionJob> {
        let job = CompactionJob {
            id: Uuid::new_v4(),
            agent_instance_id,
            team_instance_id,
            status: CompactionJobStatus::Pending,
            categories_processed: categories.unwrap_or_default(),
            entries_compacted: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        let repo = self.repo_v1.clone();
        let job_id = job.id;

        tokio::spawn(async move {
            let _ = Self::run_compaction(repo, job_id).await;
        });

        Ok(job)
    }

    async fn run_compaction(
        repo: Arc<dyn MemoryRepositoryV1>,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
