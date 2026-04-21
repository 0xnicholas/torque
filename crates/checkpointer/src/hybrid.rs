use async_trait::async_trait;
use chrono::Utc;
use sha2::{Sha256, Digest};
use uuid::Uuid;

use crate::error::{CheckpointerError, Result};
use crate::r#trait::{CheckpointId, CheckpointMeta, CheckpointState, Checkpointer};

pub struct HybridCheckpointer {
    pool: sqlx::PgPool,
    redis: redis::aio::ConnectionManager,
    tenant_id: Uuid,
}

impl HybridCheckpointer {
    pub fn new(pool: sqlx::PgPool, redis: redis::aio::ConnectionManager, tenant_id: Uuid) -> Self {
        Self { pool, redis, tenant_id }
    }

    fn compute_hash(state: &CheckpointState) -> String {
        let mut hasher = Sha256::new();
        let state_json = serde_json::to_string(state).unwrap_or_default();
        hasher.update(state_json);
        format!("{:x}", hasher.finalize())
    }

    fn redis_key(&self, checkpoint_id: &CheckpointId) -> String {
        format!("{}:checkpoint:{}", self.tenant_id, checkpoint_id.0)
    }
}

#[async_trait]
impl Checkpointer for HybridCheckpointer {
    async fn save(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        state: CheckpointState,
    ) -> Result<CheckpointId> {
        let checkpoint_id = CheckpointId::new();
        let state_hash = Self::compute_hash(&state);
        let redis_key = self.redis_key(&checkpoint_id);

        let state_json = serde_json::to_string(&state).map_err(|e| {
            CheckpointerError::Serialization(e.to_string())
        })?;

        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, run_id, node_id, tenant_id, state_hash, storage, location, created_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, 'pending', $6, NOW(), NOW() + INTERVAL '24 hours')
            "#,
        )
        .bind(checkpoint_id.0)
        .bind(run_id)
        .bind(node_id)
        .bind(self.tenant_id)
        .bind(&state_hash)
        .bind(format!("pending:{}", redis_key))
        .execute(&self.pool)
        .await?;

        let mut conn = self.redis.clone();
        if let Err(e) = redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(86400)
            .arg(&state_json)
            .query_async::<_, ()>(&mut conn)
            .await
        {
            sqlx::query("DELETE FROM checkpoints WHERE id = $1")
                .bind(checkpoint_id.0)
                .execute(&self.pool)
                .await?;
            return Err(CheckpointerError::Redis(e));
        }

        sqlx::query(
            r#"UPDATE checkpoints SET storage = 'redis', location = $1 WHERE id = $2"#,
        )
        .bind(&redis_key)
        .bind(checkpoint_id.0)
        .execute(&self.pool)
        .await?;

        Ok(checkpoint_id)
    }

    async fn load(&self, checkpoint_id: CheckpointId) -> Result<CheckpointState> {
        let redis_key = self.redis_key(&checkpoint_id);
        let mut conn = self.redis.clone();

        let state_json: Option<String> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;

        match state_json {
            Some(json) => {
                serde_json::from_str(&json).map_err(|e| {
                    CheckpointerError::Serialization(e.to_string())
                })
            }
            None => Err(CheckpointerError::NotFound(checkpoint_id.0.to_string())),
        }
    }

    async fn list_run_checkpoints(&self, run_id: Uuid) -> Result<Vec<CheckpointMeta>> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, chrono::DateTime<Utc>, String)>(
            "SELECT id, run_id, node_id, created_at, state_hash FROM checkpoints WHERE run_id = $1 ORDER BY created_at DESC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, run_id, node_id, created_at, state_hash)| CheckpointMeta {
                id: CheckpointId(id),
                run_id,
                node_id,
                created_at,
                state_hash,
            })
            .collect())
    }

    async fn list_node_checkpoints(&self, node_id: Uuid) -> Result<Vec<CheckpointMeta>> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, chrono::DateTime<Utc>, String)>(
            "SELECT id, run_id, node_id, created_at, state_hash FROM checkpoints WHERE node_id = $1 ORDER BY created_at DESC",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, run_id, node_id, created_at, state_hash)| CheckpointMeta {
                id: CheckpointId(id),
                run_id,
                node_id,
                created_at,
                state_hash,
            })
            .collect())
    }

    async fn delete(&self, checkpoint_id: CheckpointId) -> Result<()> {
        let redis_key = self.redis_key(&checkpoint_id);

        sqlx::query("DELETE FROM checkpoints WHERE id = $1")
            .bind(checkpoint_id.0)
            .execute(&self.pool)
            .await?;

        let mut conn = self.redis.clone();
        let _: () = redis::cmd("DEL")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }
}