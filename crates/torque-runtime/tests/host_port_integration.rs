use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use torque_runtime::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::{
    RuntimeCheckpointSink, RuntimeEventSink, RuntimeExecutionContext, RuntimeModelDriver,
    RuntimeToolExecutor,
};
use torque_runtime::events::{ModelTurnResult, RuntimeFinishReason};
use torque_runtime::host::RuntimeHost;
use torque_runtime::message::RuntimeMessage;
use torque_runtime::offload::ToolOffloadPolicy;
use torque_runtime::tools::{RuntimeToolCall, RuntimeToolDef, RuntimeToolResult};
use torque_runtime::vfs::{EditResult, FileInfo, GrepMatch, VfsBackend};
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

struct ToolCallingModelDriver;

#[async_trait]
impl RuntimeModelDriver for ToolCallingModelDriver {
    async fn run_turn(
        &self,
        _messages: Vec<RuntimeMessage>,
        _tools: Vec<RuntimeToolDef>,
        _sink: Option<&dyn torque_runtime::environment::RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult> {
        Ok(ModelTurnResult {
            finish_reason: RuntimeFinishReason::ToolCalls,
            assistant_text: String::new(),
            tool_calls: vec![RuntimeToolCall {
                id: "call_test".to_string(),
                name: "test_tool".to_string(),
                arguments: serde_json::json!({}),
            }],
        })
    }
}

struct LargeOutputToolExecutor;

#[async_trait]
impl RuntimeToolExecutor for LargeOutputToolExecutor {
    async fn execute(
        &self,
        _ctx: RuntimeExecutionContext,
        _name: &str,
        _args: serde_json::Value,
    ) -> anyhow::Result<RuntimeToolResult> {
        Ok(RuntimeToolResult {
            success: true,
            content: "x".repeat(5000),
            error: None,
            offload_ref: None,
        })
    }

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> {
        Ok(vec![])
    }
}

struct RecordingScratch(Mutex<Vec<String>>);

#[async_trait]
impl VfsBackend for RecordingScratch {
    async fn ls(&self, _: &str) -> anyhow::Result<Vec<FileInfo>> {
        Ok(vec![])
    }
    async fn read(&self, _: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
    async fn write(&self, path: &str, _: &str) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(path.to_string());
        Ok(())
    }
    async fn edit(&self, _: &str, _: &str, _: &str, _: bool) -> anyhow::Result<EditResult> {
        Ok(EditResult { occurrences: 0 })
    }
    async fn glob(&self, _: &str, _: &str) -> anyhow::Result<Vec<FileInfo>> {
        Ok(vec![])
    }
    async fn grep(&self, _: &str, _: &str) -> anyhow::Result<Vec<GrepMatch>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn tool_result_offloaded_to_scratch_when_above_inline_threshold() {
    let scratch = Arc::new(RecordingScratch(Mutex::new(vec![])));
    let offload_policy = Arc::new(ToolOffloadPolicy::new(Some(scratch.clone()), None));
    let agent_def = torque_kernel::AgentDefinition::new("test", "system");

    let mut host = RuntimeHost::new(
        vec![agent_def.clone()],
        Arc::new(FakeEventSink::default()),
        Arc::new(FakeCheckpointSink::default()),
    )
    .with_offload_policy(offload_policy);

    let request = torque_kernel::ExecutionRequest::new(agent_def.id, "Test offload", vec![]);
    let _ = host
        .execute_v1(
            request,
            &ToolCallingModelDriver,
            &LargeOutputToolExecutor,
            None,
            vec![RuntimeMessage::user("go")],
        )
        .await;

    let paths = scratch.0.lock().unwrap();
    assert!(
        paths
            .iter()
            .any(|p| p.starts_with("/scratch/tool-results/")),
        "Expected offloaded path in {:?}",
        paths
    );
}

#[tokio::test]
async fn context_compacted_when_messages_exceed_threshold() {
    let mut messages = vec![];
    for i in 0..20 {
        messages.push(RuntimeMessage::user(format!("message {}", i)));
        messages.push(RuntimeMessage::assistant(format!("response {}", i)));
    }

    let agent_def = torque_kernel::AgentDefinition::new("test", "system");
    let mut host = RuntimeHost::new(
        vec![agent_def.clone()],
        Arc::new(FakeEventSink::default()),
        Arc::new(FakeCheckpointSink::default()),
    );

    let request = torque_kernel::ExecutionRequest::new(agent_def.id, "Compact test", vec![]);
    let result = host
        .execute_v1(request, &FakeModelDriver, &LargeOutputToolExecutor, None, messages)
        .await;

    assert!(result.is_ok());
}
