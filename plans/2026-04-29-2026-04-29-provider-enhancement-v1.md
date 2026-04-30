# Provider Enhancement Plan

## Objective

Fix data integrity bugs, add health_check/validation, unify orphaned subsystems (embedding, candidate generation, summarization) through the Provider system, and add multi-provider registry.

---

## Phase 1 — Core fixes (llm crate)

### 1.1 Refactor `OpenAiClient` constructors (`crates/llm/src/openai.rs`)

**Problem:** `from_config` calls `new()`, which re-creates `ProviderConfig` from raw params, losing `extra` fields. `new()` auto-detects provider type, overriding what the caller specified.

**Fix:** Add `new_with_config(ProviderConfig) -> Self` as canonical constructor. `new()` becomes a backward-compat wrapper that auto-detects provider type and delegates to `new_with_config`.

```rust
impl OpenAiClient {
    /// Backward-compat: auto-detect provider type from URL.
    pub fn new(base_url: String, api_key: String, default_model: String) -> Self {
        let provider_type = if base_url.contains("localhost:11434") {
            ProviderType::Ollama
        } else if base_url.contains("api.openai.com") {
            ProviderType::OpenAI
        } else {
            ProviderType::Custom("openai-compatible".into())
        };
        let config = ProviderConfig {
            provider_type,
            base_url: Some(base_url),
            api_key: Some(api_key),
            default_model: Some(default_model),
            extra: HashMap::new(),
        };
        Self::new_with_config(config)
    }

    /// Canonical constructor: preserves full ProviderConfig including `extra`.
    pub fn new_with_config(config: ProviderConfig) -> Self {
        let cfg = config.with_defaults();
        let base_url = cfg.base_url.as_deref().unwrap_or("https://api.openai.com/v1").to_string();
        let api_key = cfg.api_key.as_deref().unwrap_or("").to_string();
        let default_model = cfg.default_model.as_deref().unwrap_or("gpt-4o-mini").to_string();
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { http_client, base_url, api_key, default_model, provider_config: cfg }
    }

    pub fn from_config(config: &ProviderConfig) -> Result<Self> {
        Ok(Self::new_with_config(config.clone()))
    }

    pub fn from_env() -> Result<Self> {
        let config = ProviderConfig::from_env();
        Ok(Self::new_with_config(config))
    }
}
```

Also fix the `impl Provider for OpenAiClient::create_client()` method — currently it clones fields manually, which skips `provider_config`. Change to call `new_with_config(self.provider_config.clone())`.

---

### 1.2 Add `ProviderConfig::validate()` (`crates/llm/src/provider.rs`)

```rust
impl ProviderConfig {
    /// Validate the config, returning a list of issues.
    /// A config that requires authentication but has no api_key is invalid.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        match self.provider_type {
            ProviderType::OpenAI | ProviderType::Anthropic | ProviderType::Google => {
                if self.api_key.as_deref().map_or(true, str::is_empty) {
                    issues.push("api_key is required for this provider".into());
                }
            }
            ProviderType::Ollama | ProviderType::Custom(_) => {
                // api_key is optional for these
            }
        }
        if self.base_url.as_deref().map_or(true, str::is_empty) {
            issues.push("base_url is empty and no default is available".into());
        }
        if self.default_model.as_deref().map_or(true, str::is_empty) {
            issues.push("default_model is empty".into());
        }
        issues
    }

    /// Returns Ok if validate() yields no issues, Err with joined messages otherwise.
    pub fn validate_or_error(&self) -> Result<()> {
        let issues = self.validate();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(LlmError::Config(issues.join("; ")))
        }
    }
}
```

---

### 1.3 Add `health_check()` to `Provider` trait (`crates/llm/src/provider.rs`)

```rust
/// Result of a provider health check.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub reachable: bool,
    pub latency_ms: u64,
    pub model_count: Option<usize>,
    pub error: Option<String>,
}

#[async_trait]
pub trait Provider: Send + Sync {
    // ... existing methods ...

    /// Verify the provider is reachable and responsive.
    /// Default implementation lists models to check connectivity.
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
```

Note: This requires that `LlmClient` has a `list_models()` method (the current `OpenAiClient` already does — `pub async fn list_models(&self)` — check if it's on the trait). It is NOT currently on the trait. Add it:

In `crates/llm/src/client.rs`, add to the `LlmClient` trait:

```rust
/// List available models from this provider.
async fn list_models(&self) -> Result<Vec<String>>;
```

---

## Phase 2 — Unify Embedding (`crates/torque-harness/src/embedding.rs`)

**Problem:** `OpenAIEmbeddingGenerator` constructs its own `reqwest::Client`, reads `OPENAI_API_KEY` / `OPENAI_BASE_URL` env vars directly, completely bypassing the Provider system.

**Fix:** Add a `from_provider_config` constructor that accepts `Arc<ProviderConfig>` or use the existing `Arc<dyn LlmClient>`:

```rust
impl OpenAIEmbeddingGenerator {
    /// Create from a ProviderConfig (reuses base_url + api_key).
    pub fn from_provider_config(config: &ProviderConfig, model: String) -> Self {
        let cfg = config.clone().with_defaults();
        let base_url = cfg.base_url.unwrap_or_else(|| "https://api.openai.com/v1".into());
        let api_key = cfg.api_key.unwrap_or_default();
        Self::new(base_url, api_key, model)
    }
}
```

Then in `app.rs`, pass the provider config to embedding init.

---

## Phase 3 — Eliminate raw HTTP clients

### 3.1 `CandidateGenerator` (`crates/torque-harness/src/service/candidate_generator.rs`)

**Problem:** Creates its own `reqwest::Client`, constructs headers manually, and calls the chat endpoint directly instead of using `LlmClient`.

**Fix:** Accept `Arc<dyn LlmClient>` in the constructor and use `client.chat(request).await` instead of raw HTTP.

### 3.2 `SummarizeStrategy` (`crates/torque-harness/src/service/merge_strategy.rs`)

**Problem:** Same pattern — custom `reqwest::Client` with raw HTTP call.

**Fix:** Same approach — accept `Arc<dyn LlmClient>` and call `client.chat(request).await`.

---

## Phase 4 — ProviderRegistry (`crates/llm/src/provider.rs`)

```rust
/// Manages multiple named providers, enabling routing by purpose
/// (e.g. "default" vs "embedding" vs "reasoning").
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
    default_name: String,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new(), default_name: "default".into() }
    }

    pub fn register(&mut self, name: impl Into<String>, provider: Box<dyn Provider>) {
        self.providers.insert(name.into(), provider);
    }

    pub fn set_default(&mut self, name: impl Into<String>) {
        self.default_name = name.into();
    }

    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    pub fn default(&self) -> Option<&dyn Provider> {
        self.providers.get(&self.default_name).map(|p| p.as_ref())
    }

    pub fn default_name(&self) -> &str {
        &self.default_name
    }

    /// Create a client from the named provider.
    pub async fn create_client(&self, name: &str) -> Result<Box<dyn LlmClient>> {
        match self.providers.get(name) {
            Some(p) => p.create_client().await,
            None => Err(LlmError::Config(format!("provider '{}' not found", name))),
        }
    }

    /// Create a client from the default provider.
    pub async fn create_default_client(&self) -> Result<Box<dyn LlmClient>> {
        self.create_client(&self.default_name.clone()).await
    }

    /// Health-check all registered providers.
    pub async fn health_check_all(&self) -> HashMap<String, HealthStatus> {
        let mut results = HashMap::new();
        for (name, provider) in &self.providers {
            results.insert(name.clone(), provider.health_check().await);
        }
        results
    }
}
```

Export `ProviderRegistry` and `HealthStatus` from `crates/llm/src/lib.rs`.

---

## Phase 5 — Wire into harness

### 5.1 `crates/torque-harness/src/main.rs` — Use ProviderRegistry

```rust
let mut registry = llm::ProviderRegistry::new();
let default_config = llm::ProviderConfig::from_env().with_defaults();
let default_provider = llm::create_provider(default_config)?;
registry.register("default", default_provider);
// Optionally register additional providers from env vars:
// LLM_PROVIDER_EMBEDDING, LLM_PROVIDER_REASONING, etc.

let health = registry.health_check_all().await;
for (name, status) in &health {
    if !status.reachable {
        tracing::warn!("Provider '{}' health check failed: {:?}", name, status.error);
    } else {
        tracing::info!("Provider '{}' OK ({}ms)", name, status.latency_ms);
    }
}

let llm = registry.create_default_client().await?;
```

### 5.2 `crates/torque-harness/src/infra/llm.rs` — Re-export new types

Add to re-exports: `ProviderRegistry`, `HealthStatus`

---

## Implementation Order

- [ ] 1. Add `list_models()` to `LlmClient` trait in `crates/llm/src/client.rs`
- [ ] 2. Add `HealthStatus` struct and `health_check()` default method to `Provider` trait
- [ ] 3. Add `ProviderConfig::validate()` and `validate_or_error()`
- [ ] 4. Refactor `OpenAiClient` constructors: add `new_with_config`, rewrite `new`/`from_config`/`from_env`
- [ ] 5. Fix `impl Provider for OpenAiClient::create_client()` to use `new_with_config`
- [ ] 6. Add `ProviderRegistry` to `crates/llm/src/provider.rs`
- [ ] 7. Update `crates/llm/src/lib.rs` exports: `HealthStatus`, `ProviderRegistry`, `list_models`
- [ ] 8. Build and verify `llm` crate compiles
- [ ] 9. Add `list_models()` to `OpenAiClient` impl (if not already using trait default), and add `list_models()` to `infra/llm.rs` re-exports
- [ ] 10. `OpenAIEmbeddingGenerator::from_provider_config()` in `embedding.rs`
- [ ] 11. Wire `from_provider_config` in `app.rs` for embedding init
- [ ] 12. `CandidateGenerator` refactor: `Arc<dyn LlmClient>` instead of raw HTTP
- [ ] 13. `SummarizeStrategy` refactor: `Arc<dyn LlmClient>` instead of raw HTTP
- [ ] 14. Wire ProviderRegistry in `main.rs` with health check on startup
- [ ] 15. Build and verify all crates compile

## Verification Criteria

- `ProviderConfig.extra` fields survive round-trip through `from_config` → `create_client`
- `health_check()` returns `reachable: true` for a valid OpenAI/Ollama endpoint
- `health_check()` returns `reachable: false` with error message for an unreachable endpoint
- `ProviderRegistry` can register 2+ providers and route to each independently
- `OpenAIEmbeddingGenerator` works when initialized via `from_provider_config`
- `CandidateGenerator` and `SummarizeStrategy` use `LlmClient` instead of raw HTTP
- All crates compile with 0 errors

## Risks and Mitigations

1. **`list_models()` not on `LlmClient` trait** — currently only on `OpenAiClient`. Mitigation: add it to the trait with a default that returns unimplemented, implement properly in `OpenAiClient`.
2. **`CandidateGenerator`/`SummarizeStrategy` may call non-OpenAI-compatible models** — their prompts assume OpenAI-style chat format. Mitigation: after switching to `LlmClient`, the same chat format works for any OpenAI-compatible provider.
3. **Embedding model name differs per provider** — `text-embedding-3-small` is OpenAI-specific. Mitigation: make embedding model configurable via `ProviderConfig.extra`.
