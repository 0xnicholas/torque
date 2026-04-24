use super::{Tool, ToolArc, ToolResult};
use crate::models::v1::artifact::ArtifactScope;
use crate::service::artifact::{ArtifactService, TODO_DOCUMENT_KIND};
use anyhow::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub status: TodoStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TodoDocument {
    #[serde(default)]
    pub items: Vec<TodoItem>,
}

pub fn create_todo_tools(artifact_service: Arc<ArtifactService>) -> Vec<ToolArc> {
    vec![
        Arc::new(WriteTodosTool::new(artifact_service.clone())) as ToolArc,
        Arc::new(ReadTodosTool::new(artifact_service.clone())) as ToolArc,
        Arc::new(UpdateTodoTool::new(artifact_service)) as ToolArc,
    ]
}

#[derive(Debug, Deserialize)]
struct WriteTodosArgs {
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    replace: bool,
    items: Vec<TodoItem>,
}

#[derive(Debug, Deserialize)]
struct ReadTodosArgs {
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTodoArgs {
    #[serde(default)]
    scope: Option<String>,
    id: String,
    status: TodoStatus,
}

pub struct WriteTodosTool {
    artifacts: Arc<ArtifactService>,
}

impl WriteTodosTool {
    pub fn new(artifacts: Arc<ArtifactService>) -> Self {
        Self { artifacts }
    }
}

#[async_trait]
impl Tool for WriteTodosTool {
    fn name(&self) -> &str {
        "write_todos"
    }

    fn description(&self) -> &str {
        "Write todo items into the todo scratchpad"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": { "type": "string", "description": "Artifact scope (private, team_shared, external_published)" },
                "replace": { "type": "boolean", "description": "When true, replace all existing todos for the scope" },
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "blocked"] },
                            "notes": { "type": "string" }
                        },
                        "required": ["id", "content"]
                    }
                }
            },
            "required": ["items"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let parsed: WriteTodosArgs = serde_json::from_value(args).context("invalid write_todos args")?;
        let scope = parse_scope(parsed.scope.as_deref())?;
        let mut document = read_document(&self.artifacts, copy_scope(&scope)).await?;

        if parsed.replace {
            document.items = parsed.items;
        } else {
            for item in parsed.items {
                upsert_item(&mut document.items, item);
            }
        }

        persist_document(&self.artifacts, scope, &document).await?;
        Ok(success_result(document)?)
    }
}

pub struct ReadTodosTool {
    artifacts: Arc<ArtifactService>,
}

impl ReadTodosTool {
    pub fn new(artifacts: Arc<ArtifactService>) -> Self {
        Self { artifacts }
    }
}

#[async_trait]
impl Tool for ReadTodosTool {
    fn name(&self) -> &str {
        "read_todos"
    }

    fn description(&self) -> &str {
        "Read todo items from the todo scratchpad"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": { "type": "string", "description": "Artifact scope (private, team_shared, external_published)" }
            }
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let parsed: ReadTodosArgs = serde_json::from_value(args).context("invalid read_todos args")?;
        let scope = parse_scope(parsed.scope.as_deref())?;
        let document = read_document(&self.artifacts, scope).await?;
        success_result(document)
    }
}

pub struct UpdateTodoTool {
    artifacts: Arc<ArtifactService>,
}

impl UpdateTodoTool {
    pub fn new(artifacts: Arc<ArtifactService>) -> Self {
        Self { artifacts }
    }
}

#[async_trait]
impl Tool for UpdateTodoTool {
    fn name(&self) -> &str {
        "update_todo"
    }

    fn description(&self) -> &str {
        "Update one todo item's status (and optional notes)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": { "type": "string", "description": "Artifact scope (private, team_shared, external_published)" },
                "id": { "type": "string" },
                "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "blocked"] },
                "notes": { "type": "string" }
            },
            "required": ["id", "status"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let parsed: UpdateTodoArgs =
            serde_json::from_value(args.clone()).context("invalid update_todo args")?;
        let scope = parse_scope(parsed.scope.as_deref())?;
        let notes_update = extract_notes_update(&args);

        let mut document = read_document(&self.artifacts, copy_scope(&scope)).await?;
        let todo = document
            .items
            .iter_mut()
            .find(|item| item.id == parsed.id)
            .ok_or_else(|| anyhow::anyhow!("todo item '{}' not found", parsed.id))?;

        todo.status = parsed.status;
        if let Some(notes) = notes_update {
            todo.notes = notes;
        }

        persist_document(&self.artifacts, scope, &document).await?;
        success_result(document)
    }
}

async fn read_document(
    artifact_service: &ArtifactService,
    scope: ArtifactScope,
) -> anyhow::Result<TodoDocument> {
    if let Some(artifact) = artifact_service
        .latest_by_kind_and_scope(TODO_DOCUMENT_KIND, scope)
        .await?
    {
        return serde_json::from_value(artifact.content)
            .context("stored todo_document is not valid TodoDocument");
    }
    Ok(TodoDocument::default())
}

async fn persist_document(
    artifact_service: &ArtifactService,
    scope: ArtifactScope,
    document: &TodoDocument,
) -> anyhow::Result<()> {
    artifact_service
        .create_json_document(
            TODO_DOCUMENT_KIND,
            scope,
            serde_json::to_value(document).context("serialize TodoDocument")?,
        )
        .await?;
    Ok(())
}

fn parse_scope(scope: Option<&str>) -> anyhow::Result<ArtifactScope> {
    let normalized = scope.unwrap_or("private").to_ascii_lowercase();
    match normalized.as_str() {
        "private" => Ok(ArtifactScope::Private),
        "team_shared" | "teamshared" => Ok(ArtifactScope::TeamShared),
        "external_published" | "externalpublished" => Ok(ArtifactScope::ExternalPublished),
        _ => Err(anyhow::anyhow!("invalid scope '{}'", normalized)),
    }
}

fn upsert_item(items: &mut Vec<TodoItem>, item: TodoItem) {
    if let Some(existing) = items.iter_mut().find(|existing| existing.id == item.id) {
        *existing = item;
    } else {
        items.push(item);
    }
}

fn success_result(document: TodoDocument) -> anyhow::Result<ToolResult> {
    Ok(ToolResult {
        success: true,
        content: serde_json::to_string(&document)?,
        error: None,
    })
}

fn extract_notes_update(args: &Value) -> Option<Option<String>> {
    if !args.as_object().is_some_and(|obj| obj.contains_key("notes")) {
        return None;
    }
    Some(
        args.get("notes")
            .and_then(|value| value.as_str().map(ToString::to_string)),
    )
}

fn copy_scope(scope: &ArtifactScope) -> ArtifactScope {
    match scope {
        ArtifactScope::Private => ArtifactScope::Private,
        ArtifactScope::TeamShared => ArtifactScope::TeamShared,
        ArtifactScope::ExternalPublished => ArtifactScope::ExternalPublished,
    }
}
