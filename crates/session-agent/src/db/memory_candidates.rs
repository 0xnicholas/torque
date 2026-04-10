use crate::models::{MemoryCandidate, MemoryCandidateStatus};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create(pool: &PgPool, candidate: &MemoryCandidate) -> anyhow::Result<MemoryCandidate> {
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
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn get_by_id(
    pool: &PgPool,
    project_scope: &str,
    id: Uuid,
) -> anyhow::Result<Option<MemoryCandidate>> {
    let row = sqlx::query_as::<_, MemoryCandidate>(
        r#"
        SELECT *
        FROM memory_candidates
        WHERE project_scope = $1 AND id = $2
        "#,
    )
    .bind(project_scope)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn list_by_project_scope(
    pool: &PgPool,
    project_scope: &str,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<MemoryCandidate>> {
    let rows = sqlx::query_as::<_, MemoryCandidate>(
        r#"
        SELECT *
        FROM memory_candidates
        WHERE project_scope = $1
        ORDER BY created_at DESC, id DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(project_scope)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_status(
    pool: &PgPool,
    project_scope: &str,
    id: Uuid,
    status: MemoryCandidateStatus,
) -> anyhow::Result<Option<MemoryCandidate>> {
    let row = sqlx::query_as::<_, MemoryCandidate>(
        r#"
        UPDATE memory_candidates
        SET status = $1,
            updated_at = NOW(),
            accepted_at = CASE WHEN $1::TEXT = 'accepted' THEN NOW() ELSE accepted_at END,
            rejected_at = CASE WHEN $1::TEXT = 'rejected' THEN NOW() ELSE rejected_at END
        WHERE project_scope = $2 AND id = $3
        RETURNING *
        "#,
    )
    .bind(status)
    .bind(project_scope)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
