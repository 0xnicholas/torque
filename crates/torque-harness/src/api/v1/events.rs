use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::event::Event;
use crate::service::ServiceContainer;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use llm::OpenAiClient;
use std::sync::Arc;

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Event>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services.event.list(limit + 1).await.map_err(|e| {
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
    let next_cursor = rows.last().map(|r| r.event_id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
    }))
}
