use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use torque_harness::infra::tool_registry::ToolRegistry;
use torque_harness::tools::{Tool, ToolArc, ToolResult};

// ── Helpers ─────────────────────────────────────────────────────────

fn make_test_tool(name: &str) -> ToolArc {
    struct TestTool {
        name: String,
    }
    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "test tool"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }
        async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
            Ok(ToolResult {
                success: true,
                content: "ok".to_string(),
                error: None,
            })
        }
    }
    Arc::new(TestTool {
        name: name.to_string(),
    }) as ToolArc
}

// ── ToolRegistry Tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_registry_register_and_get() {
    let registry = ToolRegistry::new();
    let tool = make_test_tool("my_tool");

    registry.register(tool.clone()).await;

    let retrieved = registry.get("my_tool").await;
    assert!(retrieved.is_some(), "should find registered tool");
    assert_eq!(retrieved.unwrap().name(), "my_tool");
}

#[tokio::test]
async fn test_registry_remove_returns_true_when_exists() {
    let registry = ToolRegistry::new();
    registry.register(make_test_tool("to_remove")).await;

    let removed = registry.remove("to_remove").await;
    assert!(removed, "remove should return true for existing tool");

    let retrieved = registry.get("to_remove").await;
    assert!(retrieved.is_none(), "tool should be gone after removal");
}

#[tokio::test]
async fn test_registry_remove_returns_false_when_missing() {
    let registry = ToolRegistry::new();
    let removed = registry.remove("nonexistent").await;
    assert!(!removed, "remove should return false for missing tool");
}

#[tokio::test]
async fn test_registry_update_returns_true_when_exists() {
    let registry = ToolRegistry::new();
    let original = make_test_tool("my_tool");
    registry.register(original).await;

    let replacement = make_test_tool("my_tool");
    let updated = registry.update("my_tool", replacement.clone()).await;
    assert!(updated, "update should return true when tool exists");

    let retrieved = registry.get("my_tool").await;
    assert!(retrieved.is_some(), "tool should still exist after update");
}

#[tokio::test]
async fn test_registry_update_returns_false_when_missing() {
    let registry = ToolRegistry::new();
    let tool = make_test_tool("ghost");
    let updated = registry.update("ghost", tool).await;
    assert!(!updated, "update should return false when tool does not exist");
}

#[tokio::test]
async fn test_registry_list_after_register_and_remove() {
    let registry = ToolRegistry::new();
    registry.register(make_test_tool("alpha")).await;
    registry.register(make_test_tool("beta")).await;
    registry.register(make_test_tool("gamma")).await;

    assert_eq!(registry.list().await.len(), 3);
    assert_eq!(registry.list_tool_names().await.len(), 3);

    registry.remove("beta").await;

    let names = registry.list_tool_names().await;
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alpha".to_string()));
    assert!(!names.contains(&"beta".to_string()));
    assert!(names.contains(&"gamma".to_string()));
}

#[tokio::test]
async fn test_registry_concurrent_registration() {
    let registry = Arc::new(ToolRegistry::new());
    let mut handles = vec![];

    for i in 0..20 {
        let reg = registry.clone();
        handles.push(tokio::spawn(async move {
            let name = format!("concurrent_tool_{}", i);
            reg.register(make_test_tool(&name)).await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let names = registry.list_tool_names().await;
    assert_eq!(names.len(), 20, "all 20 tools should be registered");
}

#[tokio::test]
async fn test_registry_concurrent_remove_and_list() {
    let registry = Arc::new(ToolRegistry::new());
    for i in 0..50 {
        registry
            .register(make_test_tool(&format!("tool_{}", i)))
            .await;
    }

    let reg_clone = registry.clone();
    let remover = tokio::spawn(async move {
        for i in 0..50 {
            reg_clone.remove(&format!("tool_{}", i)).await;
        }
    });

    let reg_for_read = registry.clone();
    let reader = tokio::spawn(async move {
        for _ in 0..10 {
            let _names = reg_for_read.list_tool_names().await;
        }
    });

    let (r1, r2) = tokio::join!(remover, reader);
    assert!(r1.is_ok());
    assert!(r2.is_ok());

    let remaining = registry.list_tool_names().await;
    assert!(remaining.is_empty(), "all tools should be removed");
}

#[tokio::test]
async fn test_registry_update_is_atomic() {
    let registry = Arc::new(ToolRegistry::new());
    registry.register(make_test_tool("target")).await;

    // Concurrent reads should see a consistent state during update.
    let reg_clone = registry.clone();
    let reader = tokio::spawn(async move {
        for _ in 0..5 {
            let tool = reg_clone.get("target").await;
            assert!(tool.is_some(), "tool should exist during concurrent reads");
        }
    });

    let updater = tokio::spawn(async move {
        for _ in 0..10 {
            registry
                .update("target", make_test_tool("target"))
                .await;
        }
    });

    let (r1, r2) = tokio::join!(reader, updater);
    assert!(r1.is_ok());
    assert!(r2.is_ok());
}

// ── ToolService Tests ───────────────────────────────────────────────

use torque_harness::service::ToolService;

async fn make_tool_service() -> ToolService {
    ToolService::new()
}

#[tokio::test]
async fn test_tool_service_register_and_list() {
    let service = make_tool_service().await;
    service.register_tool(make_test_tool("svc_tool")).await;

    let names = service.list_tool_names().await;
    assert!(names.contains(&"svc_tool".to_string()));
}

#[tokio::test]
async fn test_tool_service_unregister() {
    let service = make_tool_service().await;
    service.register_tool(make_test_tool("to_unreg")).await;

    let removed = service.unregister_tool("to_unreg").await;
    assert!(removed, "should succeed unregistering existing tool");

    let names = service.list_tool_names().await;
    assert!(!names.contains(&"to_unreg".to_string()));
}

#[tokio::test]
async fn test_tool_service_unregister_missing() {
    let service = make_tool_service().await;
    let removed = service.unregister_tool("missing").await;
    assert!(!removed, "should return false for missing tool");
}

#[tokio::test]
async fn test_tool_service_get_tool() {
    let service = make_tool_service().await;
    service.register_tool(make_test_tool("get_me")).await;

    let tool = service.get_tool("get_me").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name(), "get_me");

    let missing = service.get_tool("not_there").await;
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_tool_service_list_is_empty_initially() {
    let service = make_tool_service().await;
    let names = service.list_tool_names().await;
    assert!(names.is_empty(), "new ToolService should have no tools");
}

#[tokio::test]
async fn test_tool_service_register_multiple() {
    let service = make_tool_service().await;
    service.register_tool(make_test_tool("a")).await;
    service.register_tool(make_test_tool("b")).await;
    service.register_tool(make_test_tool("c")).await;

    let names = service.list_tool_names().await;
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"a".to_string()));
    assert!(names.contains(&"b".to_string()));
    assert!(names.contains(&"c".to_string()));
}
