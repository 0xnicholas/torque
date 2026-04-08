use sqlx::{PgPool, Postgres, Transaction};

pub mod messages;
pub mod sessions;

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn begin(&self) -> anyhow::Result<Transaction<'_, Postgres>> {
        Ok(self.pool.begin().await?)
    }
}