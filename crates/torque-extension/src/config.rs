use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Full configuration for an Extension instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    pub settings: serde_json::Value,
    pub tools: HashMap<String, ToolConfig>,
    pub model: Option<ModelConfig>,
}

impl Default for ExtensionConfig {
    fn default() -> Self {
        Self {
            settings: serde_json::Value::Object(Default::default()),
            tools: HashMap::new(),
            model: None,
        }
    }
}

/// Partial configuration patch for runtime updates (Layer 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfigPatch {
    pub settings: Option<serde_json::Value>,
    pub tools: Option<HashMap<String, ToolConfig>>,
    pub model: Option<ModelConfig>,
}

/// Configuration for a tool exposed by an Extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub timeout_ms: Option<u64>,
    pub retries: Option<u32>,
    pub enabled: Option<bool>,
}

/// Configuration for the model used by an Extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub parameters: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_config_default() {
        let config = ExtensionConfig::default();
        assert_eq!(config.settings, serde_json::Value::Object(Default::default()));
        assert!(config.tools.is_empty());
        assert!(config.model.is_none());
    }

    #[test]
    fn test_extension_config_new() {
        let config = ExtensionConfig {
            settings: serde_json::json!({"key": "value"}),
            tools: HashMap::new(),
            model: None,
        };
        assert_eq!(config.settings["key"], "value");
        assert!(config.tools.is_empty());
    }

    #[test]
    fn test_extension_config_patch_default() {
        let patch = ExtensionConfigPatch {
            settings: None,
            tools: None,
            model: None,
        };
        assert!(patch.settings.is_none());
        assert!(patch.tools.is_none());
        assert!(patch.model.is_none());
    }

    #[test]
    fn test_tool_config() {
        let tool = ToolConfig {
            timeout_ms: Some(5000),
            retries: Some(3),
            enabled: Some(true),
        };
        assert_eq!(tool.timeout_ms, Some(5000));
        assert!(tool.enabled.unwrap());
    }

    #[test]
    fn test_model_config() {
        let model = ModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            parameters: serde_json::json!({"temperature": 0.7}),
        };
        assert_eq!(model.provider, "openai");
        assert_eq!(model.model, "gpt-4");
    }
}
