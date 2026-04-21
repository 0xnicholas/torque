use sqlx::{PgPool, Row};
use types::{Run, RunStatus};
use uuid::Uuid;

pub async fn create(pool: &PgPool, run: &Run) -> Result<Run, sqlx::Error> {
    let row = sqlx::query(
        r#"
        INSERT INTO runs (id, tenant_id, status, instruction, failure_policy, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#
    )
    .bind(run.id)
    .bind(run.tenant_id)
    .bind(run.status.to_string())
    .bind(&run.instruction)
    .bind(&run.failure_policy)
    .bind(run.created_at)
    .fetch_one(pool)
    .await?;
    
    Ok(Run {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        status: match row.get::<String, _>("status").as_str() {
            "planning" => RunStatus::Planning,
            "pending" => RunStatus::Pending,
            "running" => RunStatus::Running,
            "done" => RunStatus::Done,
            "failed" => RunStatus::Failed,
            "planning_failed" => RunStatus::PlanningFailed,
            _ => RunStatus::Planning,
        },
        instruction: row.get("instruction"),
        failure_policy: row.get("failure_policy"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        error: row.get("error"),
    })
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Run>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM runs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    
    if let Some(row) = row {
        Ok(Some(Run {
            id: row.get("id"),
            tenant_id: row.get("tenant_id"),
            status: match row.get::<String, _>("status").as_str() {
                "planning" => RunStatus::Planning,
                "pending" => RunStatus::Pending,
                "running" => RunStatus::Running,
                "done" => RunStatus::Done,
                "failed" => RunStatus::Failed,
                "planning_failed" => RunStatus::PlanningFailed,
                _ => RunStatus::Planning,
            },
            instruction: row.get("instruction"),
            failure_policy: row.get("failure_policy"),
            created_at: row.get("created_at"),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
            error: row.get("error"),
        }))
    } else {
        Ok(None)
    }
}

pub async fn update_status(pool: &PgPool, id: Uuid, status: RunStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE runs SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status.to_string())
        .execute(pool)
        .await?;
    Ok(())
}
