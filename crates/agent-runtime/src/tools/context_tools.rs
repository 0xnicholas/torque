use std::pin::Pin;
use std::sync::Arc;
use futures::Future;
use serde::Deserialize;

use crate::context_mgr::{CompressionStrategy, ContextManager, Summary};
use tool_executor::registry::ToolRegistry;
use tool_executor::ToolResult;

#[derive(Debug, Deserialize)]
pub struct CompressArgs {
    pub strategy: Option<String>,
    pub keep_recent: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct LabelRangeArgs {
    pub start_idx: usize,
    pub end_idx: usize,
    pub label: Option<String>,
}

pub fn register_context_tools(
    registry: &mut ToolRegistry,
    context_mgr: Arc<tokio::sync::Mutex<ContextManager>>,
) {
    registry.register("context_compress", move |args: serde_json::Value| {
        let ctx = context_mgr.clone();
        Box::pin(async move {
            let args: CompressArgs = serde_json::from_value(args)
                .map_err(|e| format!("Invalid args: {}", e))?;
            
            let mut mgr = ctx.lock().await;
            
            let strategy = args.strategy.as_deref().unwrap_or("summarize");
            let keep_recent = args.keep_recent.unwrap_or(5);
            
            match strategy {
                "summarize" => {
                    mgr.compression_strategy = CompressionStrategy::SummarizeOlder {
                        summarize_count: mgr.full_history.len().saturating_sub(keep_recent),
                    };
                }
                "keep_last_n" => {
                    mgr.compression_strategy = CompressionStrategy::KeepLastN(keep_recent);
                }
                "extractive" => {
                    mgr.compression_strategy = CompressionStrategy::ExtractiveCompression;
                }
                _ => {}
            }
            
            mgr.compress().await
                .map_err(|e| format!("Compression failed: {}", e))?;
            
            Ok(ToolResult {
                output: "Context compression strategy updated and compression triggered".to_string(),
                error: None,
                metadata: None,
            })
        }) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
    });
    
    registry.register("context_label_range", move |args: serde_json::Value| {
        let ctx = context_mgr.clone();
        Box::pin(async move {
            let args: LabelRangeArgs = serde_json::from_value(args)
                .map_err(|e| format!("Invalid args: {}", e))?;

            let mut mgr = ctx.lock().await;

            let label = args.label.unwrap_or_else(|| {
                format!("Label for messages {} to {}", args.start_idx, args.end_idx)
            });

            mgr.summary_chain.push(Summary {
                covers_range: (args.start_idx, args.end_idx),
                content: label.clone(),
                created_at: chrono::Utc::now(),
            });

            mgr.persist_to_logs().await
                .map_err(|e| format!("Persist failed: {}", e))?;

            Ok(ToolResult {
                output: label,
                error: None,
                metadata: None,
            })
        }) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
    });
}