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
    pub scope_key: String,
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

#[derive(Debug)]
struct TodoScope {
    key: String,
    artifact_scope: ArtifactScope,
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
                "scope": { "type": "string", "description": "Logical todo scope key (e.g. private, sprint_42, team_shared)" },
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
        let mut document = read_document(&self.artifacts, &scope).await?;

        if parsed.replace {
            document.items = parsed.items;
        } else {
            for item in parsed.items {
                upsert_item(&mut document.items, item);
            }
        }

        persist_document(&self.artifacts, &scope, &document).await?;
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
                "scope": { "type": "string", "description": "Logical todo scope key (e.g. private, sprint_42, team_shared)" }
            }
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let parsed: ReadTodosArgs = serde_json::from_value(args).context("invalid read_todos args")?;
        let scope = parse_scope(parsed.scope.as_deref())?;
        let document = read_document(&self.artifacts, &scope).await?;
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
                "scope": { "type": "string", "description": "Logical todo scope key (e.g. private, sprint_42, team_shared)" },
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

        let mut document = read_document(&self.artifacts, &scope).await?;
        let todo = document
            .items
            .iter_mut()
            .find(|item| item.id == parsed.id)
            .ok_or_else(|| anyhow::anyhow!("todo item '{}' not found", parsed.id))?;

        todo.status = parsed.status;
        if let Some(notes) = notes_update {
            todo.notes = notes;
        }

        persist_document(&self.artifacts, &scope, &document).await?;
        success_result(document)
    }
}

async fn read_document(
    artifact_service: &ArtifactService,
    scope: &TodoScope,
) -> anyhow::Result<TodoDocument> {
    if let Some(artifact) = artifact_service
        .find_latest_by_kind_scope_and_content_string(
            TODO_DOCUMENT_KIND,
            copy_scope(&scope.artifact_scope),
            "scope_key",
            &scope.key,
        )
        .await?
    {
        let mut document: TodoDocument = serde_json::from_value(artifact.content)
            .context("stored todo_document is not valid TodoDocument")?;
        if document.scope_key.is_empty() {
            document.scope_key = scope.key.clone();
        }
        return Ok(document);
    }
    Ok(TodoDocument {
        scope_key: scope.key.clone(),
        items: vec![],
    })
}

async fn persist_document(
    artifact_service: &ArtifactService,
    scope: &TodoScope,
    document: &TodoDocument,
) -> anyhow::Result<()> {
    let mut payload = document.clone();
    payload.scope_key = scope.key.clone();
    artifact_service
        .create_json_document(
            TODO_DOCUMENT_KIND,
            copy_scope(&scope.artifact_scope),
            serde_json::to_value(payload).context("serialize TodoDocument")?,
        )
        .await?;
    Ok(())
}

fn parse_scope(scope: Option<&str>) -> anyhow::Result<TodoScope> {
    let raw = scope.unwrap_or("private").trim();
    if raw.is_empty() {
        return Err(anyhow::anyhow!("scope must not be empty"));
    }

    let normalized = raw.to_ascii_lowercase();
    let artifact_scope = match normalized.as_str() {
        "team_shared" | "teamshared" => ArtifactScope::TeamShared,
        "external_published" | "externalpublished" => ArtifactScope::ExternalPublished,
        _ => ArtifactScope::Private,
    };

    Ok(TodoScope {
        key: normalized,
        artifact_scope,
    })
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
