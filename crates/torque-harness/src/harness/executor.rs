use crate::agent::stream::StreamEvent;
use crate::harness::planner::{NodeStatus, Planner, SubTask};
use crate::infra::llm::LlmClient;
use crate::infra::tool_registry::ToolRegistry;
use crate::policy::ToolGovernanceService;
use crate::service::governed_tool::GovernedToolRegistry;
use crate::service::reflexion::ReflexionService;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub id: Uuid,
    pub goal: String,
    pub nodes: Vec<ExecutionNode>,
    pub suggested_parallelism: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub id: Uuid,
    pub description: String,
    pub status: NodeStatus,
    #[serde(default)]
    pub depends_on: Vec<Uuid>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

pub struct PlanExecutor {
    react: crate::harness::ReActHarness,
    reflexion: Option<Arc<ReflexionService>>,
    max_concurrency: usize,
}

impl PlanExecutor {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        tool_governance: Arc<ToolGovernanceService>,
        reflexion: Option<Arc<ReflexionService>>,
    ) -> Self {
        let governed_registry = Arc::new(GovernedToolRegistry::new(tools, tool_governance));
        Self {
            react: crate::harness::ReActHarness::new(llm, Arc::new(crate::harness::react::ToolExecution::Governed(governed_registry))),
            reflexion,
            max_concurrency: 3,
        }
    }

    pub fn with_max_concurrency(mut self, max: usize) -> Self {
        self.max_concurrency = max;
        self
    }

    pub async fn execute(
        &mut self,
        graph: &mut TaskGraph,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<ExecutorResult> {
        let mut completed_count = 0;
        let total_nodes = graph.nodes.len();

        while completed_count < total_nodes {
            let ready_nodes = self.get_ready_nodes(graph);

            if ready_nodes.is_empty() {
                if let Some(failed) = graph.nodes.iter().find(|n| n.status == NodeStatus::Failed) {
                    return Ok(ExecutorResult {
                        success: false,
                        summary: format!(
                            "Task failed: {}",
                            failed.error.as_deref().unwrap_or("Unknown error")
                        ),
                        failed_node_id: Some(failed.id),
                        completed_nodes: completed_count,
                        total_nodes,
                    });
                }
                break;
            }

            let to_execute: Vec<_> = ready_nodes.into_iter().take(self.max_concurrency).collect();

            for node_id in to_execute {
                if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.status = NodeStatus::Running;
                    let _ = event_sink
                        .send(StreamEvent::Chunk {
                            content: format!("Starting subtask: {}\n", node.description),
                        })
                        .await;

                    let result = self
                        .execute_node(&node.description, event_sink.clone())
                        .await;

                    match result {
                        Ok(outcome) => {
                            node.status = NodeStatus::Completed;
                            node.result = Some(outcome.clone());
                            completed_count += 1;

                            if let Some(ref reflexion) = self.reflexion {
                                let subtask_result = crate::service::reflexion::SubtaskResult {
                                    task_id: node.id,
                                    plan_id: graph.id,
                                    input: Some(node.description.clone()),
                                    output: Some(outcome.clone()),
                                    duration_ms: None,
                                    status: "completed".to_string(),
                                    error_message: None,
                                };
                                let _ = reflexion.log_subtask(subtask_result).await;
                            }
                        }
                        Err(e) => {
                            node.status = NodeStatus::Failed;
                            node.error = Some(e.to_string());

                            if let Some(ref reflexion) = self.reflexion {
                                let subtask_result = crate::service::reflexion::SubtaskResult {
                                    task_id: node.id,
                                    plan_id: graph.id,
                                    input: Some(node.description.clone()),
                                    output: None,
                                    duration_ms: None,
                                    status: "failed".to_string(),
                                    error_message: Some(e.to_string()),
                                };
                                let _ = reflexion.log_subtask(subtask_result).await;

                                if let Ok(reflection) = reflexion
                                    .reflect_on_failure(node.id, graph.id, &e.to_string())
                                    .await
                                {
                                    tracing::info!(
                                        "Reflection on failure: root_cause={}, confidence={}",
                                        reflection.root_cause,
                                        reflection.confidence
                                    );
                                }
                            }

                            return Ok(ExecutorResult {
                                success: false,
                                summary: format!(
                                    "Subtask '{}' failed: {}",
                                    node.description,
                                    e.to_string()
                                ),
                                failed_node_id: Some(node.id),
                                completed_nodes: completed_count,
                                total_nodes,
                            });
                        }
                    }
                }
            }
        }

        let summary = self.aggregate_results(graph);
        Ok(ExecutorResult {
            success: true,
            summary,
            failed_node_id: None,
            completed_nodes: completed_count,
            total_nodes,
        })
    }

    fn get_ready_nodes(&self, graph: &TaskGraph) -> Vec<Uuid> {
        let mut ready = Vec::new();
        let mut completed: HashSet<Uuid> = graph
            .nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Completed)
            .map(|n| n.id)
            .collect();

        for node in &graph.nodes {
            if node.status != NodeStatus::Pending {
                continue;
            }

            let all_deps_complete = node
                .depends_on
                .iter()
                .all(|dep_id| completed.contains(dep_id));
            if all_deps_complete {
                ready.push(node.id);
            }
        }

        ready
    }

    async fn execute_node(
        &mut self,
        description: &str,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<String> {
        let result = self.react.run(description, None, event_sink).await?;

        match result {
            torque_kernel::StepDecision::CompleteTask(summary) => Ok(summary),
            torque_kernel::StepDecision::FailTask(reason) => {
                anyhow::bail!("Task failed: {}", reason)
            }
            _ => Ok("Task completed (no summary)".to_string()),
        }
    }

    fn aggregate_results(&self, graph: &TaskGraph) -> String {
        let mut lines = vec![format!("Task graph '{}' completed:\n", graph.goal)];

        for node in &graph.nodes {
            let status_str = match node.status {
                NodeStatus::Completed => "✓",
                NodeStatus::Failed => "✗",
                NodeStatus::Skipped => "○",
                _ => "?",
            };
            lines.push(format!(
                "  {} {}: {}",
                status_str,
                node.description,
                node.result.as_deref().unwrap_or("")
            ));
        }

        lines.join("\n")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorResult {
    pub success: bool,
    pub summary: String,
    pub failed_node_id: Option<Uuid>,
    pub completed_nodes: usize,
    pub total_nodes: usize,
}

pub struct PlanningExecutor {
    planner: Planner,
    executor: PlanExecutor,
}

impl PlanningExecutor {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        tool_governance: Arc<ToolGovernanceService>,
        reflexion: Option<Arc<ReflexionService>>,
    ) -> Self {
        Self {
            planner: Planner::new(llm.clone()),
            executor: PlanExecutor::new(llm, tools, tool_governance, reflexion),
        }
    }

    pub async fn plan_and_execute(
        &mut self,
        goal: &str,
        context: Option<&str>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<ExecutorResult> {
        let _ = event_sink
            .send(StreamEvent::Chunk {
                content: format!("Planning: {}\n", goal),
            })
            .await;

        let planned = self.planner.plan(goal, context).await?;
        let mut graph = self.planner.to_task_graph(&planned);

        let _ = event_sink
            .send(StreamEvent::Chunk {
                content: format!(
                    "Planned {} subtasks (parallelism: {})\n",
                    graph.nodes.len(),
                    graph.suggested_parallelism
                ),
            })
            .await;

        self.executor.execute(&mut graph, event_sink).await
    }
}
