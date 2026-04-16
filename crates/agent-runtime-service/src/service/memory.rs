use crate::repository::MemoryRepository;
use std::sync::Arc;

pub struct MemoryService {
    _repo: Arc<dyn MemoryRepository>,
}

impl MemoryService {
    pub fn new(_repo: Arc<dyn MemoryRepository>) -> Self {
        todo!("implemented in Task 4.2")
    }
}
