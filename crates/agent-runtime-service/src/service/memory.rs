use crate::repository::MemoryRepository;
use std::sync::Arc;

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
}
