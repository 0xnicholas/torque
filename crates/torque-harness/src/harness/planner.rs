use crate::harness::executor::{ExecutionNode, TaskGraph};
use crate::infra::llm::{LlmClient, LlmMessage};
use llm::ChatRequest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: Uuid,
    pub description: String,
    pub depends_on: Vec<Uuid>,
    pub estimated_duration_ms: Option<u64>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    pub id: Uuid,
    pub goal: String,
    pub tasks: Vec<SubTask>,
    pub suggested_parallelism: usize,
}

pub struct Planner {
    llm: Arc<dyn LlmClient>,
    max_tasks: usize,
}

impl Planner {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm, max_tasks: 10 }
    }

    pub fn with_max_tasks(mut self, max: usize) -> Self {
        self.max_tasks = max;
        self
    }

    pub async fn plan(&self, goal: &str, context: Option<&str>) -> anyhow::Result<PlannedTask> {
        let decomposition = self.decompose_task(goal, context).await?;

        let planned = PlannedTask {
            id: Uuid::new_v4(),
            goal: goal.to_string(),
            tasks: decomposition,
            suggested_parallelism: self.estimate_parallelism(),
        };

        Ok(planned)
    }

    async fn decompose_task(
        &self,
        goal: &str,
        context: Option<&str>,
    ) -> anyhow::Result<Vec<SubTask>> {
        let system_prompt = r#"You are a task decomposition planner. Break down the given task into 3-8 subtasks.

Each subtask should:
- Be independently executable
- Have a clear completion criterion
- Be atomic (not split further)

Respond ONLY with JSON array:
[{"id": "uuid", "description": "what to do", "depends_on": ["uuid1", "uuid2"], "estimated_duration_ms": null}]

The first task should have empty depends_on. Subsequent tasks should list IDs of tasks they depend on.

Generate 3-8 subtasks maximum."#;

        let user_prompt = if let Some(ctx) = context {
            format!(
                "Goal: {}\n\nContext:\n{}\n\nDecompose this into subtasks.",
                goal, ctx
            )
        } else {
            format!("Goal: {}\n\nDecompose this into subtasks.", goal)
        };

        let request = ChatRequest::new(
            self.llm.model().to_string(),
            vec![
                LlmMessage::system(system_prompt),
                LlmMessage::user(&user_prompt),
            ],
        );

        let response = self
            .llm
            .chat(request)
            .await
            .map_err(|e| anyhow::anyhow!("Planner LLM error: {}", e))?;

        let content = response.message.content.as_ref();

        let tasks: Vec<SubTask> = serde_json::from_str(content).unwrap_or_else(|_| {
            serde_json::from_str::<serde_json::Value>(content)
                .ok()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_else(|| self.default_decomposition(goal))
        });

        Ok(tasks)
    }

    fn default_decomposition(&self, goal: &str) -> Vec<SubTask> {
        vec![SubTask {
            id: Uuid::new_v4(),
            description: goal.to_string(),
            depends_on: vec![],
            estimated_duration_ms: None,
            metadata: serde_json::json!({}),
        }]
    }

    fn estimate_parallelism(&self) -> usize {
        2
    }

    pub fn to_task_graph(&self, planned: &PlannedTask) -> TaskGraph {
        let nodes: Vec<ExecutionNode> = planned
            .tasks
            .iter()
            .map(|t| ExecutionNode {
                id: t.id,
                description: t.description.clone(),
                status: NodeStatus::Pending,
                depends_on: t.depends_on.clone(),
                result: None,
                error: None,
            })
            .collect();

        TaskGraph {
            id: planned.id,
            goal: planned.goal.clone(),
            nodes,
            suggested_parallelism: planned.suggested_parallelism,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl Default for NodeStatus {
    fn default() -> Self {
        NodeStatus::Pending
    }
}
