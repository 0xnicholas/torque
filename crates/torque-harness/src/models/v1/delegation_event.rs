use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use super::partial_quality::PartialQuality;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DelegationEvent {
    Created {
        delegation_id: Uuid,
        task_id: Uuid,
        member_id: Uuid,
        created_at: DateTime<Utc>,
    },
    Accepted {
        delegation_id: Uuid,
        member_id: Uuid,
        accepted_at: DateTime<Utc>,
    },
    Rejected {
        delegation_id: Uuid,
        member_id: Uuid,
        reason: RejectionReason,
        rejected_at: DateTime<Utc>,
    },
    Completed {
        delegation_id: Uuid,
        member_id: Uuid,
        artifact_id: Uuid,
        completed_at: DateTime<Utc>,
    },
    Failed {
        delegation_id: Uuid,
        member_id: Uuid,
        error: String,
        failed_at: DateTime<Utc>,
    },
    TimeoutPartial {
        delegation_id: Uuid,
        member_id: Uuid,
        partial_quality: PartialQuality,
        timed_out_at: DateTime<Utc>,
    },
    ExtensionRequested {
        delegation_id: Uuid,
        member_id: Uuid,
        requested_seconds: u32,
        reason: String,
        requested_at: DateTime<Utc>,
    },
    ExtensionGranted {
        delegation_id: Uuid,
        granted_seconds: u32,
        new_deadline: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RejectionReason {
    CapacityFull,
    CapabilityMismatch,
    PolicyViolation,
    MemberUnavailable,
    Timeout,
    Other(String),
}

impl std::fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectionReason::CapacityFull => write!(f, "CAPACITY_FULL"),
            RejectionReason::CapabilityMismatch => write!(f, "CAPABILITY_MISMATCH"),
            RejectionReason::PolicyViolation => write!(f, "POLICY_VIOLATION"),
            RejectionReason::MemberUnavailable => write!(f, "MEMBER_UNAVAILABLE"),
            RejectionReason::Timeout => write!(f, "TIMEOUT"),
            RejectionReason::Other(s) => write!(f, "OTHER: {}", s),
        }
    }
}