use torque_harness::models::v1::delegation::DelegationStatus;
use torque_harness::models::v1::delegation_event::RejectionReason;
use torque_harness::service::team::circuit_breaker::{CircuitBreaker, CircuitState};
use torque_harness::service::team::retry::{classify_rejection, RetryBudget, RetryDecision};

#[tokio::test]
async fn test_circuit_breaker_initial_closed() {
    let cb = CircuitBreaker::new(5, 3);
    assert_eq!(cb.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_opens_after_threshold() {
    let cb = CircuitBreaker::new(3, 3);
    for _ in 0..3 {
        cb.record_failure(&RejectionReason::CapacityFull).await;
    }
    assert_eq!(cb.state().await, CircuitState::Open);
}

#[tokio::test]
async fn test_circuit_breaker_allows_request_when_closed() {
    let cb = CircuitBreaker::new(5, 3);
    assert!(cb.allow_request().await);
}

#[tokio::test]
async fn test_retry_budget_initialization() {
    let budget = RetryBudget::new(3);
    assert!(budget.can_retry());
    assert!(!budget.is_exhausted());
    assert_eq!(budget.remaining, 3);
    assert_eq!(budget.spent, 0);
}

#[tokio::test]
async fn test_retry_budget_consume() {
    let mut budget = RetryBudget::new(3);
    assert!(budget.consume(1));
    assert_eq!(budget.remaining, 2);
    assert_eq!(budget.spent, 1);
}

#[tokio::test]
async fn test_retry_budget_exhausted() {
    let mut budget = RetryBudget::new(2);
    budget.consume(1);
    budget.consume(1);
    assert!(!budget.can_retry());
    assert!(budget.is_exhausted());
}

#[tokio::test]
async fn test_classify_rejection_capacity_full() {
    let mut budget = RetryBudget::new(3);
    let reason = RejectionReason::CapacityFull;
    let decision = classify_rejection(&reason, &budget);
    assert!(matches!(decision, RetryDecision::RetryWithOtherMember));
}

#[tokio::test]
async fn test_classify_rejection_timeout() {
    let mut budget = RetryBudget::new(3);
    let reason = RejectionReason::Timeout;
    let decision = classify_rejection(&reason, &budget);
    assert!(matches!(decision, RetryDecision::RetryWithSameMember));
}

#[tokio::test]
async fn test_classify_rejection_capability_mismatch() {
    let budget = RetryBudget::new(3);
    let reason = RejectionReason::CapabilityMismatch;
    let decision = classify_rejection(&reason, &budget);
    assert!(matches!(decision, RetryDecision::DoNotRetry { .. }));
}

#[tokio::test]
async fn test_delegation_status_is_terminal() {
    assert!(DelegationStatus::Completed.is_terminal());
    assert!(DelegationStatus::Failed.is_terminal());
    assert!(DelegationStatus::TimeoutPartial.is_terminal());
    assert!(!DelegationStatus::Pending.is_terminal());
    assert!(!DelegationStatus::Accepted.is_terminal());
}
