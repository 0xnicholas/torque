use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentType {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub max_tokens: i32,
    pub timeout_secs: i32,
    pub created_at: DateTime<Utc>,
}

impl AgentType {
    pub fn new(name: String, system_prompt: String, tools: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description: None,
            system_prompt,
            tools,
            max_tokens: 4096,
            timeout_secs: 300,
            created_at: Utc::now(),
        }
    }
}
