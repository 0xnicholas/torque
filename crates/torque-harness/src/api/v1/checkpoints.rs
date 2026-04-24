use crate::db::Database;
use crate::models::v1::checkpoint::Checkpoint;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::recovery::{RecoveryAssessmentSummary, RecoveryResult};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use checkpointer::r#trait::Message;
use llm::OpenAiClient;
use std::sync::Arc;
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
    let assessment = services
        .recovery
        .assess_recovery(id)
        .await
        .map_err(|e| {
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
    let messages = services.recovery.get_checkpoint_messages(id).await.map_err(|e| {
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
    Ok(Json(CheckpointMessagesResponse { checkpoint_id: id, messages }))
}

#[derive(serde::Serialize)]
pub struct CheckpointMessagesResponse {
    pub checkpoint_id: Uuid,
    pub messages: Vec<Message>,
}
