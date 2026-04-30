# Torque Extension Actor 系统开发计划

## 概述

**目标**: 在 Torque 中实现 Actor 模式的扩展系统，使扩展之间可以通过消息传递进行通信  
**基础**: 复用现有的 `torque-kernel` 类型 (`AgentInstance`, `DelegationRequest`, `ExecutionResult`)  
**范围**: 新建 `torque-extension` crate，定义 Extension Runtime 和核心 trait

---

## Phase 1: 核心抽象层

**目标**: 定义 Extension Actor 的核心类型和 trait  
**时间**: 预估 2-3 周

### 1.1 新建 crate 结构

- [ ] 创建 `crates/torque-extension/Cargo.toml`
- [ ] 定义 `torque-extension` workspace 依赖:
  - `torque-kernel` (核心类型复用)
  - `tokio` (异步运行时)
  - `serde` (序列化)
  - `uuid` (ID 生成)
  - `thiserror` (错误处理)
  - `tracing` (日志)

### 1.2 核心 ID 类型

- [ ] 定义 `ExtensionId` (基于 `AgentInstanceId` 或独立)
- [ ] 定义 `ExtensionTopic` (主题订阅标识)
- [ ] 定义 `ExtensionVersion` (语义化版本)

### 1.3 Extension Actor Trait

- [ ] 定义 `ExtensionActor` trait:
  ```rust
  pub trait ExtensionActor: Send + Sync {
      fn id(&self) -> ExtensionId;
      fn name(&self) -> &'static str;
      fn version(&self) -> ExtensionVersion;
      async fn on_start(&self, ctx: &ExtensionContext) -> Result<()>;
      async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()>;
      async fn handle(&self, ctx: &ExtensionContext, msg: ExtensionMessage) -> Result<ExtensionResponse>;
      fn hook_points(&self) -> Vec<HookPoint>;
  }
  ```

### 1.4 Extension Context

- [ ] 定义 `ExtensionContext` 结构体
- [ ] 实现消息发送方法:
  - `send(target, msg)` - Fire-and-Forget
  - `call(target, req)` - Request/Reply
- [ ] 实现事件发布/订阅方法:
  - `publish(topic, event)`
  - `subscribe(topic, handler)`
- [ ] 实现状态管理方法:
  - `get_state(key)`
  - `set_state(key, value)`

### 1.5 消息类型

- [ ] 定义 `ExtensionMessage` (复用 `DelegationRequest` 格式)
- [ ] 定义 `ExtensionRequest` 结构体
- [ ] 定义 `ExtensionResponse` 结构体
- [ ] 定义 `ExtensionEvent` 结构体

### 1.6 错误类型

- [ ] 定义 `ExtensionError` enum:
  - `NotFound` - Extension 不存在
  - `Timeout` - 消息超时
  - `SerializationError` - 序列化失败
  - `HookPointError` - Hook 点错误
  - `LifecycleError` - 生命周期错误

### 1.7 Hook Point 定义

- [ ] 定义 `HookPoint` enum:
  ```rust
  pub enum HookPoint {
      PreExecution,
      PostExecution,
      OnToolCall,
      OnDelegation,
      OnArtifactCreated,
      OnApprovalRequested,
      OnCheckpoint,
      OnRecovery,
      OnTeamEvent,
  }
  ```

### 1.8 验证标准

- [ ] 所有 trait 方法可编译
- [ ] 基础单元测试覆盖核心逻辑
- [ ] 与 `torque-kernel` 无循环依赖

---

## Phase 2: Extension Runtime

**目标**: 实现 Extension 运行时，管理 Extension 的生命周期和消息路由  
**时间**: 预估 2-3 周

### 2.1 Extension Runtime Trait

- [ ] 定义 `ExtensionRuntime` trait:
  ```rust
  pub trait ExtensionRuntime: Send + Sync {
      async fn spawn(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId>;
      async fn terminate(&self, id: ExtensionId) -> Result<()>;
      async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()>;
      async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse>;
      async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()>;
      fn find(&self, name: &str) -> Option<Arc<dyn ExtensionActor>>;
      fn list(&self) -> Vec<ExtensionId>;
  }
  ```

### 2.2 In-Memory Extension Runtime 实现

- [ ] 实现 `InMemoryExtensionRuntime`:
  - 使用 `HashMap<ExtensionId, Arc<dyn ExtensionActor>>` 存储
  - 使用 channel 进行消息传递
  - 实现 spawn/terminate 生命周期
  - 实现消息路由逻辑

### 2.3 消息邮箱实现

- [ ] 为每个 Extension 实现专属 mailbox
- [ ] 实现消息队列:
  - 按顺序处理消息
  - 支持并发控制
  - 消息优先级 (可选)

### 2.4 事件总线

- [ ] 实现 `TopicRegistry`:
  - `HashMap<ExtensionTopic, Vec<ExtensionId>>`
  - 主题订阅/退订
- [ ] 实现事件分发逻辑:
  - 同步分发
  - 异步分发 (可选)

### 2.5 生命周期管理

- [ ] 实现 Extension 状态机:
  - `Loaded` → `Registered` → `Initialized` → `Running`
  - `Running` → `Suspended` → `Stopped`
  - 错误状态处理

### 2.6 验证标准

- [ ] Runtime 可正常 spawn/terminate Extension
- [ ] Fire-and-Forget 消息可送达
- [ ] Request/Reply 消息可正确响应
- [ ] Pub/Sub 事件可正确分发

---

## Phase 3: 与 Torque 集成

**目标**: 将 Extension Runtime 集成到 `torque-harness` 中  
**时间**: 预估 2-3 周

### 3.1 Harness 集成点

- [ ] 在 `AppState` 或 `ServiceContainer` 中添加 `ExtensionRuntime`
- [ ] 实现 `HarnessExtensionRuntime`:
  - 包装 `InMemoryExtensionRuntime`
  - 与现有 Service 层集成

### 3.2 Hook 点实现

- [ ] 实现 `PreExecutionHook`:
  - 在 `AgentInstance` 执行前调用
  - 可拦截/修改执行请求
- [ ] 实现 `PostExecutionHook`:
  - 在 `AgentInstance` 执行后调用
  - 可处理执行结果
- [ ] 实现 `OnToolCallHook`:
  - 在 Tool 调用前后触发
- [ ] 实现其他 Hook 点 (根据需要)

### 3.3 Team Supervisor 集成

- [ ] 定义 `TeamExtensionRole`:
  - Extension 作为 Team Member 参与协作
  - 可接收 delegation
  - 可发布 artifact

### 3.4 API 端点

- [ ] 实现 `POST /v1/extensions` - 注册 Extension
- [ ] 实现 `DELETE /v1/extensions/{id}` - 注销 Extension
- [ ] 实现 `GET /v1/extensions` - 列出 Extension
- [ ] 实现 `POST /v1/extensions/{id}/messages` - 发送消息

### 3.5 配置管理

- [ ] 在 `app.rs` 中添加 Extension 配置:
  - 是否启用 Extension
  - 内置 Extension 列表
  - Extension 加载顺序

### 3.6 验证标准

- [ ] Extension 可通过 API 注册
- [ ] Hook 点可正常触发
- [ ] Extension 消息可路由到 Team Supervisor

---

## Phase 4: 内置 Extension 示例

**目标**: 实现 1-2 个内置 Extension 作为参考实现  
**时间**: 预估 1-2 周

### 4.1 Logging Extension (日志扩展)

- [ ] 订阅所有 `PostExecution` 事件
- [ ] 将执行结果写入指定日志系统
- [ ] 提供日志查询 API

### 4.2 Metrics Extension (指标扩展)

- [ ] 收集执行指标:
  - 执行次数
  - 执行时长
  - Tool 调用频率
  - 错误率
- [ ] 提供指标导出接口

### 4.3 验证标准

- [ ] 内置 Extension 可正常加载
- [ ] Logging Extension 可记录执行事件
- [ ] Metrics Extension 可收集和导出指标

---

## Phase 5: 持久化和恢复

**目标**: Extension 状态持久化，支持重启恢复  
**时间**: 预估 1-2 周

### 5.1 Extension 状态持久化

- [ ] 定义 `ExtensionSnapshot` 结构体:
  ```rust
  pub struct ExtensionSnapshot {
      id: ExtensionId,
      name: String,
      version: ExtensionVersion,
      state: serde_json::Value,
      mailbox_position: u64,
      registered_topics: Vec<ExtensionTopic>,
  }
  ```

### 5.2 Recovery 集成

- [ ] 实现 `ExtensionRecoveryManager`:
  - 保存 Extension 快照
  - 恢复 Extension 状态
  - 处理 Extension 版本迁移

### 5.3 验证标准

- [ ] 重启后 Extension 可恢复
- [ ] Extension 消息不丢失
- [ ] 状态一致性验证

---

## Phase 6: 分布式支持 (可选)

**目标**: 支持跨进程/跨机器的 Extension 通信  
**时间**: 预估 3-4 周

### 6.1 Remote Extension Runtime

- [ ] 定义 `RemoteExtensionRuntime` trait
- [ ] 实现基于 gRPC 的传输层

### 6.2 Service Discovery

- [ ] 实现 Extension 注册中心
- [ ] 实现负载均衡策略

### 6.3 验证标准

- [ ] 跨进程消息可达
- [ ] 延迟可接受

---

## 里程碑

| 阶段 | 里程碑 | 交付物 |
|------|--------|--------|
| Phase 1 | Core Traits | `ExtensionActor`, `ExtensionContext` |
| Phase 2 | Runtime | `InMemoryExtensionRuntime` |
| Phase 3 | Integration | Hook 点, API 端点 |
| Phase 4 | Examples | Logging, Metrics Extension |
| Phase 5 | Persistence | Snapshot, Recovery |
| Phase 6 | Distributed | Remote Runtime (可选) |

---

## 技术债务和风险

### 技术债务

1. **类型重复**: `ExtensionMessage` 与 `DelegationRequest` 可能有重复字段
   - 缓解: ExtensionMessage 可直接包装 DelegationRequest

2. **错误处理一致性**: Extension 错误与 Kernel 错误需要统一
   - 缓解: 定义统一的错误转换层

### 风险

1. **性能**: 每个 Extension 独立 mailbox 可能带来内存开销
   - 缓解: 实现 mailbox 容量限制

2. **调试困难**: 分布式 Actor 调试复杂
   - 缓解: 实现 tracing 集成

3. **版本升级**: Extension 版本升级可能破坏兼容性
   - 缓解: 实现版本协商机制

---

## 依赖关系

```
Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4
   │           │           │           │
   ▼           ▼           ▼           ▼
 独立       依赖        依赖        依赖
 测试       Phase 1     Phase 2     Phase 3
                            │
                            ▼
                      Phase 5 (可选)
```

---

## 后续扩展方向

1. **动态加载**: 支持动态加载 Extension (.so 文件)
2. **安全沙箱**: Extension 权限控制
3. **流量控制**: Extension 调用频率限制
4. **可视化**: Extension 状态监控面板
