use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use crate::db::Database;
use crate::models::v1::run::RunRequest;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

pub async fn run(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
    Json(_req): Json<RunRequest>,
) -> Sse<ReceiverStream<Result<Event, axum::Error>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(32);

    tokio::spawn(async move {
        let start = serde_json::json!({"event": "run.started", "data": {"status": "RUNNING"}});
        let _ = tx.send(Ok(Event::default().event("run.started").json_data(start).unwrap())).await;
        
        let done = serde_json::json!({"event": "run.completed", "data": {"status": "COMPLETED"}});
        let _ = tx.send(Ok(Event::default().event("run.completed").json_data(done).unwrap())).await;
    });

    Sse::new(ReceiverStream::new(rx))
}
