# AGENTS.md "除草" 分析报告

## 概述

分析日期: 2026-04-30  
目标文件: `/Users/nicholasl/Documents/build-whatever/torque/AGENTS.md`  
代码库状态: `v0.1.1` 已提交并推送

---

## 1. 当前代码库结构 vs AGENTS.md 描述

### AGENTS.md 声称的 Crate 结构 (过时)

```
crates/llm              # OpenAI-compatible client
crates/torque-harness  # lightweight session agent prototype  
crates/checkpointer     # emerging checkpoint abstraction
```

### 实际代码库结构

```
crates/
├── torque-kernel/      # Kernel 定义: AgentInstance, Task, ExecutionRequest, DelegationRequest, etc.
├── torque-harness/     # Harness 层: API, Service, Repository, Team, Policy
├── torque-runtime/     # Runtime 实现: Environment, Checkpoint, Host, VFS, Events
└── llm/                # LLM client
```

**关键差异**:
- `checkpointer` 已合并到 `torque-runtime`
- 新增 `torque-kernel` (核心运行时契约)
- 新增 `torque-runtime` (持久化和适配层)

---

## 2. 过时内容清单

### 2.1 "Current Repo State" 章节 (第 35-56 行)

**问题**: 描述的三个 crate 和当前实际结构严重不符

**需要删除/替换的内容**:
- 第 40-46 行: 整个 "Current codebase" 描述块
- 第 48 行: `docs/superpowers/specs/` 引用路径验证
- 第 55 行: 关于 "older DAG/planner model" 的警告 (已不再适用)

**建议替换为**:
```
The repository contains four production crates:

- `crates/torque-kernel`
  Core execution contracts: AgentInstance, Task, ExecutionRequest, Event, Checkpoint, DelegationRequest
  
- `crates/torque-runtime`
  Runtime implementation: Environment, Host, VFS, Checkpoint persistence, Event storage
  
- `crates/torque-harness`
  Harness layer: API handlers, Service orchestration, Repository persistence, Team, Policy
  
- `crates/llm`
  OpenAI-compatible LLM client with streaming support
```

### 2.2 缺少 `torque-runtime` 的任何描述

**问题**: `torque-runtime` 是当前架构的重要组成部分，但 AGENTS.md 完全没有提及

**关键模块** (来自 `crates/torque-runtime/src/`):
- `environment.rs` - 执行环境封装
- `checkpoint.rs` - Checkpoint 持久化实现
- `host.rs` - Host 抽象
- `events.rs` - Event 存储
- `message.rs` - 消息处理
- `vfs.rs` - 虚拟文件系统抽象
- `offload.rs` - 卸载策略
- `tools.rs` - Tool 基础设施

**建议**: 在 Core Architecture 或新增一个 Runtime Implementation 小节描述

### 2.3 "Key Invariants" 部分可能需要更新

**第 4 点 "Capability is not implementation"**:
- 描述的概念 (CapabilityRef, CapabilityProfile, CapabilityRegistryBinding, CapabilityResolution, AgentDefinition) 仍然有效
- 但实际代码中这些概念的边界和命名可能已有变化
- 建议添加: 验证 CapabilityResolution 实际类型名称

**第 5 点 "Policy is evaluated governance"**:
- 代码库已有 `torque-harness/src/policy/` 模块
- 包含 `decision.rs`, `evaluator.rs`, `mod.rs`
- 概念仍然有效，但可能需要引用实际模块路径

### 2.4 "Execution and Delegation Rules" (第 180-206 行)

**现状**: 概念描述仍然准确
- `Task` vs `DelegationRequest` 的区分仍然有效
- `DelegationResult` 不自动接受的概念仍然正确
- Parent-side handling 步骤仍然适用

**潜在更新**: 可能需要引用实际代码中的类型名称验证

---

## 3. 仍然有效的部分

### 3.1 Project Positioning (第 17-31 行)

✅ 仍然准确:
- Agent Runtime Kernel 定位
- Harness 作为上层抽象
- 明确的非目标 (DAG-first, workflow engine, chat history centric)

### 3.2 Core Architecture 分层 (第 59-81 行)

✅ 概念层划分仍然有效:
1. Kernel Execution
2. Capability Layer  
3. Policy Layer
4. Context and State Layer
5. Harness / Team Layer
6. Recovery Layer

### 3.3 Authoritative Specs 引用 (第 85-99 行)

✅ 规格文档列表完整且准确

### 3.4 所有 Key Invariants (第 103-176 行)

✅ 7 条不变式仍然有效且重要

### 3.5 Implementation Guidance (第 209-223 行)

✅ 仍然适用

### 3.6 Working Notes For Agents (第 227-243 行)

✅ 仍然有效，路径引用可能需要更新

---

## 4. 代码库中已实现但 AGENTS.md 未提及

### 4.1 torque-kernel 核心类型

| 类型 | 文件 | 状态 |
|------|------|------|
| `AgentInstance` | `torque-kernel/src/agent_instance.rs` | ✅ 实现 |
| `AgentDefinition` | `torque-kernel/src/agent_definition.rs` | ✅ 实现 |
| `Task` | `torque-kernel/src/task.rs` | ✅ 实现 |
| `DelegationRequest` | `torque-kernel/src/execution.rs` | ✅ 实现 |
| `DelegationResult` | `torque-kernel/src/execution.rs` | ✅ 实现 |
| `Checkpoint` | `torque-kernel/src/recovery.rs` | ✅ 实现 |
| `Event` | `torque-runtime/src/events.rs` | ✅ 实现 |
| `TaskPacket` | `torque-kernel/src/task_packet.rs` | ✅ 实现 |
| `ExternalContextRef` | `torque-kernel/src/context_ref.rs` | ✅ 实现 |

### 4.2 torque-harness 已实现功能

| 功能 | 位置 | AGENTS.md 描述 |
|------|------|----------------|
| Capability Registry | `service/capability.rs` | ✅ 提及 |
| Policy Evaluator | `policy/evaluator.rs` | ✅ 概念提及 |
| Team Supervisor | `service/team/supervisor.rs` | ✅ 提及 |
| SharedTaskState | `repository/team/shared_task_state.rs` | ✅ 提及 |
| Checkpoint Recovery | `service/recovery.rs` | ✅ 提及 |
| Event Replay | `service/event_replay.rs` | ⚠️ 未明确描述 |

---

## 5. 推荐修改行动

### 优先级 P0 (必须修改)

- [ ] 更新 "Current Repo State" 章节，替换为正确的 crate 描述
- [ ] 添加 `torque-runtime` 的描述
- [ ] 验证并更新 spec 文件引用路径

### 优先级 P1 (强烈建议)

- [ ] 验证 "Key Invariants" 中引用的类型名称与实际代码一致
- [ ] 在 Core Architecture 中标注各层对应的实际 crate
- [ ] 添加 torque-runtime 在 Runtime/Environment 层的描述

### 优先级 P2 (可选优化)

- [ ] 更新 Working Notes 中的路径引用
- [ ] 添加代码库成熟度标注 (哪些模块是 prototype, 哪些是 production-ready)

---

## 6. 验证清单

修改完成后需验证:

- [ ] Spec 文档路径全部可访问
- [ ] 引用的所有类型名称在代码中存在
- [ ] Crate 描述与 Cargo.toml workspace.members 一致
- [ ] Layer 到 crate 的映射准确

---

## 7. 结论

AGENTS.md 的**核心架构理念和不变式**仍然完全有效，不需要大幅改动。

主要问题集中在:
1. **过时的基础设施描述** (crate 结构)
2. **缺失的新 crate 描述** (`torque-runtime`)
3. **轻微的路径/命名验证需求**

建议采用**增量修复**策略，保留现有结构和理念，仅更新过时的基础设施描述部分。
