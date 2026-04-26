use crate::infra::tool_registry::ToolRegistry;
use crate::service::ArtifactService;
use crate::service::tool_offload::HarnessOffloadArtifactStore;
use crate::service::vfs::RoutedVfs;
use crate::tools::builtin::create_builtin_tools;
use futures::executor::block_on;
use std::sync::Arc;

pub struct ToolService {
    registry: Arc<ToolRegistry>,
    artifact_service: Option<Arc<ArtifactService>>,
    vfs: Option<Arc<RoutedVfs>>,
}

impl ToolService {
    pub fn new() -> Self {
        let registry = Arc::new(ToolRegistry::new());
        Self {
            registry,
            artifact_service: None,
            vfs: None,
        }
    }

    pub fn new_with_builtins(artifact_service: Arc<ArtifactService>) -> Self {
        let vfs = Arc::new(RoutedVfs::for_current_workspace());
        let service = Self {
            registry: Arc::new(ToolRegistry::new()),
            artifact_service: Some(artifact_service.clone()),
            vfs: Some(vfs.clone()),
        };
        for tool in create_builtin_tools(artifact_service, vfs) {
            block_on(service.registry.register(tool));
        }
        service
    }

    pub fn registry(&self) -> Arc<ToolRegistry> {
        self.registry.clone()
    }

    pub fn artifact_service(&self) -> Option<Arc<ArtifactService>> {
        self.artifact_service.clone()
    }

    pub fn vfs(&self) -> Option<Arc<RoutedVfs>> {
        self.vfs.clone()
    }

    pub fn tool_offload_service(&self) -> torque_runtime::offload::ToolOffloadPolicy {
        let vfs: Option<std::sync::Arc<dyn torque_runtime::vfs::VfsBackend>> =
            self.vfs().map(|v| v as std::sync::Arc<dyn torque_runtime::vfs::VfsBackend>);
        let artifact_store: Option<std::sync::Arc<dyn torque_runtime::offload::OffloadArtifactStore>> =
            self.artifact_service()
                .map(|a| std::sync::Arc::new(HarnessOffloadArtifactStore::new(a))
                    as std::sync::Arc<dyn torque_runtime::offload::OffloadArtifactStore>);
        torque_runtime::offload::ToolOffloadPolicy::new(vfs, artifact_store)
    }
}
