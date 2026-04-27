mod common;

use async_trait::async_trait;
use chrono::Utc;
use common::fake_llm::FakeLlm;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use torque_harness::agent::stream::StreamEvent;
use torque_harness::models::v1::artifact::{Artifact, ArtifactScope};
use torque_harness::models::v1::event::Event;
use torque_harness::repository::{ArtifactRepository, EventRepository};
use torque_harness::runtime::{
    HarnessEventSink, HarnessModelDriver, HarnessToolExecutor, StreamEventSinkAdapter,
};
use torque_harness::service::{ArtifactService, ToolService};
use torque_runtime::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::{
    RuntimeCheckpointSink, RuntimeEventSink, RuntimeExecutionContext, RuntimeToolExecutor,
    RuntimeModelDriver, RuntimeOutputSink,
};
use torque_runtime::message::{RuntimeMessage, RuntimeMessageRole};
use torque_runtime::tools::RuntimeToolDef;
use uuid::Uuid;

struct NoopArtifactRepository;

#[async_trait]
impl ArtifactRepository for NoopArtifactRepository {
    async fn create(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
    ) -> anyhow::Result<Artifact> {
        self.create_with_source_instance(kind, scope, mime_type, content, None)
            .await
    }

    async fn create_with_source_instance(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Artifact> {
        Ok(Artifact {
            id: Uuid::new_v4(),
            kind: kind.to_string(),
            scope,
            source_instance_id,
            published_to_team_instance_id: None,
            mime_type: mime_type.to_string(),
            size_bytes: serde_json::to_string(&content)?.len() as i64,
            summary: None,
            content,
            created_at: Utc::now(),
        })
    }

    async fn list(&self, _limit: i64) -> anyhow::Result<Vec<Artifact>> { Ok(vec![]) }
    async fn list_by_instance(&self, _instance_id: Uuid, _limit: i64) -> anyhow::Result<Vec<Artifact>> { Ok(vec![]) }
    async fn get(&self, _id: Uuid) -> anyhow::Result<Option<Artifact>> { Ok(None) }
    async fn delete(&self, _id: Uuid) -> anyhow::Result<bool> { Ok(false) }
    async fn update_scope(&self, _id: Uuid, _scope: ArtifactScope) -> anyhow::Result<bool> { Ok(false) }
    async fn find_latest_by_kind_scope_and_content_string(&self, _kind: &str, _scope: ArtifactScope, _content_key: &str, _content_value: &str) -> anyhow::Result<Option<Artifact>> { Ok(None) }
    async fn find_latest_by_kind_scope_and_content_string_with_source_instance(&self, _kind: &str, _scope: ArtifactScope, _content_key: &str, _content_value: &str, _source_instance_id: Option<Uuid>) -> anyhow::Result<Option<Artifact>> { Ok(None) }
}

#[derive(Default)]
struct InMemoryEventRepository {
    events: Mutex<Vec<Event>>,
}

#[async_trait]
impl EventRepository for InMemoryEventRepository {
    async fn create(&self, event: Event) -> anyhow::Result<()> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }

    async fn create_batch(&self, events: Vec<Event>) -> anyhow::Result<()> {
        self.events.lock().unwrap().extend(events);
        Ok(())
    }

    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Event>> {
        Ok(self
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.resource_type == resource_type && event.resource_id == resource_id)
            .cloned()
            .collect())
    }
}

#[derive(Default)]
struct FakeCheckpointSink {
    saved: Mutex<VecDeque<RuntimeCheckpointPayload>>,
}

#[async_trait]
impl RuntimeCheckpointSink for FakeCheckpointSink {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef> {
        let checkpoint_id = Uuid::new_v4();
        let instance_id = checkpoint.instance_id.as_uuid();
        self.saved.lock().unwrap().push_back(checkpoint);
        Ok(RuntimeCheckpointRef {
            checkpoint_id,
            instance_id,
        })
    }
}

fn setup_tool_service() -> Arc<ToolService> {
    let artifact_service = Arc::new(ArtifactService::new(Arc::new(NoopArtifactRepository)));
    Arc::new(ToolService::new_with_builtins(artifact_service))
}

#[tokio::test]
async fn model_driver_adapter_converts_messages_and_streams_output() {
    let llm = Arc::new(FakeLlm::single_text("hello from model"));
    let driver = HarnessModelDriver::new(llm.clone());
    let (tx, mut rx) = mpsc::channel(8);
    let sink = StreamEventSinkAdapter::new(tx);

    let result = driver
        .run_turn(
            vec![RuntimeMessage::new(RuntimeMessageRole::User, "hello")],
            vec![RuntimeToolDef {
                name: "echo".to_string(),
                description: "Echo".to_string(),
                parameters: json!({}),
            }],
            Some(&sink),
        )
        .await
        .expect("model driver should succeed");

    assert_eq!(result.assistant_text, "hello from model");
    let event = rx.recv().await.expect("chunk event should be emitted");
    assert!(matches!(event, StreamEvent::Chunk { content } if content == "hello from model"));
    assert_eq!(llm.recorded_requests().len(), 1);
}

#[tokio::test]
async fn tool_executor_adapter_executes_registered_tools() {
    let executor = HarnessToolExecutor::new(setup_tool_service());

    let defs = executor.tool_defs().await.expect("tool defs");
    assert!(defs.iter().any(|def| def.name == "read_todos"));

    let result = executor
        .execute(
            RuntimeExecutionContext {
                instance_id: Uuid::new_v4(),
                request_id: None,
                source_task_id: None,
            },
            "write_file",
            json!({ "path": "/scratch/test.txt", "content": "hello" }),
        )
        .await
        .expect("tool execution should succeed");

    assert!(result.success);
}

#[tokio::test]
async fn output_sink_adapter_translates_runtime_events() {
    let (tx, mut rx) = mpsc::channel(8);
    let sink = StreamEventSinkAdapter::new(tx);

    sink.on_text_chunk("abc");
    sink.on_tool_call("echo", &json!({"x": 1}));
    sink.on_tool_result("echo", &torque_runtime::tools::RuntimeToolResult::success("ok"));
    sink.on_checkpoint(Uuid::nil(), "checkpoint");

    assert!(matches!(rx.recv().await.unwrap(), StreamEvent::Chunk { content } if content == "abc"));
    assert!(matches!(rx.recv().await.unwrap(), StreamEvent::ToolCall { name, .. } if name == "echo"));
    assert!(matches!(rx.recv().await.unwrap(), StreamEvent::ToolResult { name, .. } if name == "echo"));
    assert!(matches!(rx.recv().await.unwrap(), StreamEvent::CheckpointCreated { .. }));
}

#[tokio::test]
async fn event_checkpoint_and_hydration_adapters_return_expected_shapes() {
    let event_repo = Arc::new(InMemoryEventRepository::default());
    let event_sink = HarnessEventSink::new(event_repo.clone());
    let checkpoint_sink = Arc::new(FakeCheckpointSink::default());

    let instance_id = torque_kernel::AgentInstanceId::new();
    let agent_definition = torque_kernel::AgentDefinition::new("adapter-test", "adapter test");
    let request = torque_kernel::ExecutionRequest::new(
        agent_definition.id,
        "goal",
        vec!["do it".to_string()],
    );
    let mut runtime = torque_kernel::InMemoryKernelRuntime::new(vec![agent_definition]);
    let result = torque_kernel::KernelRuntime::handle(
        &mut runtime,
        request,
        torque_kernel::StepDecision::Continue,
    )
    .expect("kernel result");

    event_sink
        .record_execution_result(&result)
        .await
        .expect("event sink should record");
    event_sink
        .record_checkpoint_created(Uuid::new_v4(), result.instance_id, "reason")
        .await
        .expect("checkpoint event should record");

    let checkpoint_ref = checkpoint_sink
        .save(RuntimeCheckpointPayload {
            instance_id,
            node_id: Uuid::new_v4(),
            reason: "test".to_string(),
            state: serde_json::json!({
                "messages": [],
                "tool_call_count": 0,
                "intermediate_results": [],
                "custom_state": null,
            }),
        })
        .await
        .expect("checkpoint save should succeed");
    assert_eq!(checkpoint_ref.instance_id, instance_id.as_uuid());
}
