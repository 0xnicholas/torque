use async_trait::async_trait;
use llm::{ChatRequest, Chunk, FinishReason, LlmClient};
use std::sync::{Arc, Mutex};
use torque_runtime::environment::{RuntimeModelDriver, RuntimeOutputSink};
use torque_runtime::events::{ModelTurnResult, RuntimeFinishReason};
use torque_runtime::message::RuntimeMessage;
use torque_runtime::tools::{RuntimeToolCall, RuntimeToolDef};

pub struct LiteModelDriver {
    llm: Arc<dyn LlmClient>,
}

impl LiteModelDriver {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl RuntimeModelDriver for LiteModelDriver {
    async fn run_turn(
        &self,
        messages: Vec<RuntimeMessage>,
        tools: Vec<RuntimeToolDef>,
        sink: Option<&dyn RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult> {
        let llm_messages = messages
            .into_iter()
            .map(Into::into)
            .collect::<Vec<llm::Message>>();
        let llm_tools = tools.into_iter().map(Into::into).collect::<Vec<llm::ToolDef>>();

        let text_chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let tool_calls = Arc::new(Mutex::new(Vec::<RuntimeToolCall>::new()));
        let text_chunks_clone = text_chunks.clone();
        let tool_calls_clone = tool_calls.clone();

        let callback = Box::new(move |chunk: Chunk| {
            if !chunk.content.is_empty() {
                text_chunks_clone
                    .lock()
                    .expect("text chunk lock poisoned")
                    .push(chunk.content.clone());
            }
            if let Some(tool_call) = chunk.tool_call {
                tool_calls_clone
                    .lock()
                    .expect("tool call lock poisoned")
                    .push(tool_call.into());
            }
        });

        let response = self
            .llm
            .chat_streaming(
                ChatRequest::new(self.llm.model().to_string(), llm_messages)
                    .with_tools(llm_tools),
                callback,
            )
            .await?;

        let assistant_text = text_chunks
            .lock()
            .expect("text chunk lock poisoned")
            .join("");
        let tool_calls = tool_calls
            .lock()
            .expect("tool call lock poisoned")
            .clone();

        if let Some(sink) = sink {
            if !assistant_text.is_empty() {
                sink.on_text_chunk(&assistant_text);
            }
            for tool_call in &tool_calls {
                sink.on_tool_call(&tool_call.name, &tool_call.arguments);
            }
        }

        Ok(ModelTurnResult {
            finish_reason: match response.finish_reason {
                FinishReason::Stop => RuntimeFinishReason::Stop,
                FinishReason::Length => RuntimeFinishReason::Length,
                FinishReason::ContentFilter => RuntimeFinishReason::ContentFilter,
                FinishReason::ToolCalls => RuntimeFinishReason::ToolCalls,
            },
            assistant_text,
            tool_calls,
        })
    }
}
