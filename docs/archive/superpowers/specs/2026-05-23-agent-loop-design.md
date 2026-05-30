# DeepSeekX 应用内自动规划 Agent 设计

> 日期：2026-05-23
> 状态：Archived
> 主题：应用内多步执行 Agent Loop

---

## 1. 背景

当前 DeepSeekX 的 GUI 主链路以 `send_message_via_api()` 为核心，主要能力是：

- 接收用户输入
- 拼装 system prompt
- 调用 DeepSeek Chat Completions API
- 将最终回复写入 session
- 从回复中解析 `file:` 代码块并写入工作区

这条链路适合“单轮问答”或“单次代码生成”，但不适合“自动规划直到完成”的任务型执行。当前缺口主要包括：

- 没有明确的任务状态机
- 没有结构化动作协议
- 没有应用侧的执行闭环
- 没有基于风险等级的权限门
- 没有面向 agent 的中间步骤展示

因此，若希望用户在应用内提出目标后，系统可以自动拆解任务、逐步执行、观察结果、继续推进直到收敛，就需要在现有聊天链路上新增一个受控的应用侧 agent loop。

---

## 2. 目标

本方案目标是在 DeepSeekX 中新增一个可手动开启的应用内 agent，使其具备以下能力：

1. 根据用户目标自动生成执行计划
2. 按步骤读取文件、修改代码、运行命令
3. 根据每一步的 observation 自动调整后续计划
4. 在任务完成、阻塞或需要确认时稳定收敛
5. 将过程以结构化步骤显示在当前对话中

同时满足以下产品约束：

- 普通聊天保持原样，不默认进入 agent 模式
- 用户可在发送时显式选择“自动执行”
- 默认采用分级授权策略
- 以“目标达成优先”作为结束标准，而不是必须测试全绿
- 第一版只支持单任务闭环，不做长期后台常驻 agent

---

## 3. 非目标

第一版不解决以下问题：

- 多任务并行调度
- 跨会话长期运行与后台守护
- 浏览器自动化、桌面自动化、远程机器操作
- 大规模 patch 规划和多分支协作
- 自动提交 git、自动推送远端、自动发布
- 自定义动作 marketplace 或插件系统

第一版只保证：一个用户目标，进入一个受控 loop，在有限步数内完成、阻塞或请求确认。

---

## 4. 方案概述

整体采用应用侧 `Plan -> Act -> Observe -> Replan -> Finish` 循环。

### 4.1 入口方式

前端在现有聊天输入区增加一个 `Agent` 或 `自动执行` 入口。

- 普通发送：继续走现有 `send_message_via_api()`
- Agent 发送：走新的 `run_agent_task()` 命令

### 4.2 核心循环

Agent 模式下，每一轮模型只输出一个结构化动作，应用完成执行后再把结果作为 observation 回填给模型，进入下一轮。

循环直到：

- 任务完成
- 任务阻塞
- 需要用户确认
- 达到最大步数或超时

### 4.3 设计原则

- 模型负责规划与决策
- 应用负责执行、权限控制、状态管理和收敛
- 中间步骤结构化记录
- 最终回复与中间过程分离

---

## 5. 高层架构

```text
User Input
  |
  v
Agent Mode Toggle
  |
  +--> off --> send_message_via_api --> Final Response
  |
  +--> on --> run_agent_task
               |
               v
         AgentOrchestrator
               |
      +--------+---------+
      |                  |
      v                  v
 PermissionGate     AgentStateStore
      |                  |
      v                  |
 ActionExecutor <--------+
      |
      v
 Observation Builder
      |
      v
 Agent Planner Completion
      |
      +--> next action
      +--> ask_user
      +--> finish
```

---

## 6. 核心模块

### 6.1 AgentOrchestrator

职责：

- 创建 agent task 上下文
- 驱动主循环
- 控制最大步数、超时、失败阈值
- 组织 planner 输入与 observation
- 产出最终结果

它是第一版的核心入口，建议由 `src-tauri/src/lib.rs` 中新增的 Tauri command 调用。

### 6.2 PermissionGate

职责：

- 判断动作是否允许自动执行
- 根据风险级别决定放行、拦截或转为确认
- 阻止越权写文件、危险命令、外部副作用操作

它不关心模型为什么做这个动作，只负责判断“能不能执行”。

### 6.3 ActionExecutor

职责：

- 读取文件
- 列目录或检索路径
- 写入工作区文件
- 运行受控命令
- 生成统一格式的执行结果

它是所有副作用的统一出口，避免在 orchestrator 中散落文件和命令逻辑。

### 6.4 AgentStateStore

职责：

- 保存任务目标
- 保存当前计划摘要
- 保存已执行动作
- 保存 observation 和失败原因
- 保存待确认动作

第一版可以先用内存结构驱动一次请求，并将摘要同步到 session；如后续需要恢复执行，再扩展为持久化状态。

---

## 7. 动作协议设计

第一版只支持有限动作集，每轮只允许一个动作。

### 7.1 动作列表

1. `list_files`
2. `read_files`
3. `write_files`
4. `run_command`
5. `ask_user`
6. `finish`

### 7.2 统一输出格式

Planner 强制输出 JSON，示例：

```json
{
  "action": "read_files",
  "reason": "需要查看入口文件和现有数据结构",
  "files": ["src/App.tsx", "src/components/ChatPanel.tsx"]
}
```

```json
{
  "action": "run_command",
  "reason": "验证变更是否可编译",
  "command": "cargo test",
  "cwd": "/workspace"
}
```

```json
{
  "action": "finish",
  "reason": "目标已完成",
  "goal_status": "done",
  "summary": "已增加 Agent 模式入口并完成后端 loop 接线",
  "verification": "已通过构建验证"
}
```

### 7.3 为什么单轮只允许一个动作

- 权限判断更直接
- 错误定位更简单
- UI 展示更清晰
- 更容易做重试和恢复
- 减少模型一次性输出复杂计划后失控的概率

---

## 8. 分级授权设计

默认策略采用“高自治 + 安全护栏”。

### 8.1 自动放行

- `list_files`
- `read_files`
- `write_files`，但只允许项目工作区内路径

### 8.2 条件放行

`run_command` 允许执行低风险命令白名单，例如：

- `ls`
- `pwd`
- `cat`
- `npm test`
- `pnpm test`
- `pnpm build`
- `cargo test`
- `cargo build`

### 8.3 强制确认

以下动作必须转为 `ask_user`：

- 删除文件或目录
- 覆盖大量文件
- 包管理器安装全局依赖
- 网络副作用明显的命令
- `git push`
- `rm`
- `sudo`
- 任何超出白名单的 shell 操作

### 8.4 越界保护

应用必须保证：

- 写文件只能落在当前项目工作区内
- 不能通过相对路径跳出工作区
- 命令执行目录必须受控
- 不允许隐式调用危险系统命令

---

## 9. 主循环状态机

建议状态如下：

- `planning`
- `awaiting_approval`
- `executing`
- `observing`
- `completed`
- `blocked`
- `failed`

### 状态转换

```text
planning -> executing
planning -> awaiting_approval
planning -> completed
planning -> failed

executing -> observing
executing -> failed

observing -> planning
observing -> completed
observing -> blocked
```

### 收敛条件

任一条件命中即退出循环：

- 收到 `finish`
- 达到最大步数，例如 12
- 达到总耗时上限，例如 2-5 分钟
- 连续动作执行失败超过阈值
- 模型连续输出非法动作
- 出现必须确认但当前未确认

---

## 10. Planner Prompt 设计

Planner 不直接生成最终自然语言答复，而是输出结构化动作。

要求：

- 只输出 JSON
- 每轮只输出一个动作
- 必须包含 `reason`
- 不允许输出 markdown 包裹
- 不允许输出额外解释文本
- 只能使用受支持动作
- 若任务已足够完成，必须输出 `finish`
- 若动作存在权限风险或需要外部决策，必须输出 `ask_user`

Planner 输入建议包含：

- 用户目标
- 当前任务摘要
- 最近若干步 observation
- 当前工作区信息
- 已完成动作列表
- 当前权限策略
- 剩余步数预算

---

## 11. Observation 设计

每个动作执行后，应用都要生成统一 observation 回填给 Planner。

### 11.1 文件类 observation

包含：

- 读取的文件路径
- 内容摘要或裁剪内容
- 是否成功
- 错误信息

### 11.2 命令类 observation

包含：

- 实际执行的命令
- 工作目录
- exit code
- stdout 摘要
- stderr 摘要
- 是否超时

### 11.3 写文件类 observation

包含：

- 写入文件列表
- 写入类型（create / overwrite)
- 字节数或摘要
- 是否成功

这样 Planner 不需要理解原始复杂结果，只需要消费结构化 observation 做下一轮决策。

---

## 12. 前端交互设计

### 12.1 输入区

在现有 `ChatInput` 中新增 agent 发送入口，建议有两种可选形态：

- 发送按钮旁的 `Agent` toggle
- 独立的 `自动执行` 按钮

第一版推荐用轻量 toggle，避免增加新的输入组件。

### 12.2 消息展示

Agent 模式下，一次任务建议显示为：

- 用户目标
- 当前状态标签
- 可折叠步骤列表
- 最终结果摘要

步骤列表中每一步展示：

- 动作名
- 动作原因
- 输入摘要
- 执行结果摘要
- 是否需要确认

### 12.3 用户确认交互

当动作被 PermissionGate 拦截时，前端展示一个明确的确认卡片：

- 要执行什么
- 为什么需要确认
- 风险等级
- 可选项：允许一次 / 拒绝 / 修改要求

---

## 13. 会话与持久化设计

### 13.1 用户可见最终消息

最终 assistant 文本只保留：

- 结果摘要
- 修改内容
- 验证方式
- 未完成项

不要把原始 JSON、长命令输出、原始文件内容直接拼进最终消息。

### 13.2 中间步骤记录

中间过程建议落到现有 turn 体系中，以 tool step 形式展示：

- `tool_name = agent_plan`
- `tool_name = read_files`
- `tool_name = write_files`
- `tool_name = run_command`
- `tool_name = ask_user`

这样可以复用现有 `Turn` / `ToolCallSummary` / `ToolResultSummary` 结构。

### 13.3 恢复策略

第一版可以不做完整恢复执行，只需要保证一次请求内的状态稳定。

如果未来需要“继续执行”，可以基于 `AgentStateStore` 与 session 中的步骤摘要恢复最近上下文。

---

## 14. 完成判定设计

由于产品要求是“目标达成优先”，所以第一版结束时不要求一定跑过测试，但必须输出可检查的完成结果。

建议 `finish` 动作包含以下字段：

- `goal_status`: `done | blocked | needs_confirmation`
- `summary`
- `what_changed`
- `verification`
- `next_step`

结束语义如下：

- `done`：当前目标已实现，有足够证据说明完成
- `blocked`：当前无法继续推进，明确阻塞原因
- `needs_confirmation`：下一步必须由用户决定

---

## 15. 模块拆分建议

### Rust 后端

- `src-tauri/src/agent/mod.rs`
  - agent 模块入口和公共类型
- `src-tauri/src/agent/orchestrator.rs`
  - 主循环
- `src-tauri/src/agent/planner.rs`
  - planner prompt 与 JSON 解析
- `src-tauri/src/agent/actions.rs`
  - 动作类型定义
- `src-tauri/src/agent/executor.rs`
  - 文件和命令执行
- `src-tauri/src/agent/permissions.rs`
  - 风险判断和分级授权
- `src-tauri/src/agent/state.rs`
  - 任务状态结构
- `src-tauri/src/lib.rs`
  - 新增 `run_agent_task()` Tauri command
- `src-tauri/src/session.rs`
  - 挂接 agent 步骤摘要

### 前端

- `src/components/ChatInput.tsx`
  - 增加 agent 入口
- `src/components/MessageList.tsx`
  - 支持渲染 agent 中间步骤
- `src/components/TurnItem.tsx`
  - 展示 agent 动作和结果摘要
- `src/types.ts`
  - 增加 agent 任务、动作、状态类型
- `src/api.ts`
  - 增加 `runAgentTask()` 调用

---

## 16. 实施顺序

### Phase 1：后端最小闭环

- 定义动作协议
- 实现 planner JSON 解析
- 实现 orchestrator 主循环
- 实现 `list_files` / `read_files` / `write_files`
- 接入 `finish`

### Phase 2：命令执行和权限门

- 实现 `run_command`
- 实现白名单命令策略
- 实现 `ask_user`
- 接入风险分级和越界保护

### Phase 3：前端入口和过程展示

- 输入区增加 agent 入口
- 消息区域增加步骤渲染
- 增加确认交互卡片

### Phase 4：收敛与体验优化

- 最大步数和超时配置
- 更好的完成摘要模板
- 更好的错误提示
- 更好的 observation 裁剪

---

## 17. 测试策略

### 17.1 单元测试

- Planner JSON 解析
- 非法动作拦截
- 工作区路径越界保护
- 权限门白名单判断
- 状态机收敛逻辑

### 17.2 集成测试

- 读取文件后再写文件的最小闭环
- 命令执行成功后 `finish`
- 危险命令转 `ask_user`
- 超过最大步数自动终止
- 模型返回非法动作时失败收敛

### 17.3 回归验证

- “帮我看看这个项目入口文件并整理结构”
- “把这个按钮文案改成英文并验证构建”
- “先分析原因，再修复一个明显报错”

验证重点：

- 是否真的按步骤执行
- 是否会在必要时请求确认
- 是否能在有限步数内收敛
- 最终消息是否有明确完成证据

---

## 18. 风险与取舍

### 风险

- 模型规划质量不稳定，可能走弯路
- 命令执行能力一旦放开，风险显著上升
- 中间步骤过多时，UI 容易变得嘈杂
- 多轮上下文增长会推高 token 成本

### 取舍

- 选择应用侧 loop，是以更高实现复杂度换更强确定性
- 选择有限动作集，是以能力边界换可控性
- 选择手动开启，而非默认开启，是为了避免日常聊天体验变重
- 选择“目标达成优先”，是为了适配没有完整测试体系的项目

---

## 19. 验收标准

- 用户可以在现有聊天输入区显式开启 Agent 模式
- Agent 可以在一次任务内自动规划并执行多个步骤
- Agent 可以读取文件、写文件、运行受控命令
- 危险动作不会直接执行，而是进入确认流程
- 任务能以 `done`、`blocked` 或 `needs_confirmation` 收敛
- UI 能展示中间步骤和最终结果
- 普通聊天链路不受影响

---

## 20. 结论

本方案通过在现有聊天链路之上增加一个受控的应用内 agent loop，使 DeepSeekX 从“单轮问答工具”升级为“可自动规划并逐步执行任务的桌面应用”。

第一版不追求大而全，而是聚焦于：

- 手动开启
- 单任务闭环
- 有限动作集
- 分级授权
- 明确收敛

在这一基础上，后续可以继续扩展：

- 联网搜索动作接入
- 更多工具类型
- 更强的恢复执行能力
- 更丰富的任务视图
