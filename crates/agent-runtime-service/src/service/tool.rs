use crate::infra::tool_registry::ToolRegistry;
use std::sync::Arc;

pub struct ToolService {
    registry: Arc<ToolRegistry>,
}

impl ToolService {
    pub async fn new() -> Self {
        let registry = Arc::new(ToolRegistry::new());
        Self { registry }
    }

    pub fn registry(&self) -> Arc<ToolRegistry> {
        self.registry.clone()
    }
}
