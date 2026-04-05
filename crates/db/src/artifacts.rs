use sqlx::{PgPool, Row};
use types::{Artifact, StorageType};
use uuid::Uuid;

pub async fn create(pool: &PgPool, artifact: &Artifact) -> Result<Artifact, sqlx::Error> {
    let row = sqlx::query(
        r#"
        INSERT INTO artifacts (id, node_id, tenant_id, storage, location, size_bytes, content_type, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#
    )
    .bind(artifact.id)
    .bind(artifact.node_id)
    .bind(artifact.tenant_id)
    .bind(artifact.storage.to_string())
    .bind(&artifact.location)
    .bind(artifact.size_bytes)
    .bind(&artifact.content_type)
    .bind(artifact.created_at)
    .fetch_one(pool)
    .await?;
    
    Ok(Artifact {
        id: row.get("id"),
        node_id: row.get("node_id"),
        tenant_id: row.get("tenant_id"),
        storage: match row.get::<String, _>("storage").as_str() {
            "redis" => StorageType::Redis,
            "s3" => StorageType::S3,
            _ => StorageType::Redis,
        },
        location: row.get("location"),
        size_bytes: row.get("size_bytes"),
        content_type: row.get("content_type"),
        created_at: row.get("created_at"),
    })
}

pub async fn get_by_node(pool: &PgPool, node_id: Uuid) -> Result<Vec<Artifact>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM artifacts WHERE node_id = $1")
        .bind(node_id)
        .fetch_all(pool)
        .await?;
    
    Ok(rows.into_iter().map(|row| Artifact {
        id: row.get("id"),
        node_id: row.get("node_id"),
        tenant_id: row.get("tenant_id"),
        storage: match row.get::<String, _>("storage").as_str() {
            "redis" => StorageType::Redis,
            "s3" => StorageType::S3,
            _ => StorageType::Redis,
        },
        location: row.get("location"),
        size_bytes: row.get("size_bytes"),
        content_type: row.get("content_type"),
        created_at: row.get("created_at"),
    }).collect())
}
