use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use torque_runtime::environment::RuntimeModelDriver;
use torque_runtime::message::RuntimeMessage;
use uuid::Uuid;

use crate::config;
use crate::models::v1::gating::{CandidateGenerationConfig, ExecutionSummary};
use crate::models::v1::memory::{MemoryContent, MemoryWriteCandidate, MemoryWriteCandidateStatus};

#[async_trait]
pub trait CandidateGenerator: Send + Sync {
    async fn generate_candidates(
        &self,
        execution_summary: &ExecutionSummary,
        config: &CandidateGenerationConfig,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>>;
}

pub struct OpenAICandidateGenerator {
    model_driver: Arc<dyn RuntimeModelDriver>,
}

impl OpenAICandidateGenerator {
    pub fn new(model_driver: Arc<dyn RuntimeModelDriver>) -> Self {
        Self { model_driver }
    }

    async fn extract_candidates_via_llm(
        &self,
        execution_summary: &ExecutionSummary,
        config: &CandidateGenerationConfig,
    ) -> anyhow::Result<Vec<MemoryContent>> {
        let tool_calls_text = execution_summary
            .tool_calls
            .iter()
            .take(20)
            .map(|t| format!("- {}: {}", t.tool_name, t.input))
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = format!(
            "You are a memory candidate extractor. Your job is to identify factual, recall-worthy information from agent executions.\n\n\
            Extract memory candidates as JSON array with objects:\n\
            [{{\"category\": \"agent_profile_memory|user_preference_memory|task_or_domain_memory|episodic_memory|external_context_memory\", \"key\": \"semantic_key\", \"value\": {{\"fact\": \"specific fact\"}}}}\n\n\
            Rules:\n\
            - Only extract information with genuine long-term value\n\
            - Keys should be semantic identifiers (e.g., \"prefers_dark_mode\", \"db_migration_pattern\")\n\
            - Values should be specific, factual statements\n\
            - Maximum {max} candidates per execution\n\
            - Minimum content length: {min} characters\n\
            - Excluded tools: {excluded}\n\
            - Categories: agent_profile_memory (agent behavior), user_preference_memory (user preferences), task_or_domain_memory (domain knowledge), episodic_memory (events/experiences), external_context_memory (references)\n\n\
            Respond ONLY with valid JSON array.",
            max = config.max_candidates_per_execution,
            min = config.min_content_length,
            excluded = config.excluded_tools.join(", ")
        );

        let user_prompt = format!(
            "You are a memory extraction system. Analyze the following execution and extract key information worth remembering.\n\n\
            Task Goal: {goal}\n\
            Execution Summary: {summary}\n\n\
            Tool Calls ({count} total):\n{tool_calls}",
            goal = execution_summary.goal,
            summary = execution_summary.output_summary,
            count = execution_summary.tool_calls.len(),
            tool_calls = tool_calls_text
        );

        let content = self
            .model_driver
            .chat(
                vec![
                    RuntimeMessage::system(&system_prompt),
                    RuntimeMessage::user(&user_prompt),
                ],
                Some(2000),
                Some(0.3),
            )
            .await?;
        let content = content.trim();

        let parsed: Vec<MemoryContent> = serde_json::from_str(content)
            .or_else(|_| {
                let cleaned = content
                    .trim()
                    .strip_prefix("```json")
                    .and_then(|s| s.strip_suffix("```"))
                    .unwrap_or(content);
                serde_json::from_str(cleaned.trim())
            })
            .unwrap_or_default();

        Ok(parsed)
    }
}

#[async_trait]
impl CandidateGenerator for OpenAICandidateGenerator {
    async fn generate_candidates(
        &self,
        execution_summary: &ExecutionSummary,
        config: &CandidateGenerationConfig,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>> {
        if !config.enabled {
            return Ok(vec![]);
        }

        let tool_names: Vec<&str> = execution_summary
            .tool_calls
            .iter()
            .map(|t| t.tool_name.as_str())
            .collect();
        let excluded: Vec<&str> = config.excluded_tools.iter().map(|s| s.as_str()).collect();
        if tool_names.iter().any(|t| excluded.contains(t))
            && execution_summary.tool_calls.len() == 1
        {
            return Ok(vec![]);
        }

        let contents = self
            .extract_candidates_via_llm(execution_summary, config)
            .await?;

        let candidates: Vec<MemoryWriteCandidate> = contents
            .into_iter()
            .take(config.max_candidates_per_execution)
            .map(|content| {
                let reasoning = format!(
                    "Extracted from task '{}' - {}",
                    execution_summary.task_id, execution_summary.goal
                );
                MemoryWriteCandidate {
                    id: Uuid::new_v4(),
                    agent_instance_id: execution_summary.agent_instance_id,
                    team_instance_id: None,
                    content: serde_json::to_value(content).unwrap_or_default(),
                    reasoning: Some(reasoning),
                    status: MemoryWriteCandidateStatus::Pending,
                    memory_entry_id: None,
                    reviewed_by: None,
                    created_at: Utc::now(),
                    reviewed_at: None,
                    updated_at: Utc::now(),
                }
            })
            .collect();

        Ok(candidates)
    }
}

pub struct NoOpCandidateGenerator;

#[async_trait]
impl CandidateGenerator for NoOpCandidateGenerator {
    async fn generate_candidates(
        &self,
        _execution_summary: &ExecutionSummary,
        _config: &CandidateGenerationConfig,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>> {
        Ok(vec![])
    }
}
