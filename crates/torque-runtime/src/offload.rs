use async_trait::async_trait;
use crate::tools::RuntimeToolResult;
use crate::vfs::VfsBackend;
use uuid::Uuid;

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

#[derive(Debug, Clone)]
pub struct OffloadArtifactRef {
    pub artifact_id: Uuid,
}

#[async_trait]
pub trait OffloadArtifactStore: Send + Sync {
    async fn write_tool_output(
        &self,
        tool_name: &str,
        content: serde_json::Value,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<OffloadArtifactRef>;
}

pub struct ToolOffloadPolicy {
    vfs: Option<std::sync::Arc<dyn VfsBackend>>,
    artifact_store: Option<std::sync::Arc<dyn OffloadArtifactStore>>,
    config: ToolOffloadConfig,
}

impl ToolOffloadPolicy {
    pub fn new(
        vfs: Option<std::sync::Arc<dyn VfsBackend>>,
        artifact_store: Option<std::sync::Arc<dyn OffloadArtifactStore>>,
    ) -> Self {
        Self {
            vfs,
            artifact_store,
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
        result: RuntimeToolResult,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<RuntimeToolResult> {
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
                let path = format!(
                    "/scratch/tool-results/{}-{}.txt",
                    sanitize(tool_name),
                    Uuid::new_v4(),
                );
                vfs.as_ref().write(&path, &result.content).await?;
                let offload_ref = crate::tools::RuntimeArtifactRef {
                    storage: "scratch".to_string(),
                    locator: path.clone(),
                    artifact_id: None,
                };
                let content = format!(
                    "Tool result offloaded to scratch: {} ({} bytes)\nPreview: {}",
                    path, bytes, preview,
                );
                return Ok(RuntimeToolResult {
                    success: true,
                    content,
                    error: None,
                    offload_ref: Some(offload_ref),
                });
            }
        }

        if let Some(artifact_store) = &self.artifact_store {
            let artifact_ref = artifact_store.as_ref()
                .write_tool_output(
                    tool_name,
                    serde_json::json!({ "tool_name": tool_name, "content": result.content }),
                    source_instance_id,
                )
                .await?;
            let offload_ref = crate::tools::RuntimeArtifactRef {
                storage: "artifact".to_string(),
                locator: artifact_ref.artifact_id.to_string(),
                artifact_id: Some(artifact_ref.artifact_id),
            };
            let content = format!(
                "Tool result offloaded to artifact: {} ({} bytes)\nPreview: {}",
                artifact_ref.artifact_id, bytes, preview,
            );
            return Ok(RuntimeToolResult {
                success: true,
                content,
                error: None,
                offload_ref: Some(offload_ref),
            });
        }

        Ok(result)
    }
}

fn preview(content: &str) -> String {
    const MAX_PREVIEW: usize = 160;
    let char_count = content.chars().count();
    if char_count <= MAX_PREVIEW {
        content.to_string()
    } else {
        let truncated: String = content.chars().take(MAX_PREVIEW).collect();
        format!("{}...", truncated)
    }
}

fn sanitize(tool_name: &str) -> String {
    tool_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}
