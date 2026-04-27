pub use torque_runtime::offload::{ToolOffloadConfig, ToolOffloadPolicy};

use async_trait::async_trait;
use std::sync::Arc;
use torque_runtime::offload::{OffloadArtifactRef, OffloadArtifactStore};
use uuid::Uuid;

use crate::models::v1::artifact::ArtifactScope;
use crate::service::ArtifactService;

pub const TOOL_OUTPUT_ARTIFACT_KIND: &str = "tool_output";
const TEXT_MIME_TYPE: &str = "text/plain";

pub struct HarnessOffloadArtifactStore {
    artifact_service: Arc<ArtifactService>,
}

impl HarnessOffloadArtifactStore {
    pub fn new(artifact_service: Arc<ArtifactService>) -> Self {
        Self { artifact_service }
    }
}

#[async_trait]
impl OffloadArtifactStore for HarnessOffloadArtifactStore {
    async fn write_tool_output(
        &self,
        _tool_name: &str,
        content: serde_json::Value,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<OffloadArtifactRef> {
        let artifact = self
            .artifact_service
            .create_with_source_instance(
                TOOL_OUTPUT_ARTIFACT_KIND,
                ArtifactScope::Private,
                TEXT_MIME_TYPE,
                content,
                source_instance_id,
            )
            .await?;
        Ok(OffloadArtifactRef {
            artifact_id: artifact.id,
        })
    }
}
