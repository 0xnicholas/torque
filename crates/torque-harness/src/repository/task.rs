use crate::db::Database;
use crate::models::v1::task::{Task, TaskStatus, TaskType};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(
        &self,
        task_type: TaskType,
        goal: &str,
        instructions: Option<&str>,
        agent_instance_id: Option<Uuid>,
        input_artifacts: serde_json::Value,
    ) -> anyhow::Result<Task>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Task>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Task>>;
    async fn update_status(&self, id: Uuid, status: TaskStatus) -> anyhow::Result<bool>;
    async fn cancel(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn update_produced_artifacts(
        &self,
        id: Uuid,
        artifacts: serde_json::Value,
    ) -> anyhow::Result<bool>;
    async fn list_by_team(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Task>>;
}

pub struct PostgresTaskRepository {
    db: Database,
}

impl PostgresTaskRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn create(
        &self,
        task_type: TaskType,
        goal: &str,
        instructions: Option<&str>,
        agent_instance_id: Option<Uuid>,
        input_artifacts: serde_json::Value,
    ) -> anyhow::Result<Task> {
        let row = sqlx::query_as::<_, Task>(
            "INSERT INTO v1_tasks (task_type, status, goal, instructions, agent_instance_id, input_artifacts) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
        )
        .bind(task_type)
        .bind(TaskStatus::Created)
        .bind(goal)
        .bind(instructions)
        .bind(agent_instance_id)
        .bind(input_artifacts)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Task>> {
        let rows =
            sqlx::query_as::<_, Task>("SELECT * FROM v1_tasks ORDER BY created_at DESC LIMIT $1")
                .bind(limit)
                .fetch_all(self.db.pool())
                .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Task>> {
        let row = sqlx::query_as::<_, Task>("SELECT * FROM v1_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn update_status(&self, id: Uuid, status: TaskStatus) -> anyhow::Result<bool> {
        // Validate state transition
        let current = self.get(id).await?;
        if let Some(ref task) = current {
            if !task.status.can_transition_to(&status) {
                return Err(anyhow::anyhow!(
                    "Invalid task state transition: {:?} -> {:?}",
                    task.status,
                    status
                ));
            }
        }

        let result =
            sqlx::query("UPDATE v1_tasks SET status = $1, updated_at = NOW() WHERE id = $2")
                .bind(status)
                .bind(id)
                .execute(self.db.pool())
                .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn cancel(&self, id: Uuid) -> anyhow::Result<bool> {
        self.update_status(id, TaskStatus::Cancelled).await
    }

    async fn update_produced_artifacts(
        &self,
        id: Uuid,
        artifacts: serde_json::Value,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_tasks SET produced_artifacts = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(artifacts)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn list_by_team(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, Task>(
            "SELECT * FROM v1_tasks WHERE team_instance_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(team_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}
