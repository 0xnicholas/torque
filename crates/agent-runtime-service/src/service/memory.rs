use crate::models::{MemoryCandidate, MemoryEntry, MemoryEntryStatus};
use crate::repository::MemoryRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct MemoryService {
    repo: Arc<dyn MemoryRepository>,
}

impl MemoryService {
    pub fn new(repo: Arc<dyn MemoryRepository>) -> Self {
        Self { repo }
    }

    pub fn repo(&self) -> &Arc<dyn MemoryRepository> {
        &self.repo
    }

    pub async fn create_candidate(
        &self,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryCandidate> {
        self.repo.create_candidate(candidate).await
    }

    pub async fn accept_candidate(
        &self,
        project_scope: &str,
        candidate_id: Uuid,
    ) -> anyhow::Result<Option<(MemoryCandidate, MemoryEntry)>> {
        self.repo.accept_candidate_to_entry(project_scope, candidate_id).await
    }

    pub async fn create_entry(
        &self,
        entry: &MemoryEntry,
    ) -> anyhow::Result<MemoryEntry> {
        self.repo.create_entry(entry).await
    }

    pub async fn list_entries(
        &self,
        project_scope: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.repo.list_entries(project_scope, limit, offset).await
    }

    pub async fn search_entries(
        &self,
        project_scope: &str,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.repo.search_entries(project_scope, query, limit).await
    }

    pub async fn get_entry_by_id(
        &self,
        project_scope: &str,
        id: Uuid,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        self.repo.get_entry_by_id(project_scope, id).await
    }

    pub async fn update_entry_status(
        &self,
        project_scope: &str,
        id: Uuid,
        status: MemoryEntryStatus,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        self.repo.update_entry_status(project_scope, id, status).await
    }
}
