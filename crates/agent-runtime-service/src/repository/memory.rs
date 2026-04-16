use async_trait::async_trait;
use crate::db::Database;
use crate::models::{MemoryCandidate, MemoryEntry, MemoryEntryStatus};
use uuid::Uuid;

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn create_candidate(
        &self,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryCandidate>;
    async fn accept_candidate_to_entry(
        &self,
        project_scope: &str,
        candidate_id: Uuid,
    ) -> anyhow::Result<Option<(MemoryCandidate, MemoryEntry)>>;
    async fn list_entries(
        &self,
        project_scope: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
    async fn search_entries(
        &self,
        project_scope: &str,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
    async fn get_entry_by_id(
        &self,
        project_scope: &str,
        id: Uuid,
    ) -> anyhow::Result<Option<MemoryEntry>>;
    async fn update_entry_status(
        &self,
        project_scope: &str,
        id: Uuid,
        status: MemoryEntryStatus,
    ) -> anyhow::Result<Option<MemoryEntry>>;
}

#[allow(dead_code)]
pub struct PostgresMemoryRepository {
    db: Database,
}

impl PostgresMemoryRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MemoryRepository for PostgresMemoryRepository {
    async fn create_candidate(
        &self,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryCandidate> {
        let row = sqlx::query_as::<_, MemoryCandidate>(
            r#"
            INSERT INTO memory_candidates (
                id,
                project_scope,
                layer,
                proposed_fact,
                source_type,
                source_ref,
                proposer,
                confidence,
                status,
                created_at,
                updated_at,
                accepted_at,
                rejected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(candidate.id)
        .bind(&candidate.project_scope)
        .bind(&candidate.layer)
        .bind(&candidate.proposed_fact)
        .bind(&candidate.source_type)
        .bind(&candidate.source_ref)
        .bind(&candidate.proposer)
        .bind(candidate.confidence)
        .bind(&candidate.status)
        .bind(candidate.created_at)
        .bind(candidate.updated_at)
        .bind(candidate.accepted_at)
        .bind(candidate.rejected_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn accept_candidate_to_entry(
        &self,
        project_scope: &str,
        candidate_id: Uuid,
    ) -> anyhow::Result<Option<(MemoryCandidate, MemoryEntry)>> {
        let mut tx = self.db.pool().begin().await?;

        let candidate = sqlx::query_as::<_, MemoryCandidate>(
            r#"
            SELECT *
            FROM memory_candidates
            WHERE project_scope = $1 AND id = $2
            FOR UPDATE
            "#,
        )
        .bind(project_scope)
        .bind(candidate_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(_candidate) = candidate else {
            tx.rollback().await?;
            return Ok(None);
        };

        if let Some(existing_entry) = sqlx::query_as::<_, MemoryEntry>(
            r#"
            SELECT *
            FROM memory_entries
            WHERE project_scope = $1 AND source_candidate_id = $2
            "#,
        )
        .bind(project_scope)
        .bind(candidate_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            let accepted_candidate = sqlx::query_as::<_, MemoryCandidate>(
                r#"
                UPDATE memory_candidates
                SET status = 'accepted',
                    updated_at = NOW(),
                    accepted_at = COALESCE(accepted_at, NOW()),
                    rejected_at = NULL
                WHERE project_scope = $1 AND id = $2
                RETURNING *
                "#,
            )
            .bind(project_scope)
            .bind(candidate_id)
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            return Ok(Some((accepted_candidate, existing_entry)));
        }

        let accepted_candidate = sqlx::query_as::<_, MemoryCandidate>(
            r#"
            UPDATE memory_candidates
            SET status = 'accepted',
                updated_at = NOW(),
                accepted_at = COALESCE(accepted_at, NOW()),
                rejected_at = NULL
            WHERE project_scope = $1 AND id = $2
            RETURNING *
            "#,
        )
        .bind(project_scope)
        .bind(candidate_id)
        .fetch_one(&mut *tx)
        .await?;

        let entry = sqlx::query_as::<_, MemoryEntry>(
            r#"
            INSERT INTO memory_entries (
                id,
                project_scope,
                layer,
                content,
                source_candidate_id,
                source_type,
                source_ref,
                proposer,
                status,
                created_at,
                updated_at,
                invalidated_at
            )
            VALUES (
                gen_random_uuid(),
                $1,
                $2,
                $3,
                $4,
                'candidate_acceptance',
                $5,
                $6,
                $7,
                NOW(),
                NOW(),
                NULL
            )
            RETURNING *
            "#,
        )
        .bind(project_scope)
        .bind(&accepted_candidate.layer)
        .bind(&accepted_candidate.proposed_fact)
        .bind(candidate_id)
        .bind(format!("memory-candidate://{}", candidate_id))
        .bind(accepted_candidate.proposer.clone())
        .bind(MemoryEntryStatus::Active)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(Some((accepted_candidate, entry)))
    }

    async fn list_entries(
        &self,
        project_scope: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let rows = sqlx::query_as::<_, MemoryEntry>(
            r#"
            SELECT *
            FROM memory_entries
            WHERE project_scope = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(project_scope)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows)
    }

    async fn search_entries(
        &self,
        project_scope: &str,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let candidate_limit = std::cmp::max(
            limit.saturating_mul(5),
            100,
        );
        let mut rows = sqlx::query_as::<_, MemoryEntry>(
            r#"
            SELECT *
            FROM memory_entries
            WHERE project_scope = $1
              AND status = 'active'
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(project_scope)
        .bind(candidate_limit)
        .fetch_all(self.db.pool())
        .await?;

        let query_terms: Vec<String> = query
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter_map(|part| {
                let term = part.trim().to_ascii_lowercase();
                if term.len() >= 3 {
                    Some(term)
                } else {
                    None
                }
            })
            .collect();

        rows.sort_by(|a, b| {
            let a_score = recall_match_score(&a.content, &query_terms);
            let b_score = recall_match_score(&b.content, &query_terms);
            b_score
                .cmp(&a_score)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.id.cmp(&a.id))
        });

        rows.truncate(std::cmp::max(limit, 0) as usize);
        Ok(rows)
    }

    async fn get_entry_by_id(
        &self,
        project_scope: &str,
        id: Uuid,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        let row = sqlx::query_as::<_, MemoryEntry>(
            r#"
            SELECT *
            FROM memory_entries
            WHERE project_scope = $1 AND id = $2
            "#,
        )
        .bind(project_scope)
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn update_entry_status(
        &self,
        project_scope: &str,
        id: Uuid,
        status: MemoryEntryStatus,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        let row = sqlx::query_as::<_, MemoryEntry>(
            r#"
            UPDATE memory_entries
            SET status = $1,
                updated_at = NOW(),
                invalidated_at = CASE
                    WHEN $1::TEXT = 'invalidated' THEN COALESCE(invalidated_at, NOW())
                    ELSE NULL
                END
            WHERE project_scope = $2 AND id = $3
            RETURNING *
            "#,
        )
        .bind(status)
        .bind(project_scope)
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row)
    }
}

fn recall_match_score(content: &str, query_terms: &[String]) -> usize {
    if query_terms.is_empty() {
        return 0;
    }

    let haystack = content.to_ascii_lowercase();
    query_terms
        .iter()
        .filter(|term| haystack.contains(term.as_str()))
        .count()
}
