use std::pin::Pin;
use std::sync::Arc;
use futures::Future;
use serde::Deserialize;

use context_store::VirtualFileSystem;
use tool_executor::registry::ToolRegistry;
use tool_executor::ToolResult;

#[derive(Debug, Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ListArgs {
    pub dir: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileArgs {
    pub path: String,
}

pub fn register_vfs_tools(registry: &mut ToolRegistry, vfs: Arc<dyn VirtualFileSystem>) {
    registry.register("read_file", move |args: serde_json::Value| {
        let vfs = vfs.clone();
        Box::pin(async move {
            let args: ReadFileArgs = serde_json::from_value(args)
                .map_err(|e| format!("Invalid args: {}", e))?;
            let content = vfs.read(&args.path).await
                .map_err(|e| format!("Read failed: {}", e))?;
            Ok(ToolResult {
                output: base64::encode(&content),
                error: None,
                metadata: None,
            })
        }) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
    });
    
    registry.register("write_file", move |args: serde_json::Value| {
        let vfs = vfs.clone();
        Box::pin(async move {
            let args: WriteFileArgs = serde_json::from_value(args)
                .map_err(|e| format!("Invalid args: {}", e))?;
            let content = args.content.as_bytes();
            let pointer = vfs.write(&args.path, content).await
                .map_err(|e| format!("Write failed: {}", e))?;
            Ok(ToolResult {
                output: serde_json::to_string(&pointer).unwrap_or_default(),
                error: None,
                metadata: None,
            })
        }) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
    });
    
    registry.register("list_files", move |args: serde_json::Value| {
        let vfs = vfs.clone();
        Box::pin(async move {
            let args: ListArgs = serde_json::from_value(args)
                .map_err(|e| format!("Invalid args: {}", e))?;
            let files = vfs.list(&args.dir).await
                .map_err(|e| format!("List failed: {}", e))?;
            Ok(ToolResult {
                output: serde_json::to_string(&files).unwrap_or_default(),
                error: None,
                metadata: None,
            })
        }) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
    });
    
    registry.register("delete_file", move |args: serde_json::Value| {
        let vfs = vfs.clone();
        Box::pin(async move {
            let args: DeleteFileArgs = serde_json::from_value(args)
                .map_err(|e| format!("Invalid args: {}", e))?;
            vfs.delete(&args.path).await
                .map_err(|e| format!("Delete failed: {}", e))?;
            Ok(ToolResult {
                output: "deleted".to_string(),
                error: None,
                metadata: None,
            })
        }) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
    });
}