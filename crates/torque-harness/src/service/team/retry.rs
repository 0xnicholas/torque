use super::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::models::v1::delegation_event::RejectionReason;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MemberHealth {
    pub member_id: Uuid,
    pub circuit_state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub is_healthy: bool,
    pub last_seen: DateTime<Utc>,
}

pub struct MemberHealthTracker {
    members: RwLock<HashMap<Uuid, Arc<CircuitBreaker>>>,
}

impl MemberHealthTracker {
    pub fn new() -> Self {
        Self {
            members: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_or_create(&self, member_id: Uuid) -> Arc<CircuitBreaker> {
        let mut members = self.members.write().await;
        if let Some(cb) = members.get(&member_id) {
            return cb.clone();
        }
        let cb = Arc::new(CircuitBreaker::new(5, 3));
        members.insert(member_id, cb.clone());
        cb
    }

    pub async fn record_failure(&self, member_id: Uuid, reason: RejectionReason) {
        let cb = self.get_or_create(member_id).await;
        cb.record_failure(&reason).await;
    }

    pub async fn record_success(&self, member_id: Uuid) {
        let cb = self.get_or_create(member_id).await;
        cb.record_success().await;
    }

    pub async fn is_healthy(&self, member_id: Uuid) -> bool {
        let cb = self.get_or_create(member_id).await;
        cb.allow_request().await
    }

    pub async fn get_health(&self, member_id: Uuid) -> Option<MemberHealth> {
        let cb = self.get_or_create(member_id).await;
        Some(MemberHealth {
            member_id,
            circuit_state: cb.state().await,
            failure_count: 0,
            success_count: 0,
            is_healthy: cb.allow_request().await,
            last_seen: Utc::now(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RetryBudget {
    pub total: usize,
    pub remaining: usize,
    pub spent: usize,
}

impl RetryBudget {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            remaining: total,
            spent: 0,
        }
    }

    pub fn consume(&mut self, amount: usize) -> bool {
        if self.remaining >= amount {
            self.remaining -= amount;
            self.spent += amount;
            true
        } else {
            false
        }
    }

    pub fn can_retry(&self) -> bool {
        self.remaining > 0
    }

    pub fn is_exhausted(&self) -> bool {
        self.remaining == 0
    }
}

pub enum RetryDecision {
    RetryWithSameMember,
    RetryWithOtherMember,
    DoNotRetry { reason: String },
}

pub fn classify_rejection(reason: &RejectionReason, budget: &RetryBudget) -> RetryDecision {
    match reason {
        RejectionReason::CapacityFull => {
            if budget.can_retry() {
                RetryDecision::RetryWithOtherMember
            } else {
                RetryDecision::DoNotRetry {
                    reason: "Budget exhausted".to_string(),
                }
            }
        }
        RejectionReason::Timeout => {
            if budget.can_retry() {
                RetryDecision::RetryWithSameMember
            } else {
                RetryDecision::DoNotRetry {
                    reason: "Budget exhausted".to_string(),
                }
            }
        }
        RejectionReason::CapabilityMismatch => RetryDecision::DoNotRetry {
            reason: "Capability mismatch".to_string(),
        },
        RejectionReason::PolicyViolation => RetryDecision::DoNotRetry {
            reason: "Policy violation".to_string(),
        },
        RejectionReason::MemberUnavailable => {
            if budget.can_retry() {
                RetryDecision::RetryWithOtherMember
            } else {
                RetryDecision::DoNotRetry {
                    reason: "Budget exhausted".to_string(),
                }
            }
        }
        RejectionReason::Other(_) => RetryDecision::DoNotRetry {
            reason: "Unknown error".to_string(),
        },
    }
}
