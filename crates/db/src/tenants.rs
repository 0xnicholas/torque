use sqlx::{PgPool, Row};
use types::Tenant;
use uuid::Uuid;

pub async fn list(pool: &PgPool) -> Result<Vec<Tenant>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM tenants ORDER BY created_at")
        .fetch_all(pool)
        .await?;
    
    Ok(rows.into_iter().map(row_to_tenant).collect())
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Tenant>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM tenants WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    
    Ok(row.map(row_to_tenant))
}

fn row_to_tenant(row: sqlx::postgres::PgRow) -> Tenant {
    Tenant {
        id: row.get("id"),
        name: row.get("name"),
        weight: row.get("weight"),
        max_concurrency: row.get("max_concurrency"),
        monthly_token_quota: row.get("monthly_token_quota"),
        created_at: row.get("created_at"),
    }
}