use torque_harness::models::v1::delegation::DelegationStatus;

#[test]
fn test_delegation_status_constants() {
    assert!(DelegationStatus::Pending.to_string() == "PENDING");
    assert!(DelegationStatus::Completed.to_string() == "COMPLETED");
}

#[test]
fn test_delegation_status_is_terminal() {
    assert!(DelegationStatus::Completed.is_terminal());
    assert!(DelegationStatus::Failed.is_terminal());
    assert!(DelegationStatus::TimeoutPartial.is_terminal());
    assert!(!DelegationStatus::Pending.is_terminal());
    assert!(!DelegationStatus::Accepted.is_terminal());
    assert!(!DelegationStatus::Rejected.is_terminal());
}