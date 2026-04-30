# Phase 0: 预研与启动 - 详细执行计划

## 概述

**目标**: 完成 Extension 系统的技术方案确认、API 设计评审和启动准备  
**时间**: 1 周 (5 个工作日)  
**产出**: 技术方案评审通过文档、API 设计定稿、Phase 1 启动准备就绪

---

## 任务分解

### Day 1: 技术方案深度评审

#### 上午: Actor 方案最终确认

- [ ] **1.1.1** 评审 Actor 模型选型
  - 确认复用 `AgentInstance` 状态机模式的合理性
  - 评估 Mailbox 实现的复杂度
  - 确认异步消息传递模型适合 Torque 场景
  
  评审要点:
  ```
  候选方案:
  - Actor (当前选择): 状态隔离、消息驱动、适合分布式
  - Event Bus: 简单但耦合度高
  - Shared State: 简单但扩展性差
  
  决策: 采用 Actor，理由:
  1. Torque 已有 AgentInstance 作为 Actor 基础
  2. Mailbox 模式天然支持 Request/Reply 和 Pub/Sub
  3. 易于实现隔离和故障恢复
  ```

- [ ] **1.1.2** 确认 crate 结构
  - 独立 `torque-extension` crate
  - 依赖 `torque-kernel` (核心类型)
  - 可选依赖 `tokio` (异步运行时)
  
  依赖关系图:
  ```
  torque-kernel (无依赖)
       │
       ▼
  torque-extension (依赖 kernel)
       │
       ▼
  torque-harness (依赖 extension)
  ```

#### 下午: API 边界确认

- [ ] **1.2.1** 确认 Extension 与 Torque 交互边界
  
  边界定义:
  ```
  Extension ──→ Hook 点 ──→ Torque 执行流程
  Extension ──→ 消息 ──→ 其他 Extension
  Extension ──→ 事件 ──→ 外部系统
  
  不在边界内:
  ✗ Extension 不直接操作数据库
  ✗ Extension 不直接调用 LLM
  ✗ Extension 不直接访问文件系统
  ```

- [ ] **1.2.2** 确认 Hook 点粒度
  - 评审 Hook 点列表是否满足扩展需求
  - 确认 Pre/Post 分离的必要性
  - 评估 Intercept 类型 Hook 的复杂性

### Day 2: API 设计评审

#### 上午: REST API 评审

- [ ] **2.1.1** 评审 Extension 注册 API
  
  ```http
  POST /v1/extensions
  Content-Type: application/json
  
  {
    "name": "my-extension",
    "version": "1.0.0",
    "hooks": ["post_execution", "on_tool_call"],
    "config": {}
  }
  
  Response: 201 Created
  {
    "id": "uuid",
    "name": "my-extension",
    "status": "registered"
  }
  ```

  评审问题:
  - [ ] 是否需要预审批流程？
  - [ ] 配置项是否支持热更新？
  - [ ] 初始状态是否应该是 "registered" 还是 "initialized"？

- [ ] **2.1.2** 评审消息传递 API
  
  ```http
  POST /v1/extensions/{id}/messages
  Content-Type: application/json
  
  {
    "type": "command",
    "action": {
      "type": "execute",
      "goal": "analyze this",
      "instructions": ["step 1", "step 2"]
    }
  }
  
  Response: 202 Accepted
  {
    "correlation_id": "uuid"
  }
  ```

  评审问题:
  - [ ] 消息是否需要持久化？
  - [ ] 是否支持批量消息？
  - [ ] 响应超时如何处理？

- [ ] **2.1.3** 评审订阅管理 API
  
  ```http
  POST /v1/extensions/{id}/subscriptions
  Content-Type: application/json
  
  {
    "topic": "torque:execution.completed"
  }
  
  DELETE /v1/extensions/{id}/subscriptions/{topic}
  ```

#### 下午: 内部 API 评审 (Rust API)

- [ ] **2.2.1** 评审 ExtensionActor trait
  
  ```rust
  #[async_trait]
  pub trait ExtensionActor: Send + Sync {
      fn id(&self) -> ExtensionId;
      fn name(&self) -> &'static str;
      fn version(&self) -> ExtensionVersion;
      
      // 生命周期
      async fn on_start(&self, ctx: &ExtensionContext) -> Result<()>;
      async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()>;
      
      // 消息处理
      async fn handle(&self, ctx: &ExtensionContext, msg: ExtensionMessage) 
          -> Result<ExtensionResponse>;
  }
  ```

  评审问题:
  - [ ] `on_start` 是否应该是同步的？
  - [ ] 是否需要 `on_error` 回调？
  - [ ] 是否需要 `on_config_update` 回调？

- [ ] **2.2.2** 评审 HookResult 处理
  
  ```rust
  pub enum HookResult {
      Continue,                    // 继续执行
      Blocked { reason: String }, // 阻止执行
      Modified(serde_json::Value), // 修改后继续
      ShortCircuit(serde_json::Value), // 短路返回
  }
  ```

  评审问题:
  - [ ] `Modified` 的修改范围是什么？
  - [ ] `ShortCircuit` 的返回值如何处理？
  - [ ] 多个 Extension 注册同一 Hook 如何合并结果？

### Day 3: 技术细节确认

#### 上午: 生命周期和状态机

- [ ] **3.1.1** 确认 Extension 生命周期状态
  
  ```
  Loaded ──► Registered ──► Initialized ──► Running
    │            │               │             │
    │            │               │             ▼
    │            │               │         Suspended
    │            │               │             │
    │            │               │             ▼
    │            │               │          Resumed
    │            │               │             │
    │            │               │             ▼
    │            │               │          Stopped
    │            │               │             │
    ▼            ▼               ▼             ▼
  Unloaded    Unregistered    Failed       Cleanup
  ```

- [ ] **3.1.2** 确认消息传递语义
  - Fire-and-Forget: 发送后不等待响应
  - Request/Reply: 同步等待响应，超时返回错误
  - Pub/Sub: 异步分发，不等待处理

- [ ] **3.1.3** 确认错误处理策略
  - Extension panic: 隔离，不影响其他 Extension
  - 消息超时: 默认 30 秒，可配置
  - Hook 失败: 记录日志，继续执行 (可配置为阻止)

#### 下午: 性能和安全考量

- [ ] **3.2.1** 确认性能约束
  ```
  性能目标:
  - Extension 消息延迟: < 10ms (同进程)
  - Hook 触发开销: < 1ms
  - 支持并发 Extension 数: 100+
  - Mailbox 默认容量: 1000 条消息
  ```

- [ ] **3.2.2** 确认安全约束 (初版)
  ```
  安全约束 (初版):
  - Extension 运行在相同进程空间
  - 无沙箱隔离 (Phase 6+)
  - 依赖 Rust 类型系统保证内存安全
  
  未来 (Phase 6+):
  - 进程隔离
  - 权限控制
  - 资源限制
  ```

- [ ] **3.2.3** 确认日志和监控
  - Extension 生命周期事件日志
  - 消息流量监控
  - Hook 执行指标

### Day 4: 文档和启动准备

#### 上午: 编写评审总结

- [ ] **4.1.1** 编写技术方案评审报告
  
  报告结构:
  ```markdown
  # Extension 系统技术方案评审报告
  
  ## 1. 背景
  ## 2. 技术方案
     2.1 Actor 模型选择
     2.2 Crate 结构
     2.3 API 边界
  ## 3. API 设计
     3.1 REST API
     3.2 Rust API
  ## 4. 技术决策
  ## 5. 开放问题
  ## 6. 后续计划
  ```

- [ ] **4.1.2** 编写 API 设计文档
  
  文档结构:
  ```markdown
  # Extension API 参考
  
  ## REST API
  ## Rust API
  ## 消息格式
  ## 错误代码
  ## 示例
  ```

#### 下午: Phase 1 启动准备

- [ ] **4.2.1** 创建 Phase 1 任务清单
  - 提取 Phase 1 的具体开发任务
  - 分配到人 (如有多人)
  - 预估工时

- [ ] **4.2.2** 准备开发环境
  - 创建 `crates/torque-extension` 目录结构
  - 准备 Cargo.toml 模板
  - 配置 CI/CD 流程 (可选)

- [ ] **4.2.3** 确定验收标准
  ```
  Phase 0 验收标准:
  ✓ 技术方案评审通过
  ✓ API 设计文档完成
  ✓ 开放问题清单明确
  ✓ Phase 1 任务清单就绪
  ✓ 开发环境准备完成
  ```

### Day 5: 评审和收尾

#### 上午: 内部评审

- [ ] **5.1.1** 组织技术评审会议
  - 评审技术方案
  - 评审 API 设计
  - 确认开放问题处理方案

- [ ] **5.1.2** 更新文档
  - 根据评审意见更新文档
  - 记录决策理由
  - 整理开放问题清单

#### 下午: 最终交付

- [ ] **5.2.1** 最终文档交付
  - 技术方案评审报告
  - API 设计文档
  - Phase 1 开发计划

- [ ] **5.2.2** Phase 1 Kickoff
  - 分配任务
  - 确认时间线
  - 启动开发

---

## 产出清单

| 文档 | 负责人 | 截止日期 | 状态 |
|------|--------|----------|------|
| 技术方案评审报告 | TBD | Day 4 | 待开始 |
| API 设计文档 | TBD | Day 4 | 待开始 |
| Phase 1 开发计划 | TBD | Day 4 | 待开始 |
| 开放问题清单 | TBD | Day 3 | 待开始 |

---

## 开放问题清单

| # | 问题 | 影响 | 处理方案 | 状态 |
|---|------|------|----------|------|
| O1 | Hook 合并策略 | 高 | 最严格者优先 | 待评审 |
| O2 | 配置热更新 | 中 | 通过 API 支持 | 待确认 |
| O3 | 扩展版本升级 | 中 | 需版本协商 | 待设计 |
| O4 | 资源配额限制 | 低 | Phase 6+ | 延期 |
| O5 | Extension 优先级 | 低 | 暂不支持 | 延期 |

---

## 评审 Checklist

### 技术方案评审

- [ ] Actor 模型是否适合 Torque 场景？
- [ ] crate 结构是否清晰？
- [ ] 依赖关系是否正确？
- [ ] 与现有 Torque 架构是否兼容？

### API 设计评审

- [ ] REST API 是否符合 Torque 风格？
- [ ] Rust API 是否 ergonomics 良好？
- [ ] 错误处理是否一致？
- [ ] 消息格式是否可扩展？

### 可行性评审

- [ ] 技术复杂度是否可接受？
- [ ] 开发周期是否合理？
- [ ] 风险是否已识别？
- [ ] 是否有备选方案？

---

## 下一步

Phase 0 完成后，进入 **Phase 1: 核心抽象层**

Phase 1 任务清单:
1. 创建 `crates/torque-extension` crate
2. 定义 ID 类型 (`ExtensionId`, `ExtensionVersion`)
3. 定义 `ExtensionActor` trait
4. 定义 `ExtensionContext`
5. 定义消息类型
6. 定义错误类型
7. 编写单元测试
