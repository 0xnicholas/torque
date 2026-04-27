use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use axum::{http::StatusCode, Json};

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ErrorBody {
    pub fn db_error(e: impl ToString) -> (StatusCode, Json<Self>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self {
                code: "DB_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    }

    pub fn not_found(resource: impl ToString) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self {
                code: "NOT_FOUND".into(),
                message: resource.to_string(),
                details: None,
                request_id: None,
            }),
        )
    }

    pub fn bad_request(msg: impl ToString) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self {
                code: "BAD_REQUEST".into(),
                message: msg.to_string(),
                details: None,
                request_id: None,
            }),
        )
    }
}

#[derive(Debug, Serialize)]
pub struct Pagination {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    pub sort: Option<String>,
    pub filter_status: Option<String>,
    pub filter_created_after: Option<DateTime<Utc>>,
    pub filter_created_before: Option<DateTime<Utc>>,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Deserialize, Default)]
pub struct EventListQuery {
    #[serde(flatten)]
    pub base: ListQuery,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub before_event_id: Option<String>,
    pub after_event_id: Option<String>,
    pub event_types: Option<Vec<String>>,
}
