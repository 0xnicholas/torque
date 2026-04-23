use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct RecoveryResult {
    pub instance_id: Uuid,
    pub checkpoint_id: Uuid,
    pub restored_status: String,
    pub assessment: RecoveryAssessmentSummary,
    pub recommended_action: String,
}

#[derive(Debug, Serialize)]
pub struct RecoveryAssessmentSummary {
    pub disposition: String,
    pub requires_replay: bool,
    pub terminal: bool,
}