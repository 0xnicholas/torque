# Torque Extension Actor 系统 - 完整开发计划 (修订版)

## 背景

**现状**: Torque 目前没有插件扩展机制，扩展之间无法通信。

**目标**: 在 Torque 中实现 Actor 模式的扩展系统，使第三方可以在 Torque 运行时中加载和执行扩展，并支持扩展之间的消息传递。

---

## Phase 0: 预研与启动

**目标**: 确认技术方案、确定 API 边界、完成架构设计评审  
**预估时间**: 1 周

### 0.1 技术方案确认

- [ ] 确认 Actor 方案作为扩展通信模型
  - 优势: 复用 `AgentInstance` 已有状态机模型
  - 优势: 每个 Extension 独立 mailbox，天然隔离
  - 优势: 支持 Request/Reply、Pub/Sub 多种模式
  - 风险: 引入异步复杂性

- [ ] 确认 crate 结构
  - `crates/torque-extension`: 独立 crate，避免循环依赖
  - 依赖: `torque-kernel`, `tokio`, `serde`

- [ ] 确认与现有系统边界
  - Extension 不直接依赖 `torque-harness`
  - Extension 通过 Hook 点与 Torque 核心交互
  - API 层在 `torque-harness` 中实现

### 0.2 API 设计评审

- [ ] 评审 Extension 注册 API
  ```http
  POST /v1/extensions
  DELETE /v1/extensions/{id}
  GET /v1/extensions
  ```

- [ ] 评审消息传递 API
  ```http
  POST /v1/extensions/{id}/messages
  GET /v1/extensions/{id}/subscriptions
  ```

- [ ] 评审 Hook 点配置 API
  ```http
  POST /v1/extensions/{id}/hooks
  DELETE /v1/extensions/{id}/hooks/{hook_point}
  ```

### 0.3 文档准备

- [ ] 编写 Extension 开发指南 (草案)
- [ ] 编写 Extension API 参考文档 (草案)
- [ ] 确定内置 Extension 示例需求

### 0.4 验证标准

- [ ] 技术方案评审通过
- [ ] API 设计评审通过
- [ ] 确定 crate 边界和依赖关系

---

## Phase 1: 核心抽象层

**目标**: 定义 Extension Actor 的核心类型和 trait  
**预估时间**: 2-3 周

### 1.1 Crate 初始化

- [ ] 创建 `crates/torque-extension/Cargo.toml`
- [ ] 添加 workspace 依赖配置
- [ ] 创建 `src/lib.rs` 入口文件

### 1.2 核心类型定义

- [ ] `ExtensionId` - 基于 `AgentInstanceId`
- [ ] `ExtensionVersion` - 语义化版本
- [ ] `ExtensionTopic` - 主题订阅标识
- [ ] `HookPoint` - Hook 点枚举

### 1.3 ExtensionActor Trait

```rust
#[async_trait]
pub trait ExtensionActor: Send + Sync {
    fn id(&self) -> ExtensionId;
    fn name(&self) -> &'static str;
    fn version(&self) -> ExtensionVersion;
    fn hook_points(&self) -> Vec<HookPoint>;
    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()>;
    async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()>;
    async fn handle(&self, ctx: &ExtensionContext, msg: ExtensionMessage) -> Result<ExtensionResponse>;
}
```

### 1.4 ExtensionContext

- [ ] 消息发送方法 (`send`, `call`)
- [ ] 事件发布/订阅方法
- [ ] 状态管理方法

### 1.5 消息类型

- [ ] `ExtensionMessage` - 消息枚举
- [ ] `ExtensionRequest` - 请求结构
- [ ] `ExtensionResponse` - 响应结构
- [ ] `ExtensionEvent` - 事件结构

### 1.6 错误类型

- [ ] `ExtensionError` enum
- [ ] 错误代码定义

### 1.7 状态管理

- [ ] `ExtensionState` - 键值存储
- [ ] `SharedExtensionState` - 线程安全包装

### 1.8 验证标准

- [ ] `cargo check --package torque-extension` 通过
- [ ] 类型可序列化/反序列化
- [ ] 与 `torque-kernel` 无循环依赖

---

## Phase 2: Extension Runtime

**目标**: 实现 Extension 运行时，管理生命周期和消息路由  
**预估时间**: 2-3 周

### 2.1 Runtime Trait

```rust
#[async_trait]
pub trait ExtensionRuntime: Send + Sync {
    async fn register(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId>;
    async fn unregister(&self, id: ExtensionId) -> Result<()>;
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()>;
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse>;
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()>;
    async fn subscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()>;
    fn find(&self, name: &str) -> Option<ExtensionId>;
    fn list(&self) -> Vec<ExtensionId>;
}
```

### 2.2 Mailbox 实现

- [ ] `Mailbox` - 消息队列
- [ ] `MailboxSender` - 发送端
- [ ] 消息优先级 (可选)

### 2.3 InMemory Runtime

- [ ] `InMemoryExtensionRuntime` 实现
- [ ] 生命周期管理
- [ ] 消息路由逻辑
- [ ] 主题订阅管理

### 2.4 验证标准

- [ ] Extension 可注册/注销
- [ ] Fire-and-Forget 消息可达
- [ ] Request/Reply 正确响应
- [ ] Pub/Sub 正确分发
- [ ] Hook 点可注册和触发

---

## Phase 3: 与 Torque Harness 集成

**目标**: 将 Extension Runtime 集成到 `torque-harness`  
**预估时间**: 2-3 周

### 3.1 ServiceContainer 集成

- [ ] 添加 `ExtensionConfig`
- [ ] 添加 `ExtensionService`
- [ ] 添加 `ExtensionHookTrigger`

### 3.2 Hook 点实现

- [ ] `PreExecution` hook
- [ ] `PostExecution` hook
- [ ] `PreToolCall` / `PostToolCall` hook
- [ ] `OnArtifactCreated` hook

### 3.3 API 端点

- [ ] `GET /v1/extensions` - 列表
- [ ] `POST /v1/extensions` - 注册
- [ ] `DELETE /v1/extensions/:id` - 注销
- [ ] `POST /v1/extensions/:id/messages` - 发消息

### 3.4 验证标准

- [ ] Extension 可通过 API 注册
- [ ] Hook 点正常触发
- [ ] 内置 Extension 加载正常

---

## Phase 4: 内置 Extension 示例

**目标**: 实现 1-2 个内置 Extension 作为参考  
**预估时间**: 1-2 周

### 4.1 Logging Extension

- [ ] 订阅 PostExecution hook
- [ ] 记录执行事件
- [ ] 可配置日志级别

### 4.2 Metrics Extension

- [ ] 收集执行指标
- [ ] 计数器 + 直方图
- [ ] 指标导出 API

### 4.3 验证标准

- [ ] Logging Extension 记录事件
- [ ] Metrics Extension 收集指标
- [ ] 可通过配置启用/禁用

---

## Phase 5: 持久化和恢复

**目标**: Extension 状态持久化，支持重启恢复  
**预估时间**: 1-2 周

### 5.1 快照管理

- [ ] `ExtensionSnapshot` 结构
- [ ] `SnapshotManager`
- [ ] `SnapshotStorage` trait

### 5.2 恢复管理

- [ ] `ExtensionRecoveryManager`
- [ ] 与 Checkpoint 同步
- [ ] 版本迁移支持

### 5.3 数据库 Schema

- [ ] `extension_snapshots` 表
- [ ] `extension_registry` 表
- [ ] 迁移脚本

### 5.4 验证标准

- [ ] Extension 重启后恢复
- [ ] 状态一致性验证

---

## Phase 6: 分布式支持 (可选)

**目标**: 跨进程 Extension 通信  
**预估时间**: 3-4 周

### 6.1 传输层

- [ ] `Transport` trait
- [ ] `RedisTransport` 实现

### 6.2 服务发现

- [ ] `ServiceRegistry`
- [ ] Extension 位置注册

### 6.3 远程 Runtime

- [ ] `RemoteExtensionRuntime`
- [ ] 透明路由

### 6.4 负载均衡

- [ ] `LoadBalancer`
- [ ] 多种策略实现

### 6.5 验证标准

- [ ] 跨进程消息可达
- [ ] 故障恢复正常

---

## 里程碑总览

| 阶段 | 里程碑 | 预估时间 | 交付物 |
|------|--------|----------|--------|
| Phase 0 | 预研完成 | 1 周 | 技术方案、API 设计 |
| Phase 1 | 核心抽象 | 2-3 周 | Trait、类型定义 |
| Phase 2 | Runtime | 2-3 周 | InMemory Runtime |
| Phase 3 | 集成 | 2-3 周 | Hook、API |
| Phase 4 | 示例 | 1-2 周 | Logging、Metrics |
| Phase 5 | 持久化 | 1-2 周 | Snapshot、Recovery |
| Phase 6 | 分布式 (可选) | 3-4 周 | Remote Runtime |

**总计**: 10-17 周 (不含 Phase 6)

---

## 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 异步复杂性 | 开发周期 | Phase 1 充分测试 |
| 性能开销 | 运行时性能 | Mailbox 容量限制 |
| 版本兼容性 | 扩展升级 | 版本协商机制 |
| 安全风险 | 恶意扩展 | 沙箱隔离 (未来) |

---

## 后续扩展方向

1. **动态加载**: 运行时加载 .so 扩展
2. **安全沙箱**: Extension 权限控制
3. **可视化**: Extension 状态监控面板
4. ** Marketplace**: 扩展市场

---

## 文档清单

1. `plans/2026-04-30-torque-extension-actor-phase1-2-detailed-v1.md` - Phase 1-2 详细设计
2. `plans/2026-04-30-torque-extension-actor-phase3-4-detailed-v1.md` - Phase 3-4 详细设计
3. `plans/2026-04-30-torque-extension-actor-phase5-6-detailed-v1.md` - Phase 5-6 详细设计
4. `plans/2026-04-30-torque-extension-actor-development-v1.md` - 完整开发计划
