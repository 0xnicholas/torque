# Concepts

Torque 当前的定位是：

- 一个 **Agent Runtime Kernel**
- 一个构建在其上的 **Agent Harness**
- 在 harness 层提供 **Agent Team** 能力

它不是一个绑定特定产品 DSL 的 workflow engine，也不应该假设自己拥有内建 workspace 模型。

## Agent Runtime

Agent runtime 解决的是生产环境里的执行语义问题：

- long-running
- streaming
- persistence
- human-in-the-loop
- low-level control
- recovery and replay

Torque Kernel 的核心不是 graph，而是 `AgentInstance`。

Kernel 一等对象：

- `AgentDefinition`
- `AgentInstance`
- `ExecutionRequest`
- `Task`
- `Artifact`
- `Event`
- `Checkpoint`
- `ApprovalRequest`
- `MemoryWriteCandidate`
- `ExternalContextRef`

适合使用 Torque Kernel 的场景：

- 需要 durable execution 的长生命周期 agent
- 需要显式 checkpoint / replay / resume
- 需要严格 delegation、approval、tool mediation
- 需要把上层 orchestration 和底层 execution 分开

## Agent Harness

Harness 是在 runtime 之上的开箱即用层，用来提供更完整的 agent 构建体验。

```text
Agent Harness
= Loop Engine
+ Tool Interface
+ Memory System
+ Built-in Capabilities
+ Collaboration Layer
```

Torque Harness 负责的不是底层持久化本身，而是更高层的使用模型，例如：

- planning and decomposition
- subagent delegation
- context engineering
- file / artifact oriented working model
- team orchestration

Harness 可以内置工具和能力，但这些能力最终都要 lower 到 kernel-level runtime objects。

## Execution Model

Torque 当前采用 instance-centric execution model。

核心流程：

1. `Instantiate`
2. `Hydrate`
3. `Deliberate`
4. `Act`
5. `Checkpoint`
6. `Publish`
7. `Suspend / Resume / Complete / Fail`

关键判断：

- `AgentInstance` 是执行中心
- `Task` 是 instance 正在推进的工作单元
- `ExecutionRequest` 是标准入口
- 上层系统不应直接把自己的 DSL 强塞给 kernel

## Memory and Continuity

Torque 把连续性拆成几个不同层次：

- **短期连续性**：`Checkpoint + Event + Instance State`
- **长期任务连续性**：`Artifact + Published Progress Snapshot`
- **跨会话语义连续性**：`Memory Plane`
- **外部协作连续性**：`ExternalContextRef + Published Artifact`

### Memory Plane

长期记忆不等于原始消息历史。

建议分层：

- `agent_profile_memory`
- `user_preference_memory`
- `task_or_domain_memory`
- `external_context_memory`

长期记忆写入先产生 `MemoryWriteCandidate`，再按策略决定是否落向量库或其他 memory backend。

向量记忆是检索层，不是真相源。

### Artifact Plane

Artifact 是精确执行产物，例如：

- 结构化结果
- 草稿
- 研究笔记
- 工具输出快照
- 文件对象
- task progress snapshot

默认是私有作用域，只有显式 publish/promote 后才进入共享可见域。

### External Context Plane

Torque 不拥有 workspace domain model。

外部上下文通过 `ExternalContextRef` 挂接，例如：

- repo
- kb
- ticket/project
- file space
- conversation thread
- 上层系统自己的 workspace/container

## Agent Team

Agent Team 是 Harness 层的一等能力，不是 Kernel 层的一等能力。

推荐建模：

```text
AgentTeam
= Governance Layer
+ Supervisor-Orchestrated Delegation
+ Capability Agents
+ Shared Task State
+ Event Log
```

正确的默认协作方式是：

`Supervisor -> Subagent`

而不是自由 peer network。

适合用 Team 的场景：

- 任务需要多个专业 agent
- 单 agent 上下文窗口会被快速打满
- 希望把能力边界、上下文边界和责任边界拆开

### Team Layer

团队级对象建议包括：

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`

其中：

- `TeamDefinition` 定义角色、治理规则、delegation policy、可用 mode
- `TeamInstance` 管理一次团队执行
- `TeamTask` 是团队级工作单元，随后被 supervisor lower 成 kernel-level tasks and delegations

### Team Modes

Harness 层可以提供一些常见协作模式：

- `coordinate`
- `route`
- `broadcast`
- `tasks`

这些是 orchestration strategies，不是 kernel primitives。

### Shared State

团队共享状态应保持很小，只存协调需要的事实：

- accepted artifact refs
- published facts
- active delegations
- decision log
- blockers
- approvals

共享状态不是：

- 全量消息历史
- 全量工具输出
- 向量记忆仓库
- 默认共享 workspace

## Delegation

Delegation 必须是显式契约，不是“把消息发给另一个 agent”。

标准模型：

`Supervisor AgentInstance`
-> `DelegationRequest`
-> `Child AgentInstance`
-> `DelegationResult`

默认策略应该保守：

- child 看不到 parent 全量上下文
- child 只拿到显式传入的 artifacts 和 external context refs
- child 结果默认仍是 private
- supervisor 接受后再 publish/promote

## Recovery Model

Torque 的恢复模型应是：

- `Event Log` 作为事实真相源
- `Checkpoint` 作为恢复加速层
- `Replay` 作为重建机制

推荐模型：

- 从最新 checkpoint hydrate
- replay checkpoint 之后的 event tail
- 对外部副作用记录 effect metadata 和 idempotency 信息
- time travel 通过新 lineage 分叉，而不是覆盖历史

## Capabilities

Capability 可以看成三层：

1. `Primitive Capability`
   tools + routines
2. `Composite Capability`
   reusable higher-level skills
3. `Orchestrated Capability`
   harness-level orchestration patterns built from agents and delegation

Capability contract 需要定义：

- input/output contract
- tool boundaries
- risk level
- policy constraints

## Upper-Layer Integration

Torque 应暴露的是标准运行时边界，而不是要求上层系统采用某种固定 DSL。

上层系统应该把自己的 orchestration lower 成：

- `ExecutionRequest`
- `DelegationRequest`
- `Artifact`
- `ApprovalRequest`
- `ExternalContextRef`

这样 Torque 才能同时服务多种上层系统，而不被某个特定 playbook/workflow/workspace 模型绑定。






---
# Torque CLI
torque_cli
