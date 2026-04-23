use torque_harness::models::v1::delegation::*;

#[test]
fn test_delegation_status_display() {
    assert_eq!(DelegationStatus::Pending.to_string(), "PENDING");
    assert_eq!(DelegationStatus::Accepted.to_string(), "ACCEPTED");
    assert_eq!(DelegationStatus::Rejected.to_string(), "REJECTED");
    assert_eq!(DelegationStatus::Completed.to_string(), "COMPLETED");
    assert_eq!(DelegationStatus::Failed.to_string(), "FAILED");
    assert_eq!(
        DelegationStatus::TimeoutPartial.to_string(),
        "TIMEOUT_PARTIAL"
    );
}

#[test]
fn test_delegation_status_try_from_str() {
    assert_eq!(
        DelegationStatus::try_from("PENDING").unwrap(),
        DelegationStatus::Pending
    );
    assert_eq!(
        DelegationStatus::try_from("COMPLETED").unwrap(),
        DelegationStatus::Completed
    );
    assert_eq!(
        DelegationStatus::try_from("FAILED").unwrap(),
        DelegationStatus::Failed
    );
    assert!(DelegationStatus::try_from("INVALID").is_err());
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
