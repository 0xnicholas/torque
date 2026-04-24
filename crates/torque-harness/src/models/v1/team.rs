use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct TeamDefinition {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub supervisor_agent_definition_id: Uuid,
    pub sub_agents: serde_json::Value,
    pub policy: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamDefinitionCreate {
    pub name: String,
    pub description: Option<String>,
    pub supervisor_agent_definition_id: Uuid,
    #[serde(default)]
    pub sub_agents: Vec<serde_json::Value>,
    #[serde(default)]
    pub policy: serde_json::Value,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TeamInstance {
    pub id: Uuid,
    pub team_definition_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamInstanceCreate {
    pub team_definition_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct TeamTaskCreate {
    pub goal: String,
    pub instructions: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub input_artifacts: Vec<Uuid>,
    #[serde(default)]
    pub parent_task_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TeamMember {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub agent_instance_id: Uuid,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamMemberCreate {
    pub team_instance_id: Uuid,
    pub agent_instance_id: Uuid,
    #[serde(default = "default_member_role")]
    pub role: String,
}

fn default_member_role() -> String {
    "member".to_string()
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TeamTask {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub goal: String,
    pub instructions: Option<String>,
    pub status: TeamTaskStatus,
    pub triage_result: Option<TriageResult>,
    pub mode_selected: Option<String>,
    pub input_artifacts: Vec<Uuid>,
    pub parent_task_id: Option<Uuid>,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub retry_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamTaskStatus {
    Open,
    Triaged,
    InProgress,
    WaitingMembers,
    ResultsReceived,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TeamTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamTaskStatus::Open => write!(f, "OPEN"),
            TeamTaskStatus::Triaged => write!(f, "TRIAGED"),
            TeamTaskStatus::InProgress => write!(f, "IN_PROGRESS"),
            TeamTaskStatus::WaitingMembers => write!(f, "WAITING_MEMBERS"),
            TeamTaskStatus::ResultsReceived => write!(f, "RESULTS_RECEIVED"),
            TeamTaskStatus::Blocked => write!(f, "BLOCKED"),
            TeamTaskStatus::Completed => write!(f, "COMPLETED"),
            TeamTaskStatus::Failed => write!(f, "FAILED"),
            TeamTaskStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

impl TryFrom<&str> for TeamTaskStatus {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "OPEN" => Ok(TeamTaskStatus::Open),
            "TRIAGED" => Ok(TeamTaskStatus::Triaged),
            "IN_PROGRESS" => Ok(TeamTaskStatus::InProgress),
            "WAITING_MEMBERS" => Ok(TeamTaskStatus::WaitingMembers),
            "RESULTS_RECEIVED" => Ok(TeamTaskStatus::ResultsReceived),
            "BLOCKED" => Ok(TeamTaskStatus::Blocked),
            "COMPLETED" => Ok(TeamTaskStatus::Completed),
            "FAILED" => Ok(TeamTaskStatus::Failed),
            "CANCELLED" => Ok(TeamTaskStatus::Cancelled),
            _ => Err(format!("Unknown task status: {}", s)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TriageResult {
    pub complexity: TaskComplexity,
    pub processing_path: ProcessingPath,
    pub selected_mode: TeamMode,
    pub lead_member_ref: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskComplexity {
    Simple,
    Medium,
    Complex,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessingPath {
    SingleRoute,
    GuidedDelegate,
    StructuredOrchestration,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamMode {
    Route,
    Broadcast,
    Coordinate,
    Tasks,
}

impl std::fmt::Display for TeamMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamMode::Route => write!(f, "ROUTE"),
            TeamMode::Broadcast => write!(f, "BROADCAST"),
            TeamMode::Coordinate => write!(f, "COORDINATE"),
            TeamMode::Tasks => write!(f, "TASKS"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SharedTaskState {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub accepted_artifact_refs: Vec<ArtifactRef>,
    pub published_facts: Vec<PublishedFact>,
    pub delegation_status: Vec<DelegationStatusEntry>,
    pub open_blockers: Vec<Blocker>,
    pub decisions: Vec<Decision>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ArtifactRef {
    pub artifact_id: Uuid,
    pub scope: PublishScope,
    pub published_by: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublishScope {
    Private,
    TeamShared,
    ExternalPublished,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublishedFact {
    pub key: String,
    pub value: serde_json::Value,
    pub published_by: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DelegationStatusEntry {
    pub delegation_id: Uuid,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Blocker {
    pub blocker_id: Uuid,
    pub description: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Decision {
    pub decision_id: Uuid,
    pub description: String,
    pub decided_by: String,
    pub decided_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TeamEvent {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub actor_ref: String,
    pub team_task_ref: Option<Uuid>,
    pub related_instance_refs: Vec<Uuid>,
    pub related_artifact_refs: Vec<Uuid>,
    pub payload: serde_json::Value,
    pub causal_event_refs: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamEventType {
    TeamTaskReceived,
    TriageCompleted,
    ModeSelected,
    LeadAssigned,
    MemberActivated,
    DelegationCreated,
    DelegationAccepted,
    DelegationRejected,
    MemberResultReceived,
    MemberResultAccepted,
    MemberResultRejected,
    ArtifactPublished,
    FactPublished,
    BlockerAdded,
    BlockerResolved,
    ApprovalRequested,
    TeamBlocked,
    TeamUnblocked,
    TeamCompleted,
    TeamFailed,
}

impl std::fmt::Display for TeamEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamEventType::TeamTaskReceived => write!(f, "TEAM_TASK_RECEIVED"),
            TeamEventType::TriageCompleted => write!(f, "TRIAGE_COMPLETED"),
            TeamEventType::ModeSelected => write!(f, "MODE_SELECTED"),
            TeamEventType::LeadAssigned => write!(f, "LEAD_ASSIGNED"),
            TeamEventType::MemberActivated => write!(f, "MEMBER_ACTIVATED"),
            TeamEventType::DelegationCreated => write!(f, "DELEGATION_CREATED"),
            TeamEventType::DelegationAccepted => write!(f, "DELEGATION_ACCEPTED"),
            TeamEventType::DelegationRejected => write!(f, "DELEGATION_REJECTED"),
            TeamEventType::MemberResultReceived => write!(f, "MEMBER_RESULT_RECEIVED"),
            TeamEventType::MemberResultAccepted => write!(f, "MEMBER_RESULT_ACCEPTED"),
            TeamEventType::MemberResultRejected => write!(f, "MEMBER_RESULT_REJECTED"),
            TeamEventType::ArtifactPublished => write!(f, "ARTIFACT_PUBLISHED"),
            TeamEventType::FactPublished => write!(f, "FACT_PUBLISHED"),
            TeamEventType::BlockerAdded => write!(f, "BLOCKER_ADDED"),
            TeamEventType::BlockerResolved => write!(f, "BLOCKER_RESOLVED"),
            TeamEventType::ApprovalRequested => write!(f, "APPROVAL_REQUESTED"),
            TeamEventType::TeamBlocked => write!(f, "TEAM_BLOCKED"),
            TeamEventType::TeamUnblocked => write!(f, "TEAM_UNBLOCKED"),
            TeamEventType::TeamCompleted => write!(f, "TEAM_COMPLETED"),
            TeamEventType::TeamFailed => write!(f, "TEAM_FAILED"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemberSelector {
    pub selector_type: SelectorType,
    pub capability_profiles: Vec<String>,
    pub role: Option<String>,
    pub agent_definition_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SelectorType {
    Capability,
    Role,
    Direct,
    Any,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CandidateMember {
    pub team_member_id: Uuid,
    pub agent_instance_id: Uuid,
    pub agent_definition_id: Uuid,
    pub role: String,
    pub capability_profiles: Vec<String>,
    pub selection_rationale: String,
    pub policy_check_summary: PolicyCheckSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolicyCheckSummary {
    pub resource_available: bool,
    pub approval_required: bool,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeamRecoveryDisposition {
    TeamHealthy,
    TeamDegraded,
    TeamFailed,
    AwaitingSupervisor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRecoveryAssessment {
    pub team_instance_id: Uuid,
    pub disposition: TeamRecoveryDisposition,
    pub failed_member_ids: Vec<Uuid>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeamRecoveryAction {
    Retry,
    EscalateToSupervisor,
    NoOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTaskRecoveryResult {
    pub task_id: Uuid,
    pub action_taken: TeamRecoveryAction,
    pub new_status: TeamTaskStatus,
}
