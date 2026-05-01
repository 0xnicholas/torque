use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::Value;

use crate::db::Database;
use crate::models::v1::common::ErrorBody;
use crate::models::v1::tool::{
    ToolInfo, ToolListResponse, ToolRegisterRequest, ToolRegisterResponse, ToolUpdateRequest,
};
use crate::service::ServiceContainer;
use crate::tools::{Tool, ToolArc, ToolResult};
use llm::LlmClient;

type ApiState = (Database, Arc<dyn LlmClient>, Arc<ServiceContainer>);

// ── Dynamic Tool ────────────────────────────────────────────────────

/// A tool whose definition was provided at runtime through the HTTP API.
///
/// This wraps the name, description, parameters schema, and source metadata
/// provided by the caller. The execute function performs a simple echo;
/// for custom execution logic, users should register tools through
/// the Extension system instead.
struct DynamicTool {
    name: String,
    description: String,
    parameters_schema: Value,
}

#[async_trait::async_trait]
impl Tool for DynamicTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.parameters_schema.clone()
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            content: serde_json::to_string_pretty(&args)
                .unwrap_or_else(|_| "{}".to_string()),
            error: None,
        })
    }
}

// ── Handlers ────────────────────────────────────────────────────────

/// POST /v1/tools/register — Register a new tool at runtime.
///
/// The tool becomes immediately available to LLM agents on their next turn.
/// Returns `409 Conflict` if a tool with the same name already exists.
pub async fn register(
    State((_, _, services)): State<ApiState>,
    Json(req): Json<ToolRegisterRequest>,
) -> Result<(StatusCode, Json<ToolRegisterResponse>), (StatusCode, Json<ErrorBody>)> {
    // Check for name conflict.
    if services.tool.get_tool(&req.name).await.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorBody {
                code: "CONFLICT".to_string(),
                message: format!("Tool '{}' already exists; use PUT to update", req.name),
                details: None,
                request_id: None,
            }),
        ));
    }

    let tool = DynamicTool {
        name: req.name.clone(),
        description: req.description,
        parameters_schema: req.parameters_schema,
    };

    services.tool.register_tool(Arc::new(tool)).await;

    Ok((
        StatusCode::CREATED,
        Json(ToolRegisterResponse {
            name: req.name,
            message: "Tool registered successfully. It will be available to LLM agents on their next turn.".to_string(),
        }),
    ))
}

/// GET /v1/tools — List all registered tools with metadata.
pub async fn list(
    State((_, _, services)): State<ApiState>,
) -> Result<Json<ToolListResponse>, (StatusCode, Json<ErrorBody>)> {
    let tools = services.tool.registry().list().await;
    let total = tools.len();
    let tool_infos: Vec<ToolInfo> = tools
        .into_iter()
        .map(|t| ToolInfo {
            name: t.name().to_string(),
            description: t.description().to_string(),
            parameters_schema: t.parameters_schema(),
            source: "manual".to_string(),
        })
        .collect();

    Ok(Json(ToolListResponse {
        tools: tool_infos,
        total,
    }))
}

/// DELETE /v1/tools/:name — Remove a tool from the registry.
pub async fn delete(
    State((_, _, services)): State<ApiState>,
    Path(name): Path<String>,
) -> Result<(StatusCode, Json<ToolRegisterResponse>), (StatusCode, Json<ErrorBody>)> {
    let removed = services.tool.unregister_tool(&name).await;
    if removed {
        Ok((
            StatusCode::OK,
            Json(ToolRegisterResponse {
                name,
                message: "Tool removed successfully.".to_string(),
            }),
        ))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                code: "NOT_FOUND".to_string(),
                message: format!("Tool '{}' not found", name),
                details: None,
                request_id: None,
            }),
        ))
    }
}

/// PUT /v1/tools/:name — Update an existing tool's definition.
pub async fn update(
    State((_, _, services)): State<ApiState>,
    Path(name): Path<String>,
    Json(req): Json<ToolUpdateRequest>,
) -> Result<(StatusCode, Json<ToolRegisterResponse>), (StatusCode, Json<ErrorBody>)> {
    // Fetch the existing tool to preserve fields not being updated.
    let existing = services.tool.get_tool(&name).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                code: "NOT_FOUND".to_string(),
                message: format!("Tool '{}' not found; use POST /v1/tools/register to create", name),
                details: None,
                request_id: None,
            }),
        )
    })?;

    let updated = DynamicTool {
        description: req.description.unwrap_or_else(|| existing.description().to_string()),
        parameters_schema: req
            .parameters_schema
            .unwrap_or_else(|| existing.parameters_schema()),
        name: name.clone(),
    };

    services.tool.registry().update(&name, Arc::new(updated)).await;

    Ok((
        StatusCode::OK,
        Json(ToolRegisterResponse {
            name,
            message: "Tool updated successfully.".to_string(),
        }),
    ))
}
