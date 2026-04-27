use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListResponse, Pagination};
use crate::models::v1::escalation::Escalation;
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct EscalationListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<EscalationListQuery>,
) -> Result<Json<ListResponse<Escalation>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let mut escalations = services
        .escalation_service
        .list_pending_escalations(limit + 1)
        .await
        .map_err(ErrorBody::db_error)?;

    let has_more = escalations.len() > limit as usize;
    if has_more {
        escalations.pop();
    }
    let next_cursor = escalations.last().map(|r| r.id.to_string());

    Ok(Json(ListResponse {
        data: escalations,
        pagination: Pagination {
            next_cursor,
            prev_cursor: None,
            has_more,
        },
    }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<Escalation>, (StatusCode, Json<ErrorBody>)> {
    let escalation = services
        .escalation_service
        .get_escalation(id)
        .await
        .map_err(ErrorBody::db_error)?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                code: "NOT_FOUND".into(),
                message: "Escalation not found".into(),
                details: None,
                request_id: None,
            }),
        ))?;

    Ok(Json(escalation))
}

#[derive(serde::Deserialize)]
pub struct EscalationResolveRequest {
    pub resolution: String,
    pub resolved_by: Uuid,
}

pub async fn resolve(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<EscalationResolveRequest>,
) -> Result<Json<Escalation>, (StatusCode, Json<ErrorBody>)> {
    services
        .escalation_service
        .get_escalation(id)
        .await
        .map_err(ErrorBody::db_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    code: "NOT_FOUND".into(),
                    message: "Escalation not found".into(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let escalation = services
        .escalation_service
        .resolve_escalation(id, &req.resolution, req.resolved_by)
        .await
        .map_err(ErrorBody::db_error)?;

    Ok(Json(escalation))
}
