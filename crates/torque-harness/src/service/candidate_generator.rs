use crate::config;
use crate::models::v1::gating::{CandidateGenerationConfig, ExecutionSummary};
use crate::models::v1::memory::{MemoryContent, MemoryWriteCandidate, MemoryWriteCandidateStatus};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

#[async_trait]
pub trait CandidateGenerator: Send + Sync {
    async fn generate_candidates(
        &self,
        execution_summary: &ExecutionSummary,
        config: &CandidateGenerationConfig,
    ) -> anyhow::Result<Vec<MemoryWriteCandidate>>;
}

pub struct OpenAICandidateGenerator {
    http_client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAICandidateGenerator {
    pub fn new() -> anyhow::Result<Self> {
        let api_key = config::extraction_api_key()
            .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY or LLM_API_KEY must be set"))?;
        let base_url = config::extraction_api_base();
        let model = config::extraction_model();

        Ok(Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            api_key,
            base_url,
            model,
        })
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

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "temperature": 0.3,
            "max_tokens": 2000
        });

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error: {} - {}", status, error_text);
        }

        #[derive(serde::Deserialize)]
        struct ChatResponse {
            choices: Vec<Choice>,
        }
        #[derive(serde::Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(serde::Deserialize)]
        struct Message {
            content: String,
        }

        let chat_response: ChatResponse = response.json().await?;
        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.trim())
            .unwrap_or("[]");

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
