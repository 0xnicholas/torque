use crate::models::{MemoryCandidate, MemoryCandidateStatus, MemoryEntry, MemoryEntryStatus};
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
            accepted_at = CASE
                WHEN $1::TEXT = 'accepted' THEN COALESCE(accepted_at, NOW())
                ELSE NULL
            END,
            rejected_at = CASE
                WHEN $1::TEXT = 'rejected' THEN COALESCE(rejected_at, NOW())
                ELSE NULL
            END
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

pub async fn accept_candidate_to_entry(
    pool: &PgPool,
    project_scope: &str,
    candidate_id: Uuid,
) -> anyhow::Result<Option<(MemoryCandidate, MemoryEntry)>> {
    let mut tx = pool.begin().await?;

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
