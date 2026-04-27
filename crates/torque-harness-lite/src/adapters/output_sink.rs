use torque_runtime::environment::RuntimeOutputSink;
use torque_runtime::tools::RuntimeToolResult;
use uuid::Uuid;

pub struct TerminalOutputSink;

impl RuntimeOutputSink for TerminalOutputSink {
    fn on_text_chunk(&self, chunk: &str) {
        print!("{}", chunk);
    }

    fn on_tool_call(&self, tool_name: &str, arguments: &serde_json::Value) {
        println!("\x1b[34m  [tool: {}]\x1b[0m {}", tool_name, arguments);
    }

    fn on_tool_result(&self, tool_name: &str, result: &RuntimeToolResult) {
        if result.success {
            let preview: String = result
                .content
                .chars()
                .take(120)
                .collect();
            println!("\x1b[32m  [result: {}]\x1b[0m {}", tool_name, preview);
        } else {
            println!(
                "\x1b[31m  [result: {}]\x1b[0m {}",
                tool_name,
                result.error.as_deref().unwrap_or("unknown")
            );
        }
    }

    fn on_checkpoint(&self, _checkpoint_id: Uuid, _reason: &str) {}
}
