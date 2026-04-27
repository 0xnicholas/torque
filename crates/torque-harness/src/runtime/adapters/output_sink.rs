use crate::agent::stream::StreamEvent;
use tokio::sync::mpsc;
use torque_runtime::environment::RuntimeOutputSink;
use torque_runtime::tools::RuntimeToolResult;
use uuid::Uuid;

pub struct StreamEventSinkAdapter {
    tx: mpsc::Sender<StreamEvent>,
}

impl StreamEventSinkAdapter {
    pub fn new(tx: mpsc::Sender<StreamEvent>) -> Self {
        Self { tx }
    }
}

impl RuntimeOutputSink for StreamEventSinkAdapter {
    fn on_text_chunk(&self, chunk: &str) {
        let _ = self.tx.try_send(StreamEvent::Chunk {
            content: chunk.to_string(),
        });
    }

    fn on_tool_call(&self, tool_name: &str, arguments: &serde_json::Value) {
        let _ = self.tx.try_send(StreamEvent::ToolCall {
            name: tool_name.to_string(),
            arguments: arguments.clone(),
        });
    }

    fn on_tool_result(&self, tool_name: &str, result: &RuntimeToolResult) {
        let _ = self.tx.try_send(StreamEvent::ToolResult {
            name: tool_name.to_string(),
            success: result.success,
            content: result.content.clone(),
            error: result.error.clone(),
        });
    }

    fn on_checkpoint(&self, checkpoint_id: Uuid, reason: &str) {
        let _ = self.tx.try_send(StreamEvent::CheckpointCreated {
            checkpoint_id,
            reason: reason.to_string(),
        });
    }
}
