use async_trait::async_trait;
use crate::db::Database;
use crate::models::{MemoryCandidate, MemoryEntry};
use uuid::Uuid;

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn create_candidate(
        &self,
        session_id: Uuid,
        entries: &[MemoryEntry],
    ) -> anyhow::Result<MemoryCandidate>;
    async fn accept_candidate(
        &self,
        candidate_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
    async fn list_entries(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
    async fn search_entries(
        &self,
        session_id: Uuid,
        query: &str,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
}

#[allow(dead_code)]
pub struct PostgresMemoryRepository {
    db: Database,
}

impl PostgresMemoryRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MemoryRepository for PostgresMemoryRepository {
    async fn create_candidate(
        &self,
        _session_id: Uuid,
        _entries: &[MemoryEntry],
    ) -> anyhow::Result<MemoryCandidate> {
        todo!("migrate from db/memory_candidates.rs")
    }

    async fn accept_candidate(
        &self,
        _candidate_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        todo!("migrate from db/memory_candidates.rs")
    }

    async fn list_entries(
        &self,
        _session_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        todo!("migrate from db/memory_entries.rs")
    }

    async fn search_entries(
        &self,
        _session_id: Uuid,
        _query: &str,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        todo!("migrate from db/memory_entries.rs")
    }
}
