use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialQuality {
    pub completeness: f32,
    pub correctness_confidence: f32,
    pub usable_as_is: bool,
    pub requires_repair: Vec<String>,
    pub estimated_remaining_work: Option<String>,
}