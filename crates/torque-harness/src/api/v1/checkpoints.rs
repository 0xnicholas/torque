use crate::agent::stream::StreamEvent;
use crate::db::Database;
use crate::models::v1::checkpoint::Checkpoint;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::recovery::{RecoveryAssessmentSummary, RecoveryResult};
use crate::models::v1::run::RunRequest;
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use torque_runtime::checkpoint::Message;
use llm::OpenAiClient;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Checkpoint>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services.checkpoint.list(limit + 1).await.map_err(|e| {
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
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
    }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<Checkpoint>, StatusCode> {
    match services.checkpoint.get(id).await {
        Ok(Some(cp)) => Ok(Json(cp)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn restore(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecoveryResult>, (StatusCode, Json<ErrorBody>)> {
    let assessment = services.recovery.assess_recovery(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "ASSESSMENT_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    let (instance, _messages, _rebuilt_state) = services
        .recovery
        .restore_from_checkpoint(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "RESTORE_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let result = RecoveryResult {
        instance_id: instance.id,
        checkpoint_id: id,
        restored_status: format!("{:?}", instance.status),
        assessment: RecoveryAssessmentSummary {
            disposition: format!("{:?}", assessment.disposition),
            requires_replay: assessment.requires_replay,
            terminal: assessment.terminal,
        },
        recommended_action: format!("{:?}", assessment.recommended_action),
    };

    Ok(Json(result))
}

pub async fn get_messages(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckpointMessagesResponse>, (StatusCode, Json<ErrorBody>)> {
    let messages = services
        .recovery
        .get_checkpoint_messages(id)
        .await
        .map_err(ErrorBody::db_error)?;
    Ok(Json(CheckpointMessagesResponse {
        checkpoint_id: id,
        messages,
    }))
}

#[derive(serde::Serialize)]
pub struct CheckpointMessagesResponse {
    pub checkpoint_id: Uuid,
    pub messages: Vec<Message>,
}

pub async fn resume(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<RunRequest>,
) -> Result<Sse<ReceiverStream<Result<Event, axum::Error>>>, (StatusCode, Json<ErrorBody>)> {
    // 1. Validate checkpoint exists
    let checkpoint = services
        .checkpoint
        .get(id)
        .await
        .map_err(ErrorBody::db_error)?
        .ok_or_else(|| ErrorBody::not_found(format!("Checkpoint {} not found", id)))?;

    // 2. Assess recovery
    let assessment = services.recovery.assess_recovery(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "ASSESSMENT_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    // 3. Reject terminal states
    if assessment.terminal {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorBody {
                code: "CANNOT_RESUME".into(),
                message: format!(
                    "Instance is in terminal state {:?}, cannot resume",
                    assessment.disposition
                ),
                details: None,
                request_id: None,
            }),
        ));
    }

    // 4. Restore from checkpoint
    let (_instance, messages, _rebuilt_state) = services
        .recovery
        .restore_from_checkpoint(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "RESTORE_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let instance_id = checkpoint.agent_instance_id;
    let runtime_messages: Vec<crate::runtime::message::RuntimeMessage> =
        messages.into_iter().map(|m| m.into()).collect();

    // 5. SSE streaming execution
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(32);
    let stream_tx = tx.clone();
    let run_service = services.run.clone();

    tokio::spawn(async move {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<StreamEvent>(32);
        let run_handle = tokio::spawn(async move {
            run_service
                .execute_with_history(instance_id, req, event_tx, runtime_messages)
                .await
        });

        while let Some(event) = event_rx.recv().await {
            let sse_event = event_to_sse(&event);
            if stream_tx.send(Ok(sse_event)).await.is_err() {
                break;
            }
        }

        let _ = run_handle.await;
    });

    Ok(Sse::new(ReceiverStream::new(rx)))
}

fn event_to_sse(event: &StreamEvent) -> Event {
    Event::default()
        .event(event.event_name())
        .json_data(&event)
        .unwrap_or_else(|_| Event::default().event("error").data("serialization error"))
}
