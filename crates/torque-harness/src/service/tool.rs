use crate::infra::tool_registry::ToolRegistry;
use crate::service::ArtifactService;
use crate::service::vfs::RoutedVfs;
use crate::tools::builtin::create_builtin_tools;
use futures::executor::block_on;
use std::sync::Arc;

pub struct ToolService {
    registry: Arc<ToolRegistry>,
}

impl ToolService {
    pub fn new() -> Self {
        let registry = Arc::new(ToolRegistry::new());
        Self { registry }
    }

    pub fn new_with_builtins(artifact_service: Arc<ArtifactService>) -> Self {
        let service = Self::new();
        let vfs = Arc::new(RoutedVfs::for_current_workspace());
        for tool in create_builtin_tools(artifact_service, vfs) {
            block_on(service.registry.register(tool));
        }
        service
    }

    pub fn registry(&self) -> Arc<ToolRegistry> {
        self.registry.clone()
    }
}
