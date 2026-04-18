use async_trait::async_trait;

#[async_trait]
pub trait EmbeddingGenerator: Send + Sync {
    async fn generate(&self, text: &str) -> anyhow::Result<Vec<f32>>;
    fn dimensions(&self) -> usize;
    fn model_name(&self) -> &str;
}

pub struct OpenAIEmbeddingGenerator {
    http_client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    dimensions: usize,
}

impl OpenAIEmbeddingGenerator {
    pub fn new(api_key: String) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http_client,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key,
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
        }
    }

    pub fn with_model(mut self, model: String, dimensions: usize) -> Self {
        self.model = model;
        self.dimensions = dimensions;
        self
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("LLM_API_KEY"))
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY or LLM_API_KEY must be set"))?;
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-3-small".to_string());
        let dimensions = std::env::var("EMBEDDING_DIMENSIONS")
            .ok()
            .and_then(|d| d.parse().ok())
            .unwrap_or(1536);

        Ok(Self::new(api_key)
            .with_base_url(base_url)
            .with_model(model, dimensions))
    }
}

#[async_trait]
impl EmbeddingGenerator for OpenAIEmbeddingGenerator {
    async fn generate(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let url = format!("{}/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "input": text,
        });

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
            anyhow::bail!("Embedding API error: {} - {}", status, error_text);
        }

        #[derive(serde::Deserialize)]
        struct EmbeddingResponse {
            data: Vec<EmbeddingData>,
        }

        #[derive(serde::Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        let body: EmbeddingResponse = response.json().await?;
        let embedding = body
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| anyhow::anyhow!("No embedding data in response"))?;

        if embedding.len() != self.dimensions {
            anyhow::bail!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            );
        }

        Ok(embedding)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

pub fn memory_to_embedding_text(content: &crate::models::v1::memory::MemoryContent) -> String {
    let category_str = match content.category {
        crate::models::v1::memory::MemoryCategory::AgentProfileMemory => "agent profile",
        crate::models::v1::memory::MemoryCategory::UserPreferenceMemory => "user preference",
        crate::models::v1::memory::MemoryCategory::TaskOrDomainMemory => "task domain",
        crate::models::v1::memory::MemoryCategory::EpisodicMemory => "episodic",
        crate::models::v1::memory::MemoryCategory::ExternalContextMemory => "external context",
    };
    let value_str = match &content.value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) if map.len() == 1 => {
            map.iter().next().map(|(k, v)| {
                format!("{}: {}", k, v.as_str().unwrap_or(&v.to_string()))
            }).unwrap_or_else(|| content.value.to_string())
        }
        _ => content.value.to_string(),
    };
    format!("{}: {} - {}", category_str, content.key, value_str)
}
