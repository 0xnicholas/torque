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
    db: Database,
}

impl PostgresMemoryRepositoryV1 {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MemoryRepositoryV1 for PostgresMemoryRepositoryV1 {
    async fn get_external_context_refs(
        &self,
        agent_instance_id: Uuid,
    ) -> anyhow::Result<Vec<ExternalContextRef>> {
        let row: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT external_context_refs FROM v1_agent_instances
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(agent_instance_id)
        .fetch_optional(self.db.pool())
        .await?;

        let refs_json = match row {
            Some((json,)) => json,
            None => return Ok(vec![]),
        };

        let refs: Vec<ExternalContextRef> = serde_json::from_value(refs_json).unwrap_or_default();

        Ok(refs)
    }

    async fn get_team_for_agent(&self, agent_instance_id: Uuid) -> anyhow::Result<Option<Uuid>> {
        let row: Option<(Option<Uuid>,)> = sqlx::query_as(
            r#"
            SELECT team_instance_id FROM v1_team_members
            WHERE agent_instance_id = $1
            LIMIT 1
            "#,
        )
        .bind(agent_instance_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.and_then(|r| r.0))
    }

    async fn get_last_event_id(&self, agent_instance_id: Uuid) -> anyhow::Result<Option<Uuid>> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT id FROM events
            WHERE subject_id = $1 AND subject_type = 'agent_instance'
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )
        .bind(agent_instance_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(id,)| id))
    }

    async fn get_artifacts_by_instance(
        &self,
        agent_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Artifact>> {
        let rows = sqlx::query_as::<_, Artifact>(
            "SELECT * FROM v1_artifacts WHERE source_instance_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(agent_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn create_entry(&self, entry: &MemoryEntry) -> anyhow::Result<MemoryEntry> {
        let row = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            INSERT INTO v1_memory_entries (
                id, agent_instance_id, team_instance_id, category, key, value,
                source_candidate_id, superseded_by, embedding, embedding_model,
                access_count, last_accessed_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NULL, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(entry.id)
        .bind(entry.agent_instance_id)
        .bind(entry.team_instance_id)
        .bind(&entry.category)
        .bind(&entry.key)
        .bind(&entry.value)
        .bind(entry.source_candidate_id)
        .bind(entry.superseded_by)
        .bind(&entry.embedding_model)
        .bind(entry.access_count)
        .bind(entry.last_accessed_at)
        .bind(entry.created_at)
        .bind(entry.updated_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row.into())
    }

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
    ) -> anyhow::Result<MemoryEntry> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let row = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            INSERT INTO v1_memory_entries (
                id, agent_instance_id, team_instance_id, category, key, value,
                source_candidate_id, superseded_by, embedding, embedding_model,
                access_count, last_accessed_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, 0, NULL, $10, $10)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(agent_instance_id)
        .bind(team_instance_id)
        .bind(&category)
        .bind(key)
        .bind(value)
        .bind(source_candidate_id)
        .bind(&embedding)
        .bind(&embedding_model)
        .bind(now)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row.into())
    }

    async fn list_entries(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            SELECT * FROM v1_memory_entries
            ORDER BY created_at DESC, id DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_entries_by_agent(
        &self,
        agent_instance_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            SELECT * FROM v1_memory_entries
            WHERE agent_instance_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(agent_instance_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_entry_by_id(&self, id: Uuid) -> anyhow::Result<Option<MemoryEntry>> {
        let row =
            sqlx::query_as::<_, MemoryEntryRow>(r#"SELECT * FROM v1_memory_entries WHERE id = $1"#)
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?;

        Ok(row.map(Into::into))
    }

    async fn get_entries_by_ids(&self, ids: Vec<Uuid>) -> anyhow::Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, MemoryEntryRow>(
            r#"SELECT * FROM v1_memory_entries WHERE id = ANY($1)"#,
        )
        .bind(&ids)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update_entry_access(&self, id: Uuid) -> anyhow::Result<Option<MemoryEntry>> {
        let row = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            UPDATE v1_memory_entries
            SET access_count = access_count + 1,
                last_accessed_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row.map(Into::into))
    }

    async fn update_entries_superseded_by(
        &self,
        entry_ids: &[Uuid],
        superseded_by: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            UPDATE v1_memory_entries
            SET superseded_by = $1,
                updated_at = NOW()
            WHERE id = ANY($2)
            RETURNING *
            "#,
        )
        .bind(superseded_by)
        .bind(entry_ids)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn semantic_search(
        &self,
        query_embedding: &crate::vector_type::Vector,
        category: Option<&MemoryCategory>,
        limit: i64,
    ) -> anyhow::Result<Vec<SemanticSearchResult>> {
        let embedding = crate::vector_type::Vector::from(query_embedding.clone());

        let rows: Vec<SemanticSearchRow> = if let Some(cat) = category {
            sqlx::query_as::<_, SemanticSearchRow>(
                r#"
                SELECT
                    id, agent_instance_id, team_instance_id, category, key, value,
                    source_candidate_id, superseded_by, embedding, embedding_model,
                    access_count, last_accessed_at, created_at, updated_at,
                    1 - (embedding <=> $1) as similarity
                FROM v1_memory_entries
                WHERE category = $2 AND embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $3
                "#,
            )
            .bind(&embedding)
            .bind(cat)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, SemanticSearchRow>(
                r#"
                SELECT
                    id, agent_instance_id, team_instance_id, category, key, value,
                    source_candidate_id, superseded_by, embedding, embedding_model,
                    access_count, last_accessed_at, created_at, updated_at,
                    1 - (embedding <=> $1) as similarity
                FROM v1_memory_entries
                WHERE embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $2
                "#,
            )
            .bind(&embedding)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn hybrid_search(
        &self,
        query_embedding: &crate::vector_type::Vector,
        keyword_query: &str,
        category: Option<&MemoryCategory>,
        limit: i64,
        vector_weight: f64,
        keyword_weight: f64,
    ) -> anyhow::Result<Vec<SemanticSearchResult>> {
        let embedding = crate::vector_type::Vector::from(query_embedding.clone());
        let search_terms = keyword_query
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| s.len() >= 2)
            .collect::<Vec<_>>()
            .join(" | ");

        if search_terms.is_empty() {
            return self.semantic_search(query_embedding, category, limit).await;
        }

        let rows: Vec<HybridSearchRow> = if let Some(cat) = category {
            sqlx::query_as::<_, HybridSearchRow>(
                r#"
                SELECT
                    id, agent_instance_id, team_instance_id, category, key, value,
                    source_candidate_id, superseded_by, embedding, embedding_model,
                    access_count, last_accessed_at, created_at, updated_at,
                    (
                        $4 * (1 - (embedding <=> $1)) +
                        $5 * COALESCE(ts_rank_cd(
                            to_tsvector('english', key || ' ' || COALESCE(value::text, '')),
                            to_tsquery('english', $2)
                        ), 0)
                    ) as score
                FROM v1_memory_entries
                WHERE category = $3
                  AND embedding IS NOT NULL
                ORDER BY score DESC
                LIMIT $6
                "#,
            )
            .bind(&embedding)
            .bind(&search_terms)
            .bind(cat)
            .bind(vector_weight)
            .bind(keyword_weight)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, HybridSearchRow>(
                r#"
                SELECT
                    id, agent_instance_id, team_instance_id, category, key, value,
                    source_candidate_id, superseded_by, embedding, embedding_model,
                    access_count, last_accessed_at, created_at, updated_at,
                    (
                        $3 * (1 - (embedding <=> $1)) +
                        $4 * COALESCE(ts_rank_cd(
                            to_tsvector('english', key || ' ' || COALESCE(value::text, '')),
                            to_tsquery('english', $2)
                        ), 0)
                    ) as score
                FROM v1_memory_entries
                WHERE embedding IS NOT NULL
                ORDER BY score DESC
                LIMIT $5
                "#,
            )
            .bind(&embedding)
            .bind(&search_terms)
            .bind(vector_weight)
            .bind(keyword_weight)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_similar_entries(
        &self,
        query_embedding: &crate::vector_type::Vector,
        category: Option<&MemoryCategory>,
        limit: i64,
    ) -> anyhow::Result<Vec<SimilarMemoryResult>> {
        let embedding = crate::vector_type::Vector::from(query_embedding.clone());

        let rows: Vec<SimilarMemoryRow> = if let Some(cat) = category {
            sqlx::query_as::<_, SimilarMemoryRow>(
                r#"
                SELECT
                    id, category, key, value,
                    1 - (embedding <=> $1) as similarity,
                    created_at
                FROM v1_memory_entries
                WHERE category = $2 AND embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $3
                "#,
            )
            .bind(&embedding)
            .bind(cat)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, SimilarMemoryRow>(
                r#"
                SELECT
                    id, category, key, value,
                    1 - (embedding <=> $1) as similarity,
                    created_at
                FROM v1_memory_entries
                WHERE embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $2
                "#,
            )
            .bind(&embedding)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create_candidate(
        &self,
        candidate: &MemoryWriteCandidate,
    ) -> anyhow::Result<MemoryWriteCandidate> {
        let row = sqlx::query_as::<_, MemoryWriteCandidate>(
            r#"
            INSERT INTO v1_memory_write_candidates (
                id, agent_instance_id, team_instance_id, content, reasoning,
                status, memory_entry_id, reviewed_by, created_at, reviewed_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(candidate.id)
        .bind(candidate.agent_instance_id)
        .bind(candidate.team_instance_id)
        .bind(&candidate.content)
        .bind(&candidate.reasoning)
        .bind(&candidate.status)
        .bind(candidate.memory_entry_id)
        .bind(&candidate.reviewed_by)
        .bind(candidate.created_at)
        .bind(candidate.reviewed_at)
        .bind(candidate.updated_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn list_candidates(
        &self,
        status: Option<MemoryWriteCandidateStatus>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>> {
        let rows = if let Some(s) = status {
            sqlx::query_as::<_, MemoryWriteCandidate>(
                r#"
                SELECT * FROM v1_memory_write_candidates
                WHERE status = $1
                ORDER BY created_at DESC, id DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(&s)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, MemoryWriteCandidate>(
                r#"
                SELECT * FROM v1_memory_write_candidates
                ORDER BY created_at DESC, id DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(rows)
    }

    async fn count_candidates_by_status(
        &self,
        agent_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = if let Some(agent_id) = agent_instance_id {
            sqlx::query_as(
                r#"
                SELECT status::text, COUNT(*)
                FROM v1_memory_write_candidates
                WHERE agent_instance_id = $1
                GROUP BY status
                "#,
            )
            .bind(agent_id)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT status::text, COUNT(*)
                FROM v1_memory_write_candidates
                GROUP BY status
                "#,
            )
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(rows)
    }

    async fn get_candidate_by_id(&self, id: Uuid) -> anyhow::Result<Option<MemoryWriteCandidate>> {
        let row = sqlx::query_as::<_, MemoryWriteCandidate>(
            r#"SELECT * FROM v1_memory_write_candidates WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn update_candidate_status(
        &self,
        id: Uuid,
        status: MemoryWriteCandidateStatus,
        reviewed_by: Option<String>,
        memory_entry_id: Option<Uuid>,
    ) -> anyhow::Result<Option<MemoryWriteCandidate>> {
        let row = sqlx::query_as::<_, MemoryWriteCandidate>(
            r#"
            UPDATE v1_memory_write_candidates
            SET status = $1,
                reviewed_by = COALESCE($2, reviewed_by),
                memory_entry_id = COALESCE($3, memory_entry_id),
                reviewed_at = CASE
                    WHEN $1::TEXT IN ('approved', 'rejected', 'merged') THEN COALESCE(reviewed_at, NOW())
                    ELSE reviewed_at
                END,
                updated_at = NOW()
            WHERE id = $4
            RETURNING *
            "#,
        )
        .bind(&status)
        .bind(reviewed_by)
        .bind(memory_entry_id)
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn session_memory_get(
        &self,
        session_id: Uuid,
        key: &str,
    ) -> anyhow::Result<Option<SessionMemoryEntry>> {
        let row = sqlx::query_as::<_, SessionMemoryEntry>(
            r#"
            SELECT * FROM session_memory
            WHERE session_id = $1 AND key = $2
              AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(session_id)
        .bind(key)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn session_memory_set(
        &self,
        session_id: Uuid,
        key: &str,
        value: serde_json::Value,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<SessionMemoryEntry> {
        let row = sqlx::query_as::<_, SessionMemoryEntry>(
            r#"
            INSERT INTO session_memory (id, session_id, key, value, created_at, expires_at)
            VALUES (gen_random_uuid(), $1, $2, $3, NOW(), $4)
            ON CONFLICT (session_id, key)
            DO UPDATE SET
                value = EXCLUDED.value,
                created_at = NOW(),
                expires_at = EXCLUDED.expires_at
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(key)
        .bind(value)
        .bind(expires_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn session_memory_delete(&self, session_id: Uuid, key: &str) -> anyhow::Result<bool> {
        let result =
            sqlx::query(r#"DELETE FROM session_memory WHERE session_id = $1 AND key = $2"#)
                .bind(session_id)
                .bind(key)
                .execute(self.db.pool())
                .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn session_memory_list(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Vec<SessionMemoryEntry>> {
        let rows = sqlx::query_as::<_, SessionMemoryEntry>(
            r#"
            SELECT * FROM session_memory
            WHERE session_id = $1
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            "#,
        )
        .bind(session_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows)
    }

    async fn session_memory_cleanup_expired(&self, batch_size: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM session_memory
            WHERE id IN (
                SELECT id FROM session_memory
                WHERE expires_at IS NOT NULL AND expires_at <= NOW()
                LIMIT $1
            )
            "#,
        )
        .bind(batch_size)
        .execute(self.db.pool())
        .await?;

        Ok(result.rows_affected())
    }

    async fn log_decision(
        &self,
        candidate_id: Option<Uuid>,
        entry_id: Option<Uuid>,
        decision_type: &str,
        decision_reason: Option<&str>,
        factors: serde_json::Value,
        processed_by: &str,
    ) -> anyhow::Result<MemoryDecisionLog> {
        let row = sqlx::query_as::<_, MemoryDecisionLog>(
            r#"
            INSERT INTO memory_decision_log (
                candidate_id, entry_id, decision_type, decision_reason,
                factors, processed_by, processed_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(candidate_id)
        .bind(entry_id)
        .bind(decision_type)
        .bind(decision_reason)
        .bind(factors)
        .bind(processed_by)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn list_decisions(
        &self,
        agent_instance_id: Option<Uuid>,
        decision_type: Option<&str>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryDecisionLog>> {
        let rows = if let (Some(agent_id), Some(dt), Some(start), Some(end)) =
            (agent_instance_id, decision_type, start_date, end_date)
        {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.decision_type = $2
                  AND mdl.created_at >= $3
                  AND mdl.created_at <= $4
                ORDER BY mdl.created_at DESC
                LIMIT $5 OFFSET $6
                "#,
            )
            .bind(agent_id)
            .bind(dt)
            .bind(start)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(agent_id), Some(dt), Some(start)) =
            (agent_instance_id, decision_type, start_date)
        {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.decision_type = $2
                  AND mdl.created_at >= $3
                ORDER BY mdl.created_at DESC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(agent_id)
            .bind(dt)
            .bind(start)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(agent_id), Some(dt), Some(end)) =
            (agent_instance_id, decision_type, end_date)
        {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.decision_type = $2
                  AND mdl.created_at <= $3
                ORDER BY mdl.created_at DESC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(agent_id)
            .bind(dt)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(agent_id), Some(start), Some(end)) =
            (agent_instance_id, start_date, end_date)
        {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.created_at >= $2
                  AND mdl.created_at <= $3
                ORDER BY mdl.created_at DESC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(agent_id)
            .bind(start)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(agent_id), Some(dt)) = (agent_instance_id, decision_type) {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.decision_type = $2
                ORDER BY mdl.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(agent_id)
            .bind(dt)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(agent_id), Some(start)) = (agent_instance_id, start_date) {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.created_at >= $2
                ORDER BY mdl.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(agent_id)
            .bind(start)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(agent_id), Some(end)) = (agent_instance_id, end_date) {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                  AND mdl.created_at <= $2
                ORDER BY mdl.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(agent_id)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let Some(agent_id) = agent_instance_id {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
                WHERE wc.agent_instance_id = $1
                ORDER BY mdl.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(agent_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(dt), Some(start), Some(end)) = (decision_type, start_date, end_date) {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                WHERE mdl.decision_type = $1
                  AND mdl.created_at >= $2
                  AND mdl.created_at <= $3
                ORDER BY mdl.created_at DESC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(dt)
            .bind(start)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let Some(dt) = decision_type {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                WHERE mdl.decision_type = $1
                ORDER BY mdl.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(dt)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let (Some(start), Some(end)) = (start_date, end_date) {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                WHERE mdl.created_at >= $1
                  AND mdl.created_at <= $2
                ORDER BY mdl.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(start)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let Some(start) = start_date {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                WHERE mdl.created_at >= $1
                ORDER BY mdl.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(start)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else if let Some(end) = end_date {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                WHERE mdl.created_at <= $1
                ORDER BY mdl.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, MemoryDecisionLog>(
                r#"
                SELECT mdl.* FROM memory_decision_log mdl
                ORDER BY mdl.created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?
        };

        Ok(rows)
    }

    async fn get_entries_without_embedding(&self, limit: i64) -> anyhow::Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            SELECT * FROM v1_memory_entries
            WHERE embedding IS NULL
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update_entry_embedding(
        &self,
        id: Uuid,
        embedding: &crate::vector_type::Vector,
        model: &str,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        let vec = embedding.clone();

        let row = sqlx::query_as::<_, MemoryEntryRow>(
            r#"
            UPDATE v1_memory_entries
            SET embedding = $1,
                embedding_model = $2,
                updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(&vec)
        .bind(model)
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row.map(Into::into))
    }

    async fn get_decision_stats(
        &self,
        agent_instance_id: Option<Uuid>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<DecisionStats> {
        let base_query = if let Some(agent_id) = agent_instance_id {
            r#"
            SELECT mdl.decision_type, COUNT(*) as count
            FROM memory_decision_log mdl
            JOIN v1_memory_write_candidates wc ON mdl.candidate_id = wc.id
            WHERE wc.agent_instance_id = $1
            "#
        } else {
            r#"
            SELECT mdl.decision_type, COUNT(*) as count
            FROM memory_decision_log mdl
            WHERE 1=1
            "#
        };

        let start_filter = if start_date.is_some() {
            " AND mdl.created_at >= $2"
        } else {
            ""
        };
        let end_filter = if end_date.is_some() {
            if start_date.is_some() {
                " AND mdl.created_at <= $3"
            } else {
                " AND mdl.created_at <= $2"
            }
        } else {
            ""
        };

        let group_by = " GROUP BY mdl.decision_type";

        let (type_rows, rejection_rows): (Vec<(String, i64)>, Vec<(String, i64)>) =
            if let (Some(agent_id), Some(start), Some(end)) =
                (agent_instance_id, start_date, end_date)
            {
                let query = format!("{}{}{}{}", base_query, start_filter, end_filter, group_by);
                let rejection_query = format!(
                    "{}{}{}{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query, start_filter, end_filter, group_by
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(agent_id)
                    .bind(start)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(agent_id)
                    .bind(start)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else if let (Some(agent_id), Some(start)) = (agent_instance_id, start_date) {
                let query = format!("{}{}{}", base_query, start_filter, group_by);
                let rejection_query = format!(
                    "{}{}{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query, start_filter, group_by
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(agent_id)
                    .bind(start)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(agent_id)
                    .bind(start)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else if let (Some(agent_id), Some(end)) = (agent_instance_id, end_date) {
                let query = format!("{}{}{}", base_query, end_filter, group_by);
                let rejection_query = format!(
                    "{}{}{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query, end_filter, group_by
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(agent_id)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(agent_id)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else if let Some(agent_id) = agent_instance_id {
                let query = format!("{}{}", base_query, group_by);
                let rejection_query = format!(
                    "{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(agent_id)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(agent_id)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else if let (Some(start), Some(end)) = (start_date, end_date) {
                let query = format!("{}{}{}{}", base_query, start_filter, end_filter, group_by);
                let rejection_query = format!(
                    "{}{}{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query, start_filter, end_filter
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(start)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(start)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else if let Some(start) = start_date {
                let query = format!("{}{}{}", base_query, start_filter, group_by);
                let rejection_query = format!(
                    "{}{}{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query, start_filter, group_by
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(start)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(start)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else if let Some(end) = end_date {
                let query = format!("{}{}{}", base_query, end_filter, group_by);
                let rejection_query = format!(
                    "{}{}{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query, end_filter, group_by
                );

                let type_rows: Vec<(String, i64)> = sqlx::query_as(&query)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .bind(end)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            } else {
                let query = format!("{} {}", base_query, group_by);
                let rejection_query = format!(
                    "{} AND mdl.decision_type = 'rejected' GROUP BY mdl.decision_reason",
                    base_query
                );

                let type_rows: Vec<(String, i64)> =
                    sqlx::query_as(&query).fetch_all(self.db.pool()).await?;

                let rejection_rows: Vec<(String, i64)> = sqlx::query_as(&rejection_query)
                    .fetch_all(self.db.pool())
                    .await?;

                (type_rows, rejection_rows)
            };

        let mut approved = 0i64;
        let mut rejected = 0i64;
        let mut merged = 0i64;
        let mut review = 0i64;

        for (decision_type, count) in type_rows {
            match decision_type.as_str() {
                "approved" => approved = count,
                "rejected" => rejected = count,
                "merged" => merged = count,
                "review" => review = count,
                _ => {}
            }
        }

        let total_decisions = approved + rejected + merged + review;
        let approval_rate = if total_decisions > 0 {
            approved as f64 / total_decisions as f64
        } else {
            0.0
        };
        let rejection_rate = if total_decisions > 0 {
            rejected as f64 / total_decisions as f64
        } else {
            0.0
        };

        let top_rejection_reasons: Vec<RejectionReasonCount> = rejection_rows
            .into_iter()
            .filter(|(reason, _)| !reason.is_empty())
            .map(|(reason, count)| RejectionReasonCount { reason, count })
            .collect();

        Ok(DecisionStats {
            total_decisions,
            approved,
            rejected,
            merged,
            review,
            approval_rate,
            rejection_rate,
            avg_quality_score: None,
            top_rejection_reasons,
        })
    }
}
