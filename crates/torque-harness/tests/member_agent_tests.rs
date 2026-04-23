use torque_harness::service::team::local_member_agent::LocalMemberAgent;
use torque_harness::service::team::MemberAgent;

#[tokio::test]
async fn test_member_agent_trait_exists() {
    fn assert_member_agent<T: MemberAgent>() {}
    assert_member_agent::<LocalMemberAgent>();
}

#[tokio::test]
async fn test_local_member_agent_creation() {
    let member_id = uuid::Uuid::new_v4();
    let agent = LocalMemberAgent::new(member_id);
    let health = agent.health_check().await.unwrap();
    assert_eq!(health.member_id, member_id);
    assert!(health.is_healthy);
}

#[tokio::test]
async fn test_member_agent_poll_returns_empty_by_default() {
    let member_id = uuid::Uuid::new_v4();
    let agent = LocalMemberAgent::new(member_id);
    let tasks = agent.poll_tasks().await.unwrap();
    assert!(tasks.is_empty());
}
