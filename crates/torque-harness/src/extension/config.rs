use std::collections::HashMap;

/// Configuration for the Harness-level Extension system.
///
/// Controls which Extensions are loaded at startup and how the
/// Extension Runtime behaves.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HarnessExtensionConfig {
    /// If `false`, the Extension system is disabled entirely
    /// (no runtime spawned, no built-ins loaded).
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Built-in extensions to load at startup.
    ///
    /// Supported values: `"logging"`, `"metrics"`.
    #[serde(default)]
    pub builtins: Vec<String>,

    /// Custom extensions to register at startup.
    ///
    /// The key is a human-readable label (e.g. `"my-ext"`), the value is
    /// a JSON object passed as the extension's configuration.
    #[serde(default)]
    pub custom_extensions: HashMap<String, serde_json::Value>,

    /// Optional: timeout in seconds for hook execution (default: 30).
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_secs: u64,
}

fn default_enabled() -> bool {
    true
}

fn default_hook_timeout() -> u64 {
    30
}

impl Default for HarnessExtensionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            builtins: Vec::new(),
            custom_extensions: HashMap::new(),
            hook_timeout_secs: 30,
        }
    }
}
