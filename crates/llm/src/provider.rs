//! Provider abstraction for LLM backends.
//!
//! A [`Provider`] is a factory that creates [`LlmClient`] instances.
//! It is separate from the client trait so that provider metadata
//! (type, config) can be queried independently of the client lifecycle.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::client::LlmClient;
use super::error::LlmError;

/// Identifies a specific LLM provider backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    /// OpenAI and OpenAI-compatible APIs (e.g. Azure, Groq, Together)
    OpenAI,
    /// Anthropic (Claude models)
    Anthropic,
    /// Google (Gemini models)
    Google,
    /// Ollama (local models)
    Ollama,
    /// A user-defined custom provider with an arbitrary name.
    Custom(String),
}

impl ProviderType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::Ollama => "ollama",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAI,
            "anthropic" => Self::Anthropic,
            "google" => Self::Google,
            "ollama" => Self::Ollama,
            other => Self::Custom(other.to_string()),
        }
    }
}

// ─── ProviderConfig ──────────────────────────────────────────

/// Standardized configuration for any LLM provider.
///
/// Different providers may use different subsets of these fields.
/// The `extra` map carries provider-specific parameters (e.g.
/// `api_version` for Azure, `org_id` for OpenAI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider type identifier (required).
    pub provider_type: ProviderType,

    /// Base URL for the provider's API endpoint.
    /// - OpenAI: defaults to `https://api.openai.com/v1`
    /// - Ollama: typically `http://localhost:11434/v1`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// API key for authentication.
    /// For local providers (Ollama), this may be empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Default model to use when none is specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,

    /// Provider-specific extra parameters.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

impl ProviderConfig {
    /// Create a config from environment variables using conventional names.
    ///
    /// Supported env vars:
    /// - `LLM_PROVIDER` (default: `"openai"`)
    /// - `LLM_BASE_URL`
    /// - `LLM_API_KEY`
    /// - `LLM_AGENT_MODEL`
    pub fn from_env() -> Self {
        let provider_type = std::env::var("LLM_PROVIDER")
            .ok()
            .map(|s| ProviderType::from_str(&s))
            .unwrap_or(ProviderType::OpenAI);

        let base_url = std::env::var("LLM_BASE_URL").ok();

        let api_key = std::env::var("LLM_API_KEY").ok();

        let default_model = std::env::var("LLM_AGENT_MODEL").ok();

        Self {
            provider_type,
            base_url,
            api_key,
            default_model,
            extra: HashMap::new(),
        }
    }

    /// Attach a provider-specific extra parameter.
    pub fn with_extra(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// Fill in defaults for fields that are absent based on [`ProviderType`].
    pub fn with_defaults(self) -> Self {
        let base_url = self.base_url.or_else(|| match self.provider_type {
            ProviderType::OpenAI => Some("https://api.openai.com/v1".into()),
            ProviderType::Ollama => Some("http://localhost:11434/v1".into()),
            _ => None,
        });

        let default_model = self.default_model.or_else(|| match self.provider_type {
            ProviderType::OpenAI => Some("gpt-4o-mini".into()),
            ProviderType::Ollama => Some("llama3".into()),
            _ => None,
        });

        Self {
            base_url,
            default_model,
            ..self
        }
    }

    /// Validate the config, returning a list of issues.
    ///
    /// A config that requires authentication but has no `api_key`
    /// (for cloud providers) is invalid. Local providers (`Ollama`)
    /// and custom endpoints may omit the key.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        match &self.provider_type {
            ProviderType::OpenAI
            | ProviderType::Anthropic
            | ProviderType::Google => {
                if self.api_key.as_deref().map_or(true, str::is_empty) {
                    issues.push(format!(
                        "api_key is required for provider '{}'",
                        self.provider_type.as_str()
                    ));
                }
            }
            ProviderType::Ollama | ProviderType::Custom(_) => {
                // api_key is optional for local / custom providers
            }
        }

        if self.base_url.as_deref().map_or(true, str::is_empty) {
            issues.push(
                "base_url is empty and no default is available for this provider".into(),
            );
        }

        if self
            .default_model
            .as_deref()
            .map_or(true, str::is_empty)
        {
            issues.push("default_model is empty".into());
        }

        issues
    }

    /// Returns `Ok(())` if [`validate`](Self::validate) yields no
    /// issues, `Err` with joined messages otherwise.
    pub fn validate_or_error(&self) -> Result<(), LlmError> {
        let issues = self.validate();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(LlmError::Config(issues.join("; ")))
        }
    }
}

// ─── Provider trait ──────────────────────────────────────────

// ─── HealthStatus ────────────────────────────────────────────

/// Result of a provider health check.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Whether the provider endpoint was reachable and responded
    /// without error.
    pub reachable: bool,
    /// Round-trip latency in milliseconds.
    pub latency_ms: u64,
    /// Number of available models (if the provider supports listing).
    pub model_count: Option<usize>,
    /// Error message if the check failed.
    pub error: Option<String>,
}

// ─── Provider trait ──────────────────────────────────────────

/// A [`Provider`] is an object that can create [`LlmClient`] instances.
///
/// The trait is separate from [`LlmClient`] so that provider metadata
/// (type, config) can be queried without creating clients.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns the provider type identifier.
    fn provider_type(&self) -> ProviderType;

    /// Returns the configuration used to create this provider.
    fn config(&self) -> &ProviderConfig;

    /// Creates a new [`LlmClient`] from this provider's configuration.
    ///
    /// Each call may create a fresh client (e.g. with a new HTTP
    /// connection pool). Implementations that are themselves
    /// `LlmClient`s may return `Box::new(self.clone())`.
    async fn create_client(&self) -> Result<Box<dyn LlmClient>, LlmError>;

    /// Convenience: create a client and wrap in `Arc`.
    async fn create_client_arc(&self) -> Result<Arc<dyn LlmClient>, LlmError> {
        self.create_client().await.map(|c| c.into())
    }

    /// Verify the provider is reachable and responsive.
    ///
    /// Default implementation calls `list_models()` on a freshly
    /// created client. Override for providers that don't support
    /// model listing (e.g. send a minimal chat request instead).
    async fn health_check(&self) -> HealthStatus {
        match self.create_client().await {
            Ok(client) => {
                let start = std::time::Instant::now();
                match client.list_models().await {
                    Ok(models) => HealthStatus {
                        reachable: true,
                        latency_ms: start.elapsed().as_millis() as u64,
                        model_count: Some(models.len()),
                        error: None,
                    },
                    Err(e) => HealthStatus {
                        reachable: false,
                        latency_ms: start.elapsed().as_millis() as u64,
                        model_count: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => HealthStatus {
                reachable: false,
                latency_ms: 0,
                model_count: None,
                error: Some(e.to_string()),
            },
        }
    }
}

// ─── ProviderFactory ─────────────────────────────────────────

/// Construct a [`Provider`] from a [`ProviderConfig`].
///
/// This is the main entry point for creating providers without
/// knowing the concrete type at compile time.
///
/// Currently supported:
/// - `OpenAI` — natively supported.
/// - `Ollama` — treated as OpenAI-compatible with a local base URL.
/// - `Custom(_)` — treated as OpenAI-compatible (HTTP API must follow
///   the OpenAI `/v1/chat/completions` shape).
/// - `Anthropic` / `Google` — not yet implemented; will error.
pub fn create_provider(config: ProviderConfig) -> Result<Box<dyn Provider>, LlmError> {
    match &config.provider_type {
        ProviderType::Anthropic => Err(LlmError::Config(
            "Anthropic provider is not yet implemented".into(),
        )),
        ProviderType::Google => Err(LlmError::Config(
            "Google provider is not yet implemented".into(),
        )),
        _ => {
            // OpenAI / Ollama / Custom all use the OpenAI-compatible client.
            let client = super::openai::OpenAiClient::from_config(&config.with_defaults())?;
            Ok(Box::new(client))
        }
    }
}

/// Create a provider from environment variables (OpenAI-compatible only).
///
/// See [`ProviderConfig::from_env`] for supported variables.
pub fn create_provider_from_env() -> Result<Box<dyn Provider>, LlmError> {
    let config = ProviderConfig::from_env().with_defaults();
    create_provider(config)
}

// ─── ProviderRegistry ────────────────────────────────────────

/// Manages multiple named [`Provider`]s, enabling routing by purpose
/// (e.g. `"default"` vs `"embedding"` vs `"reasoning"`).
///
/// # Example
///
/// ```ignore
/// let mut registry = ProviderRegistry::new();
/// registry.register("default", create_provider(config)?);
/// let client = registry.create_default_client().await?;
/// ```
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
    default_name: String,
}

impl ProviderRegistry {
    /// Create an empty registry with `"default"` as the default
    /// provider name.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            default_name: "default".into(),
        }
    }

    /// Register a provider under the given name.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        provider: Box<dyn Provider>,
    ) {
        self.providers.insert(name.into(), provider);
    }

    /// Change which provider is considered the default.
    pub fn set_default(&mut self, name: impl Into<String>) {
        self.default_name = name.into();
    }

    /// Look up a provider by name.
    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// Return the default provider.
    pub fn default(&self) -> Option<&dyn Provider> {
        self.get(&self.default_name)
    }

    /// The name of the default provider.
    pub fn default_name(&self) -> &str {
        &self.default_name
    }

    /// Create a client from the named provider.
    pub async fn create_client(
        &self,
        name: &str,
    ) -> Result<Box<dyn LlmClient>, LlmError> {
        match self.providers.get(name) {
            Some(p) => p.create_client().await,
            None => Err(LlmError::Config(format!(
                "provider '{}' not found",
                name
            ))),
        }
    }

    /// Create a client from the default provider.
    pub async fn create_default_client(
        &self,
    ) -> Result<Box<dyn LlmClient>, LlmError> {
        self.create_client(&self.default_name.clone()).await
    }

    /// Health-check all registered providers, returning a map from
    /// provider name to health status.
    pub async fn health_check_all(&self) -> HashMap<String, HealthStatus> {
        let mut results = HashMap::new();
        for (name, provider) in &self.providers {
            results.insert(name.clone(), provider.health_check().await);
        }
        results
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
