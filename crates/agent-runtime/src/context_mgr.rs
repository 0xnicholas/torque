use checkpointer::r#trait::{ArtifactPointer, Message};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub covers_range: (usize, usize),
    pub content: String,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum CompressionStrategy {
    KeepLastN(usize),
    SummarizeOlder { summarize_count: usize },
    ExtractiveCompression,
}

pub struct ContextManager {
    pub max_tokens: usize,
    pub warning_threshold: f64,
    pub compression_strategy: CompressionStrategy,
    summarizer: Option<Box<dyn Summarizer>>,
    pub db: sqlx::PgPool,
    pub node_id: Uuid,
    pub full_history: Vec<Message>,
    pub compressed_context: Vec<Message>,
    pub summary_chain: Vec<Summary>,
    pub intermediate_results: Vec<ArtifactPointer>,
}

#[async_trait::async_trait]
pub trait Summarizer: Send + Sync {
    async fn summarize(&self, messages: &[Message]) -> anyhow::Result<String>;
}

impl ContextManager {
    pub fn new(
        max_tokens: usize,
        strategy: CompressionStrategy,
        summarizer: Option<Box<dyn Summarizer>>,
        db: sqlx::PgPool,
        node_id: Uuid,
    ) -> Self {
        Self {
            max_tokens,
            warning_threshold: 0.8,
            compression_strategy: strategy,
            summarizer,
            db,
            node_id,
            full_history: Vec::new(),
            compressed_context: Vec::new(),
            summary_chain: Vec::new(),
            intermediate_results: Vec::new(),
        }
    }

    pub fn get_intermediate_results(&self) -> Vec<ArtifactPointer> {
        self.intermediate_results.clone()
    }

    pub async fn add_intermediate_result(
        &mut self,
        artifact: ArtifactPointer,
    ) -> anyhow::Result<()> {
        self.intermediate_results.push(artifact);
        self.persist_to_logs().await?;
        Ok(())
    }

    pub async fn add_message(&mut self, msg: Message) -> anyhow::Result<()> {
        self.full_history.push(msg.clone());
        self.compressed_context.push(msg.clone());
        self.persist_to_logs().await?;

        let token_count = self.estimate_tokens();
        if token_count as f64 > self.max_tokens as f64 * self.warning_threshold {
            self.compress().await?;
        }

        Ok(())
    }

    pub async fn persist_to_logs(&self) -> anyhow::Result<()> {
        let logs_json = serde_json::json!({
            "type": "context_snapshot",
            "full_history": self.full_history,
            "summary_chain": self.summary_chain,
        });

        sqlx::query(r#"UPDATE node_logs SET tool_calls = tool_calls || $1::jsonb WHERE node_id = $2"#)
            .bind(&logs_json)
            .bind(self.node_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    pub fn get_compressed_context(&self) -> &[Message] {
        &self.compressed_context
    }

    pub fn get_full_history(&self) -> Vec<Message> {
        self.full_history.clone()
    }

    pub fn get_summary_chain(&self) -> &[Summary] {
        &self.summary_chain
    }

    fn estimate_tokens(&self) -> usize {
        self.full_history.iter().map(|m| m.content.len() / 4).sum()
    }

    pub async fn compress(&mut self) -> anyhow::Result<()> {
        match &self.compression_strategy {
            CompressionStrategy::KeepLastN(n) => {
                let to_keep = self.full_history.len().saturating_sub(*n);
                self.compressed_context = self.full_history[to_keep..].to_vec();
            }
            CompressionStrategy::SummarizeOlder { summarize_count } => {
                if let Some(ref summarizer) = self.summarizer {
                    let to_summarize = &self.full_history[0..*summarize_count];
                    match summarizer.summarize(to_summarize).await {
                        Ok(summary) => {
                            self.compressed_context = vec![Message {
                                role: "system".into(),
                                content: format!("Previous conversation summary: {}", summary),
                            }];
                            self.compressed_context
                                .extend(self.full_history[*summarize_count..].to_vec());
                            self.summary_chain.push(Summary {
                                covers_range: (0, *summarize_count),
                                content: summary,
                                created_at: chrono::Utc::now(),
                            });
                        }
                        Err(e) => anyhow::bail!("Summarization failed: {}", e),
                    }
                }
            }
            CompressionStrategy::ExtractiveCompression => {
                self.compressed_context = self
                    .full_history
                    .iter()
                    .filter(|m| m.role == "tool" || m.content.len() > 100)
                    .cloned()
                    .collect();
            }
        }
        Ok(())
    }
}
