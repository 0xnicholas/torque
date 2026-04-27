mod adapters;

use adapters::checkpoint_sink::InMemoryCheckpointSink;
use adapters::event_sink::InMemoryEventSink;
use adapters::model_driver::LiteModelDriver;
use adapters::output_sink::TerminalOutputSink;
use adapters::tool_executor::LiteToolExecutor;
use std::sync::Arc;
use torque_kernel::{AgentDefinition, ExecutionRequest};
use torque_runtime::host::RuntimeHost;
use torque_runtime::message::{RuntimeMessage, RuntimeMessageRole};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let goal = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Usage: torque-harness-lite <goal>"))?;

    let system_prompt = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "You are a helpful assistant with access to file system tools. Use tools when needed to answer questions.".into());

    // 1. Initialize LLM client
    let llm = Arc::new(llm::OpenAiClient::from_env()?);

    // 2. Create adapters
    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let model_driver = LiteModelDriver::new(llm.clone());
    let tool_executor = LiteToolExecutor::new();
    let output_sink = TerminalOutputSink;

    // 3. Create RuntimeHost
    let agent_def = AgentDefinition::new("lite-agent", system_prompt);
    let agent_def_id = agent_def.id;
    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink);

    // 4. Execute
    let request = ExecutionRequest::new(agent_def_id, goal.clone(), vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, goal)];

    let result = host
        .execute_v1(request, &model_driver, &tool_executor, Some(&output_sink), messages)
        .await;

    match result {
        Ok(r) => {
            println!("\n\x1b[32m[done]\x1b[0m {}", r.summary.as_deref().unwrap_or("completed"));
            eprintln!("events recorded: {}", event_sink.execution_count());
        }
        Err(e) => {
            eprintln!("\n\x1b[31m[error]\x1b[0m {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
