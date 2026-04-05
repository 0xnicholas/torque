use sqlx::{PgPool, Row};
use types::{Node, NodeStatus};
use uuid::Uuid;

pub async fn create(pool: &PgPool, node: &Node) -> Result<Node, sqlx::Error> {
    let row = sqlx::query(
        r#"
        INSERT INTO nodes (id, run_id, tenant_id, agent_type, instruction, status)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#
    )
    .bind(node.id)
    .bind(node.run_id)
    .bind(node.tenant_id)
    .bind(&node.agent_type)
    .bind(&node.instruction)
    .bind(node.status.to_string())
    .fetch_one(pool)
    .await?;
    
    Ok(row_to_node(row))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Node>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM nodes WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    
    if let Some(row) = row {
        Ok(Some(row_to_node(row)))
    } else {
        Ok(None)
    }
}

pub async fn update_status(pool: &PgPool, id: Uuid, status: NodeStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE nodes SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_by_run(pool: &PgPool, run_id: Uuid) -> Result<Vec<Node>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM nodes WHERE run_id = $1")
        .bind(run_id)
        .fetch_all(pool)
        .await?;
    
    Ok(rows.into_iter().map(row_to_node).collect())
}

fn row_to_node(row: sqlx:: postgres::PgRow) -> Node {
    Node {
        id: row.get("id"),
        run_id: row.get("run_id"),
        tenant_id: row.get("tenant_id"),
        agent_type: row.get("agent_type"),
        fallback_agent_type: row.get("fallback_agent_type"),
        instruction: row.get("instruction"),
        tools: row.get("tools"),
        failure_policy: row.get("failure_policy"),
        requires_approval: row.get("requires_approval"),
        status: match row.get::<String, _>("status").as_str() {
            "pending" => NodeStatus::Pending,
            "running" => NodeStatus::Running,
            "done" => NodeStatus::Done,
            "failed" => NodeStatus::Failed,
            "skipped" => NodeStatus::Skipped,
            "pending_approval" => NodeStatus::PendingApproval,
            "cancelled" => NodeStatus::Cancelled,
            _ => NodeStatus::Pending,
        },
        layer: row.get("layer"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        retry_count: row.get("retry_count"),
        error: row.get("error"),
        executor_id: row.get("executor_id"),
    }
}
