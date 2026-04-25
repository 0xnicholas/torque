use crate::service::ArtifactService;
use crate::tools::builtin::create_builtin_tools;
use std::sync::Arc;

pub use crate::infra::tool_registry::ToolRegistry;
pub use crate::tools::ToolResult;

pub async fn register_builtin_tools(
    registry: &ToolRegistry,
    artifact_service: Arc<ArtifactService>,
) {
    for tool in create_builtin_tools(artifact_service) {
        registry.register(tool).await;
    }
}
