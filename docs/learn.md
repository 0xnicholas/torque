# Torque Concepts

`docs/learn.md` 现在作为 Torque 的高层概念导航页使用。

它回答两个问题：

- Torque 总体上是什么
- 关键概念分别在哪份权威 spec 中定义

如果要看细节，优先跳到对应 spec，而不是把这份文件当成完整设计文档。

---

## Overall Positioning

Torque 当前的定位是：

- 一个 **Agent Runtime Kernel**
- 一个构建在其上的 **Agent Harness**
- 在 harness 层提供 **Agent Team**、能力解析、上下文状态管理等更高层能力

它不是：

- 绑定某个产品 DSL 的 workflow engine
- 内建 workspace domain model
- 以 graph 或 playbook 作为 kernel 核心抽象的系统

Kernel 的核心是 `AgentInstance`，不是 graph。

---

## Core Layering

建议把 Torque 理解成 5 层：

1. **Kernel Execution**
   定义执行入口、实例、任务、delegation、approval、checkpoint 等运行时契约。

2. **Capability Layer**
   定义“能力是什么”“如何引用能力”“能力如何解析成候选实现”。

3. **Policy Layer**
   定义 approval、delegation、visibility、resource、memory、tool 等治理规则如何求值和合并。

4. **Context and State Layer**
   定义上下文分层、`TaskPacket`、shared state slicing、lazy loading、三平面边界。

5. **Harness / Team Layer**
   定义 team、shared task state、selector resolution、publish、team approval、team recovery 等协作层语义。

---

## Kernel

Kernel 解决的是生产环境里的执行语义问题：

- long-running execution
- streaming
- persistence
- approval / human-in-the-loop
- delegation
- replay and recovery

Kernel 一等对象包括：

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

权威文档：

- [Torque Agent Runtime / Harness Design](./superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md)
- [Torque Kernel Execution Contract Design](./superpowers/specs/2026-04-08-torque-kernel-execution-contract-design.md)

---

## Capability

Capability 需要拆成几层看：

- `CapabilityRef`
  上层 authoring / orchestration 使用的轻量引用句柄
- `CapabilityProfile`
  canonical ability contract
- `CapabilityRegistryBinding`
  capability 到候选 `AgentDefinition` 的实现绑定
- `CapabilityResolution`
  运行时在当前约束下解析出的候选集合

这一层的目标是：

- 让上层引用“能力”，而不是直接引用具体 agent 实现
- 让能力定义和实现绑定解耦
- 让运行时 resolution 成为显式对象

权威文档：

- [Torque Capability Registry Model Design](./superpowers/specs/2026-04-08-torque-capability-registry-model-design.md)

---

## Policy

Torque 不应把 policy 仅仅建模成分散的配置块。

推荐模型是：

`policy inputs -> dimensional evaluation -> conservative merge -> PolicyDecision`

关键点：

- policy 按维度求值
- 同维度默认“最严格者优先”
- 不同维度不互相偷权
- `PolicyDecision` 是结构化治理结果，不直接执行业务动作

初始维度包括：

- `approval`
- `visibility`
- `delegation`
- `resource`
- `memory`
- `tool`

权威文档：

- [Torque Policy Model Design](./superpowers/specs/2026-04-08-torque-policy-model-design.md)

---

## Context and State

Torque 应把“上下文”从聊天历史中心，改造成分层状态系统。

推荐原则：

- 不共享完整 transcript
- 共享最小任务态和必要引用
- 结构化状态优先于长自然语言历史
- lazy context loading 默认开启
- 周期性 state convergence / compaction

关键对象和层次：

- global stable layer
- team coordination layer
- agent-instance private layer
- external knowledge layer
- execution-time `TaskPacket`

`TaskPacket` 的定位是：

- 派生执行包
- 窄输入视图
- 由 assigning authority 组装
- 不是新的权威状态对象

权威文档：

- [Torque Context State Model Design](./superpowers/specs/2026-04-08-torque-context-state-model-design.md)

---

## Context Planes

Torque 还明确区分三个上下文相关平面：

- `ExternalContextRef`
  外部引用平面
- `Artifact`
  执行结果平面
- `Memory`
  语义沉淀平面

关键规则：

- external context 不自动变 artifact
- artifact 不自动变 memory
- team publish 不自动变 memory write
- 三平面之间的转换必须显式且受 policy 管控

权威文档：

- [Torque Context Planes Design](./superpowers/specs/2026-04-08-torque-context-planes-design.md)

---

## Team

`Team` 是 harness 层的一等能力，不是 kernel 层的一等对象。

推荐默认协作方式：

`Supervisor -> Subagent`

而不是自由 peer network。

团队层关键对象：

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- `SharedTaskState`
- `TeamEvent`

团队层负责：

- triage and mode selection
- selector-governed dynamic expansion
- shared-state publish
- approval routing
- collaboration-level recovery

权威文档：

- [Torque Agent Team Design](./superpowers/specs/2026-04-08-torque-agent-team-design.md)

---

## Recovery

Torque 的恢复哲学应该保持一致：

- `Event` 是 truth source
- `Checkpoint` 是 recovery acceleration layer
- `Recovery` = restore + replay + reconciliation

这条规则适用于：

- kernel instance recovery
- team recovery
- context/state recovery

权威文档：

- [Torque Recovery Core Design](./superpowers/specs/2026-04-08-torque-recovery-core-design.md)

---

## How To Read The Docs

如果你是从零开始理解 Torque，建议按这个顺序读：

1. [Torque Agent Runtime / Harness Design](./superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md)
2. [Torque Kernel Execution Contract Design](./superpowers/specs/2026-04-08-torque-kernel-execution-contract-design.md)
3. [Torque Context State Model Design](./superpowers/specs/2026-04-08-torque-context-state-model-design.md)
4. [Torque Context Planes Design](./superpowers/specs/2026-04-08-torque-context-planes-design.md)
5. [Torque Capability Registry Model Design](./superpowers/specs/2026-04-08-torque-capability-registry-model-design.md)
6. [Torque Policy Model Design](./superpowers/specs/2026-04-08-torque-policy-model-design.md)
7. [Torque Agent Team Design](./superpowers/specs/2026-04-08-torque-agent-team-design.md)
8. [Torque Recovery Core Design](./superpowers/specs/2026-04-08-torque-recovery-core-design.md)

---

## One-Line Summary

Torque 可以概括成：

一个以 `AgentInstance` 为执行中心的 runtime kernel，加上一层 harness，用能力、政策、上下文状态管理和 team 协作来把上层系统 lower 成标准运行时边界，而不是把某个特定 workflow/playbook/workspace 模型硬塞进 kernel。
