use async_trait::async_trait;
use std::sync::Mutex;
use torque_runtime::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::{
    RuntimeCheckpointSink, RuntimeEventSink, RuntimeModelDriver, RuntimeToolExecutor,
};
use torque_runtime::events::{ModelTurnResult, RuntimeFinishReason};
use torque_runtime::host::RuntimeHost;
use torque_runtime::message::RuntimeMessage;
use torque_runtime::tools::{RuntimeToolDef, RuntimeToolResult};
use uuid::Uuid;

#[derive(Default)]
struct FakeEventSink {
    execution_results: Mutex<usize>,
    checkpoint_events: Mutex<usize>,
}

#[async_trait]
impl RuntimeEventSink for FakeEventSink {
    async fn record_execution_result(
        &self,
        _result: &torque_kernel::ExecutionResult,
    ) -> anyhow::Result<()> {
        *self.execution_results.lock().unwrap() += 1;
        Ok(())
    }

    async fn record_checkpoint_created(
        &self,
        _checkpoint_id: Uuid,
        _instance_id: torque_kernel::AgentInstanceId,
        _reason: &str,
    ) -> anyhow::Result<()> {
        *self.checkpoint_events.lock().unwrap() += 1;
        Ok(())
    }
}

#[derive(Default)]
struct FakeCheckpointSink {
    saves: Mutex<usize>,
}

#[async_trait]
impl RuntimeCheckpointSink for FakeCheckpointSink {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef> {
        *self.saves.lock().unwrap() += 1;
        Ok(RuntimeCheckpointRef {
            checkpoint_id: Uuid::new_v4(),
            instance_id: checkpoint.instance_id.as_uuid(),
        })
    }
}

struct FakeModelDriver;

#[async_trait]
impl RuntimeModelDriver for FakeModelDriver {
    async fn run_turn(
        &self,
        _messages: Vec<RuntimeMessage>,
        _tools: Vec<RuntimeToolDef>,
        _sink: Option<&dyn torque_runtime::environment::RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult> {
        Ok(ModelTurnResult {
            finish_reason: RuntimeFinishReason::Stop,
            assistant_text: "final answer".to_string(),
            tool_calls: vec![],
        })
    }
}

struct FakeToolExecutor;

#[async_trait]
impl RuntimeToolExecutor for FakeToolExecutor {
    async fn execute(
        &self,
        _ctx: torque_runtime::environment::RuntimeExecutionContext,
        _tool_name: &str,
        _arguments: serde_json::Value,
    ) -> anyhow::Result<RuntimeToolResult> {
        Ok(RuntimeToolResult::success("unused"))
    }

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn runtime_host_executes_through_neutral_ports() {
    let event_sink = std::sync::Arc::new(FakeEventSink::default());
    let checkpoint_sink = std::sync::Arc::new(FakeCheckpointSink::default());
    let agent_definition = torque_kernel::AgentDefinition::new("host-test", "host test");
    let request =
        torque_kernel::ExecutionRequest::new(agent_definition.id, "goal", vec!["do it".into()]);

    let mut host = RuntimeHost::new(vec![agent_definition], event_sink.clone(), checkpoint_sink.clone());
    let result = host
        .execute_chat(
            request,
            &FakeModelDriver,
            &FakeToolExecutor,
            None,
            vec![RuntimeMessage::user("hello")],
        )
        .await
        .expect("host should execute");

    assert_eq!(result.summary.as_deref(), Some("final answer"));
    assert_eq!(*event_sink.execution_results.lock().unwrap(), 2);
    assert_eq!(*event_sink.checkpoint_events.lock().unwrap(), 1);
    assert_eq!(*checkpoint_sink.saves.lock().unwrap(), 1);
}
