use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListResponse};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use llm::LlmClient;
use std::sync::Arc;
use torque_extension::{
    id::ExtensionId,
    message::ExtensionAction,
};

/// Response body for an extension summary.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExtensionSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub state: String,
}

/// Response body for extension details.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExtensionDetail {
    pub id: String,
    pub name: String,
    pub version: String,
    pub state: String,
    pub hooks: Vec<String>,
    pub subscriptions: Vec<String>,
}

/// Request body for sending a message to an extension.
#[derive(Debug, serde::Deserialize)]
pub struct SendMessageRequest {
    pub namespace: Option<String>,
    pub action: String,
    pub payload: Option<serde_json::Value>,
}

/// Response body after sending a message.
#[derive(Debug, serde::Serialize)]
pub struct SendMessageResponse {
    pub delivered: bool,
    pub message: String,
}

// ── Helpers for parsing ExtensionId from URL path ───────────────────────

fn parse_ext_id(id: &str) -> Result<ExtensionId, (StatusCode, Json<ErrorBody>)> {
    let uuid = uuid::Uuid::parse_str(id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                code: "INVALID_ID".into(),
                message: format!("invalid extension ID: {id}"),
                details: None,
                request_id: None,
            }),
        )
    })?;
    Ok(ExtensionId::from_uuid(uuid))
}

fn svc<'a>(
    services: &'a ServiceContainer,
) -> Result<&'a crate::extension::ExtensionService, (StatusCode, Json<ErrorBody>)> {
    match &services.extension_service {
        Some(svc) => Ok(svc.as_ref()),
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                code: "EXTENSION_DISABLED".into(),
                message: "Extension system is not enabled (compile with feature 'extension')"
                    .into(),
                details: None,
                request_id: None,
            }),
        )),
    }
}

// ── Handlers ────────────────────────────────────────────────────────────

/// List all registered extensions.
pub async fn list(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
) -> Result<Json<ListResponse<ExtensionSummary>>, (StatusCode, Json<ErrorBody>)> {
    let svc = svc(&services)?;

    let extensions = svc.list_with_names().await;
    let mut data = Vec::with_capacity(extensions.len());
    for (id, name) in extensions {
        let state = svc.lifecycle(id).await
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        data.push(ExtensionSummary {
            id: id.to_string(),
            name,
            version: "-".to_string(),
            state,
        });
    }

    Ok(Json(ListResponse {
        data,
        pagination: crate::models::v1::common::Pagination {
            next_cursor: None,
            prev_cursor: None,
            has_more: false,
        },
    }))
}

/// Get details of a specific extension.
pub async fn get(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionDetail>, (StatusCode, Json<ErrorBody>)> {
    let svc = svc(&services)?;
    let ext_id = parse_ext_id(&id)?;

    // Use snapshot for full detail.
    match svc.snapshot(ext_id).await {
        Ok(snapshot) => Ok(Json(ExtensionDetail {
            id: snapshot.id.to_string(),
            name: snapshot.name,
            version: snapshot.version.to_string(),
            state: snapshot.lifecycle.to_string(),
            hooks: snapshot.registered_hooks,
            subscriptions: snapshot.bus_subscriptions,
        })),
        Err(_) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                code: "NOT_FOUND".into(),
                message: format!("extension not found: {id}"),
                details: None,
                request_id: None,
            }),
        )),
    }
}

/// Unregister (delete) an extension.
pub async fn delete(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let svc = svc(&services)?;
    let ext_id = parse_ext_id(&id)?;

    svc.unregister(ext_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "UNREGISTER_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Suspend an extension.
pub async fn suspend_extension(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let svc = svc(&services)?;
    let ext_id = parse_ext_id(&id)?;

    match svc.suspend(ext_id).await {
        Ok(_) => Ok(Json(serde_json::json!({ "suspended": true }))),
        Err(e) => Err((
            StatusCode::NOT_IMPLEMENTED,
            Json(ErrorBody {
                code: "NOT_IMPLEMENTED".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )),
    }
}

/// Resume a suspended extension.
pub async fn resume_extension(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let svc = svc(&services)?;
    let ext_id = parse_ext_id(&id)?;

    match svc.resume(ext_id).await {
        Ok(_) => Ok(Json(serde_json::json!({ "resumed": true }))),
        Err(e) => Err((
            StatusCode::NOT_IMPLEMENTED,
            Json(ErrorBody {
                code: "NOT_IMPLEMENTED".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )),
    }
}

/// Send a message to an extension.
pub async fn send_message(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, Json<ErrorBody>)> {
    let svc = svc(&services)?;
    let ext_id = parse_ext_id(&id)?;

    let namespace = body.namespace.unwrap_or_else(|| "api".to_string());
    let action = ExtensionAction::Custom {
        namespace,
        name: body.action,
        payload: body.payload.unwrap_or(serde_json::Value::Null),
    };

    svc.send(ext_id, action).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "SEND_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    Ok(Json(SendMessageResponse {
        delivered: true,
        message: "message sent".to_string(),
    }))
}
