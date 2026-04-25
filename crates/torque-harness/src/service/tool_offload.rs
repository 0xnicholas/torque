use crate::models::v1::artifact::ArtifactScope;
use crate::service::{ArtifactService, RoutedVfs};
use crate::tools::ToolResult;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

pub const TOOL_OUTPUT_ARTIFACT_KIND: &str = "tool_output";
const TEXT_MIME_TYPE: &str = "text/plain";

#[derive(Debug, Clone, Copy)]
pub struct ToolOffloadConfig {
    pub inline_max_bytes: usize,
    pub scratch_max_bytes: usize,
}

impl Default for ToolOffloadConfig {
    fn default() -> Self {
        Self {
            inline_max_bytes: 4 * 1024,
            scratch_max_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OffloadedToolResult {
    pub tool_name: String,
    pub offloaded: bool,
    pub storage: Option<&'static str>,
    pub path: Option<String>,
    pub artifact_id: Option<Uuid>,
    pub preview: String,
    pub bytes: usize,
}

impl OffloadedToolResult {
    pub fn into_tool_result(self) -> ToolResult {
        if !self.offloaded {
            return ToolResult {
                success: true,
                content: self.preview,
                error: None,
            };
        }

        let location = self
            .path
            .clone()
            .unwrap_or_else(|| self.artifact_id.map(|id| id.to_string()).unwrap_or_default());
        let storage = self.storage.unwrap_or("inline");

        ToolResult {
            success: true,
            content: format!(
                "Tool result offloaded to {}: {} ({} bytes)\nPreview: {}",
                storage, location, self.bytes, self.preview
            ),
            error: None,
        }
    }
}

pub struct ToolOffloadService {
    artifact_service: Option<Arc<ArtifactService>>,
    vfs: Option<Arc<RoutedVfs>>,
    config: ToolOffloadConfig,
}

impl ToolOffloadService {
    pub fn new(
        artifact_service: Option<Arc<ArtifactService>>,
        vfs: Option<Arc<RoutedVfs>>,
    ) -> Self {
        Self {
            artifact_service,
            vfs,
            config: ToolOffloadConfig::default(),
        }
    }

    pub fn with_config(mut self, config: ToolOffloadConfig) -> Self {
        self.config = config;
        self
    }

    pub async fn offload(
        &self,
        tool_name: &str,
        result: ToolResult,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<ToolResult> {
        if !result.success || result.content.is_empty() {
            return Ok(result);
        }

        let bytes = result.content.len();
        let preview = preview(&result.content);

        if bytes <= self.config.inline_max_bytes {
            return Ok(result);
        }

        if bytes <= self.config.scratch_max_bytes {
            if let Some(vfs) = &self.vfs {
                let path = format!("/scratch/tool-results/{}-{}.txt", sanitize(tool_name), Uuid::new_v4());
                vfs.write(&path, &result.content).await?;
                return Ok(OffloadedToolResult {
                    tool_name: tool_name.to_string(),
                    offloaded: true,
                    storage: Some("scratch"),
                    path: Some(path),
                    artifact_id: None,
                    preview,
                    bytes,
                }
                .into_tool_result());
            }
        }

        if let Some(artifact_service) = &self.artifact_service {
            let artifact = artifact_service
                .create_with_source_instance(
                    TOOL_OUTPUT_ARTIFACT_KIND,
                    ArtifactScope::Private,
                    TEXT_MIME_TYPE,
                    serde_json::json!({ "tool_name": tool_name, "content": result.content }),
                    source_instance_id,
                )
                .await?;
            return Ok(OffloadedToolResult {
                tool_name: tool_name.to_string(),
                offloaded: true,
                storage: Some("artifact"),
                path: None,
                artifact_id: Some(artifact.id),
                preview,
                bytes,
            }
            .into_tool_result());
        }

        Ok(result)
    }
}

fn preview(content: &str) -> String {
    const MAX_PREVIEW: usize = 160;
    if content.len() <= MAX_PREVIEW {
        content.to_string()
    } else {
        format!("{}...", &content[..MAX_PREVIEW])
    }
}

fn sanitize(tool_name: &str) -> String {
    tool_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' })
        .collect()
}
