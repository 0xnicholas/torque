use anyhow::ensure;
use crate::models::{MemoryEntry, MemoryEntryStatus};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create(pool: &PgPool, entry: &MemoryEntry) -> anyhow::Result<MemoryEntry> {
    if let Some(source_candidate_id) = entry.source_candidate_id {
        let candidate_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM memory_candidates
                WHERE project_scope = $1 AND id = $2
            )
            "#,
        )
        .bind(&entry.project_scope)
        .bind(source_candidate_id)
        .fetch_one(pool)
        .await?;

        ensure!(
            candidate_exists,
            "memory entry source_candidate_id must reference a candidate in the same project_scope"
        );
    }

    let row = sqlx::query_as::<_, MemoryEntry>(
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING *
        "#,
    )
    .bind(entry.id)
    .bind(&entry.project_scope)
    .bind(&entry.layer)
    .bind(&entry.content)
    .bind(entry.source_candidate_id)
    .bind(&entry.source_type)
    .bind(&entry.source_ref)
    .bind(&entry.proposer)
    .bind(&entry.status)
    .bind(entry.created_at)
    .bind(entry.updated_at)
    .bind(entry.invalidated_at)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn get_by_id(
    pool: &PgPool,
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
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn list_by_project_scope(
    pool: &PgPool,
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
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_status(
    pool: &PgPool,
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
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
