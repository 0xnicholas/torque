use crate::infra::tool_registry::ToolRegistry;
use std::sync::Arc;

pub struct ToolService;

impl ToolService {
    pub async fn new() -> Self {
        todo!("implemented in Task 2.1")
    }

    pub fn registry(&self) -> Arc<ToolRegistry> {
        todo!("implemented in Task 2.1")
    }
}
