use sqlx::{PgPool, Row};
use types::Edge;
use uuid::Uuid;

pub async fn create(pool: &PgPool, edge: &Edge) -> Result<Edge, sqlx::Error> {
    let row = sqlx::query(
        r#"
        INSERT INTO edges (id, run_id, source_node, target_node)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#
    )
    .bind(edge.id)
    .bind(edge.run_id)
    .bind(edge.source_node)
    .bind(edge.target_node)
    .fetch_one(pool)
    .await?;
    
    Ok(Edge {
        id: row.get("id"),
        run_id: row.get("run_id"),
        source_node: row.get("source_node"),
        target_node: row.get("target_node"),
    })
}

pub async fn get_by_run(pool: &PgPool, run_id: Uuid) -> Result<Vec<Edge>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM edges WHERE run_id = $1")
        .bind(run_id)
        .fetch_all(pool)
        .await?;
    
    Ok(rows.into_iter().map(|row| Edge {
        id: row.get("id"),
        run_id: row.get("run_id"),
        source_node: row.get("source_node"),
        target_node: row.get("target_node"),
    }).collect())
}

pub async fn get_upstream_deps(pool: &PgPool, node_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows = sqlx::query("SELECT source_node FROM edges WHERE target_node = $1")
        .bind(node_id)
        .fetch_all(pool)
        .await?;
    
    Ok(rows.into_iter().map(|row| row.get("source_node")).collect())
}

pub async fn get_downstream_nodes(pool: &PgPool, node_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows = sqlx::query("SELECT target_node FROM edges WHERE source_node = $1")
        .bind(node_id)
        .fetch_all(pool)
        .await?;
    
    Ok(rows.into_iter().map(|row| row.get("target_node")).collect())
}
