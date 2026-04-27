use async_trait::async_trait;
use chrono::Utc;
use std::sync::Mutex;
use torque_harness::models::v1::tool_policy::{ToolPolicy, ToolRiskLevel, ToolSideEffect};
use torque_harness::repository::ToolPolicyRepository;
use uuid::Uuid;

struct InMemoryToolPolicyRepository {
    policies: Mutex<Vec<ToolPolicy>>,
}

impl InMemoryToolPolicyRepository {
    fn new() -> Self {
        Self {
            policies: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ToolPolicyRepository for InMemoryToolPolicyRepository {
    async fn upsert(&self, policy: &ToolPolicy) -> anyhow::Result<bool> {
        let mut policies = self.policies.lock().unwrap();
        let existing = policies.iter_mut().find(|p| p.tool_name == policy.tool_name);
        if let Some(e) = existing {
            e.risk_level = policy.risk_level;
            e.requires_approval = policy.requires_approval;
            e.blocked = policy.blocked;
            e.side_effects = policy.side_effects.clone();
            e.updated_at = Utc::now();
            Ok(false)
        } else {
            policies.push(ToolPolicy {
                id: Uuid::new_v4(),
                tool_name: policy.tool_name.clone(),
                risk_level: policy.risk_level,
                side_effects: policy.side_effects.clone(),
                requires_approval: policy.requires_approval,
                blocked: policy.blocked,
                blocked_reason: policy.blocked_reason.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
            Ok(true)
        }
    }

    async fn get(&self, tool_name: &str) -> anyhow::Result<Option<ToolPolicy>> {
        Ok(self
            .policies
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.tool_name == tool_name)
            .cloned())
    }

    async fn list(&self) -> anyhow::Result<Vec<ToolPolicy>> {
        Ok(self.policies.lock().unwrap().clone())
    }

    async fn delete(&self, tool_name: &str) -> anyhow::Result<()> {
        self.policies.lock().unwrap().retain(|p| p.tool_name != tool_name);
        Ok(())
    }
}

fn make_tool_policy(name: &str, risk: ToolRiskLevel) -> ToolPolicy {
    ToolPolicy {
        id: Uuid::new_v4(),
        tool_name: name.to_string(),
        risk_level: risk,
        side_effects: vec![ToolSideEffect::FileSystem],
        requires_approval: risk.requires_approval(),
        blocked: false,
        blocked_reason: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn list_returns_empty_when_no_policies() {
    let repo = InMemoryToolPolicyRepository::new();

    let policies = repo.list().await.expect("list succeeds");
    assert!(policies.is_empty());
}

#[tokio::test]
async fn upsert_inserts_new_policy() {
    let repo = InMemoryToolPolicyRepository::new();
    let policy = make_tool_policy("web_search", ToolRiskLevel::High);

    let is_insert = repo.upsert(&policy).await.expect("upsert");
    assert!(is_insert);

    let found = repo.get("web_search").await.expect("get");
    assert!(found.is_some());
    assert_eq!(found.unwrap().risk_level, ToolRiskLevel::High);
}

#[tokio::test]
async fn upsert_updates_existing_policy() {
    let repo = InMemoryToolPolicyRepository::new();
    let policy = make_tool_policy("read_file", ToolRiskLevel::Low);
    repo.upsert(&policy).await.expect("upsert insert");

    let updated = ToolPolicy {
        risk_level: ToolRiskLevel::Critical,
        ..policy.clone()
    };
    let is_insert = repo.upsert(&updated).await.expect("upsert update");
    assert!(!is_insert);

    let found = repo.get("read_file").await.expect("get");
    assert_eq!(found.unwrap().risk_level, ToolRiskLevel::Critical);
}

#[tokio::test]
async fn get_returns_none_for_unknown_tool() {
    let repo = InMemoryToolPolicyRepository::new();

    let result = repo.get("nonexistent").await.expect("get");
    assert!(result.is_none());
}

#[tokio::test]
async fn delete_removes_policy() {
    let repo = InMemoryToolPolicyRepository::new();
    let policy = make_tool_policy("execute_command", ToolRiskLevel::Critical);
    repo.upsert(&policy).await.expect("insert");

    repo.delete("execute_command").await.expect("delete");

    let result = repo.get("execute_command").await.expect("get");
    assert!(result.is_none());
}

#[tokio::test]
async fn list_returns_all_policies_in_order() {
    let repo = InMemoryToolPolicyRepository::new();
    repo.upsert(&make_tool_policy("a_tool", ToolRiskLevel::Low)).await.expect("insert");
    repo.upsert(&make_tool_policy("b_tool", ToolRiskLevel::Medium)).await.expect("insert");
    repo.upsert(&make_tool_policy("c_tool", ToolRiskLevel::High)).await.expect("insert");

    let policies = repo.list().await.expect("list");
    assert_eq!(policies.len(), 3);
}

#[tokio::test]
async fn approval_required_for_high_and_critical() {
    assert!(ToolRiskLevel::High.requires_approval());
    assert!(ToolRiskLevel::Critical.requires_approval());
    assert!(!ToolRiskLevel::Medium.requires_approval());
    assert!(!ToolRiskLevel::Low.requires_approval());
}

#[tokio::test]
async fn critical_is_privileged() {
    assert!(ToolRiskLevel::Critical.is_privileged());
    assert!(!ToolRiskLevel::High.is_privileged());
    assert!(!ToolRiskLevel::Medium.is_privileged());
    assert!(!ToolRiskLevel::Low.is_privileged());
}
