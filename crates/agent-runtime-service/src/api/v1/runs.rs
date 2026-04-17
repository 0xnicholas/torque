use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use crate::agent::stream::StreamEvent;
use crate::db::Database;
use crate::models::v1::run::RunRequest;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

pub async fn run(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<RunRequest>,
) -> Sse<ReceiverStream<Result<Event, axum::Error>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(32);
    let stream_tx = tx.clone();
    let run_service = services.run.clone();

    tokio::spawn(async move {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<StreamEvent>(32);

        // Spawn execution
        let run_handle = tokio::spawn(async move {
            run_service.execute(id, req, event_tx).await
        });

        // Forward StreamEvents to SSE
        while let Some(event) = event_rx.recv().await {
            let sse_event = Event::default().event(event.event_name())
                .json_data(&event)
                .unwrap_or_else(|_| Event::default().event("error").data("serialization error"));
            
            if stream_tx.send(Ok(sse_event)).await.is_err() {
                break; // Client disconnected
            }
        }

        // Wait for completion
        let _ = run_handle.await;
    });

    Sse::new(ReceiverStream::new(rx))
}
