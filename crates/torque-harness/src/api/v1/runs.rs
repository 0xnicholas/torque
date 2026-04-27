use crate::agent::stream::StreamEvent;
use crate::db::Database;
use crate::models::v1::common::ErrorBody;
use crate::models::v1::run::{Run, RunRequest, RunStatus};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use chrono::Utc;
use llm::OpenAiClient;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct RunResponse {
    pub run: Run,
}

#[derive(serde::Serialize)]
pub struct WebhookStatusResponse {
    pub run_id: Uuid,
    pub webhook_url: Option<String>,
    pub webhook_sent_at: Option<chrono::DateTime<Utc>>,
    pub webhook_attempts: Option<i32>,
}

pub async fn create(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<RunRequest>,
) -> Result<(StatusCode, Json<RunResponse>), (StatusCode, Json<ErrorBody>)> {
    let run = Run {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        status: RunStatus::Queued,
        instruction: req.instructions.clone().unwrap_or_default(),
        failure_policy: None,
        webhook_url: req.webhook_url.clone(),
        async_execution: req.async_execution,
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
        webhook_sent_at: None,
        webhook_attempts: None,
    };

    services.run_repo.create(&run).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "DB_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    if req.async_execution {
        let async_runner = services.async_runner.clone();
        tokio::spawn(async move {
            if let Err(e) = async_runner.process_run(run.id).await {
                tracing::error!("Async run {} failed: {}", run.id, e);
            }
        });
    }

    Ok((StatusCode::CREATED, Json(RunResponse { run })))
}

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
        let run_handle = tokio::spawn(async move { run_service.execute(id, req, event_tx).await });

        // Forward StreamEvents to SSE
        while let Some(event) = event_rx.recv().await {
            let sse_event = Event::default()
                .event(event.event_name())
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

pub async fn webhook_status(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<WebhookStatusResponse>, (StatusCode, Json<ErrorBody>)> {
    let run = services
        .run_repo
        .get(id)
        .await
        .map_err(ErrorBody::db_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    code: "NOT_FOUND".into(),
                    message: format!("Run {} not found", id),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(WebhookStatusResponse {
        run_id: run.id,
        webhook_url: run.webhook_url,
        webhook_sent_at: run.webhook_sent_at,
        webhook_attempts: run.webhook_attempts,
    }))
}
