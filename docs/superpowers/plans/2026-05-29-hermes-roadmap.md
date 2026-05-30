# Hermes 路线图

本路线图用于将 DeepSeekX 从一个“受监督的任务执行器”演进为一个接近 Hermes 级别的自治 Agent 运行时。

本文档以 Hermes Agent 公开文档中描述的能力支柱为目标形态：

- 跨会话、跨项目的持久记忆
- 具备 Skill 创建与自我改进能力的闭环学习机制
- 支持定时任务与无人值守执行
- 可将复杂任务委派给隔离子代理并行处理
- 将完整浏览器 / Web 控制作为一等运行时能力
- 具备更强的执行安全性、回滚能力与环境隔离能力

参考资料：

- Hermes 功能总览: https://hermes-agent.nousresearch.com/docs/user-guide/features/overview/
- Hermes 文档首页: https://hermes-agent.nousresearch.com/docs/
- Hermes 官网首页: https://hermes-agent.nousresearch.com/

## 北极星目标

DeepSeekX 应该能够接收一个高层级的编码或调研目标，在多个会话中持续规划和执行它，复用先前知识，从失败中恢复，仅在策略要求时请求用户批准，并通过浏览器、终端、文件、MCP 与委派子代理持续推进任务，而不是假装自己拥有并未真正接入的能力。

## 当前状态

DeepSeekX 目前已经具备：

- 一个有边界的 Agent Loop，以及结构化动作与 observation
- MCP 发现与工具调用
- 覆盖工作区、网络与用户知识库的检索能力
- 结构化编辑、受保护命令执行与验证检查
- 对阻塞动作的批准 / 拒绝 / 恢复执行流程
- 面向项目的 skills、MCP servers 与 retrieval sources 配置

DeepSeekX 目前仍然缺少 Hermes 级 Agent 的关键层：

- 除 session 历史之外，没有真正的持久记忆系统
- 没有从经验中提炼 skill、并持续优化 skill 的自主学习闭环
- 没有内建的定时任务运行时
- 没有原生的子代理委派模型
- 浏览器与桌面能力仍更接近“配置提示”，不是原生一等 Agent Action
- 没有在编辑前提供 checkpoint / rollback 安全网
- 没有面向长时任务的远程 / 隔离执行环境抽象
- 工具运行时稳定性尚不足以支持“无人值守自治”（当前 `cargo test` 里 MCP 测试仍失败）

## 发布标准

在以下条件全部满足之前，不应把 DeepSeekX 称为 “Hermes 级” Agent：

1. Agent 能跨会话、跨项目保存并召回结构化知识。
2. Agent 能从已完成任务中提炼可复用的程序性知识，并随时间持续改进。
3. Agent 能按计划在后台执行任务，并支持结果投递、暂停、恢复与编辑。
4. Agent 能生成隔离的委派 worker，在受限工具集合内执行子任务，并把结果合并回来。
5. 浏览器自动化是原生执行路径，而不是仅靠 prompt 中的提示文本。
6. 高风险文件编辑与命令执行有 checkpoint / rollback 保护和可审计轨迹。
7. 产品的主执行路径已经统一，旧的 fallback 路径不会绕过主工具循环。
8. 核心运行时测试全部为绿色，包括 MCP 执行稳定性。

## Phase 0：先把现有 Runtime 稳住

目标：让当前 Agent 足够可信，可以作为后续自治能力的底座。

### 工作项

- [ ] 修复 [src-tauri/src/agent/mcp.rs](/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/mcp.rs) 中 MCP 发现与工具调用相关测试失败的问题。
- [ ] 将产品收敛到一个主执行路径上；逐步废弃 [src-tauri/src/lib.rs](/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs) 中仅走 CLI 的旧 `send_message` 流程。
- [ ] 将 Loop 预算改成按项目和任务类型可配置，而不是固定写死 `12` 步 / `180s`。
- [ ] 引入结构化失败分类：planner error、tool transport error、permission stop、environment failure、verification failure。
- [ ] 持久化完整的 Agent Run 状态，以支持真正的断点恢复，而不是只在批准后基于 turn 级别信息重放。

### 退出标准

- `cargo test --manifest-path src-tauri/Cargo.toml` 全绿。
- Agent 与普通 Chat 共用同一套工具执行内核。
- 长任务可通过配置获得更大的预算，而无需修改代码。

## Phase 1：持久记忆

目标：从“聊天记录回放”升级到“有边界、可查询、跨会话”的记忆系统。

### 工作项

- [ ] 增加 memory store，至少支持以下记录类型：
  - facts
  - decisions
  - preferences
  - project context
  - open threads / commitments
  - successful procedures
- [ ] 将 memory 与原始 session log 分离；memory 必须是整理后的知识，而不是追加式 transcript 回放。
- [ ] 在 memory 上增加全文检索 + 模型摘要排序能力。
- [ ] 增加明确的 memory 写入策略：哪些可以自动保存、哪些需要确认、哪些会过期。
- [ ] 支持 project-scoped 与 global 两级 memory namespace。
- [ ] 将 memory 操作暴露给 planner，作为一等 action，例如 `search_memory`、`write_memory`、`update_memory`、`forget_memory`。

### 建议实现形态

- 使用 SQLite FTS5 作为本地 memory index
- 在 `src-tauri/src/memory/` 下建立规范 memory schema
- 增加 memory compaction job，对重复项做合并，对老旧项做摘要整理

### 退出标准

- 新会话不需要重放整段旧对话，也能恢复重要项目事实。
- Agent 能跨会话记住架构决策与用户偏好。

## Phase 2：学习闭环与 Skill 自动沉淀

目标：让 Agent 能把成功任务轨迹转化为可复用的程序性记忆。

### 工作项

- [ ] 定义一个 post-task reflection pipeline，在任务成功或失败后运行。
- [ ] 提取候选经验：
  - 哪些做法成功了
  - 哪些做法失败了
  - 依赖了哪些环境前提
  - 哪些命令序列可复用
  - 哪些代码库模式值得沉淀
- [ ] 从重复出现的成功流程中自动生成 draft skill。
- [ ] 在 skill 激活前增加质量门槛：
  - confidence threshold
  - duplicate detection
  - global skill 需要人工 review
  - project-local skill 可选自动启用
- [ ] 记录 skill 的使用效果，并支持根据后续运行结果对 skill 自我改进。
- [ ] 区分 user-authored skills 与 learned skills。

### 建议新增运行时能力

- `reflect_on_run`
- `propose_skill`
- `update_skill_from_feedback`
- `activate_skill`

### 退出标准

- 当 Agent 多次完成相似任务后，会优先使用学到的流程，而不是每次重新摸索。
- Skill 的新增与更新都是可审计、可回滚的。

## Phase 3：定时任务与无人值守执行

目标：支持 Hermes 风格的 cron / automation 能力。

### 工作项

- [ ] 增加 scheduler service 及其持久化模型，用于保存 recurring tasks。
- [ ] 支持：
  - 一次性延迟执行
  - cron 风格循环执行
  - pause / resume
  - edit / update
  - result delivery target
- [ ] 允许定时任务绑定 project、memory scope、skills、MCP set 与 retrieval sources。
- [ ] 增加后台执行状态 UI：
  - queued
  - running
  - succeeded
  - failed
  - needs_confirmation
- [ ] 为已完成任务增加应用内通知投递。

### 退出标准

- 用户无需保持窗口前台打开，也能安排调研简报、维护检查或代码审计任务定时运行。

## Phase 4：原生浏览器与桌面执行能力

目标：把浏览器 / 桌面能力从“配置提示”升级成真正的一等 Agent 能力。

### 工作项

- [ ] 为浏览器工作流增加原生 agent actions：
  - `open_page`
  - `click_element`
  - `type_text`
  - `extract_dom`
  - `take_screenshot`
  - `run_browser_script`
- [ ] 在对应运行时存在时，为桌面工作流增加原生 agent actions。
- [ ] 将 capability detection 从“关键字推断”改成“基于真实 runtime / toolset 的检测”。
- [ ] 为浏览器 observation 增加结构化数据：当前 URL、选中元素、可见文本片段、截图路径。
- [ ] 增加浏览器任务恢复逻辑，覆盖导航失败、selector 失败、需要登录、被限流等场景。

### 退出标准

- Planner 能像依赖文件动作和命令动作一样，稳定依赖浏览器动作。
- 运行时不再需要通过 prompt 文本提醒模型“不要假装自己打开过页面”。

## Phase 5：委派子代理

目标：为复杂任务提供并行、隔离的 worker。

### 工作项

- [ ] 增加 `delegate_task` action，包含：
  - subgoal
  - bounded toolset
  - bounded workspace 或 worktree
  - memory visibility rules
  - return contract
- [ ] 至少支持两种隔离模式：
  - 同一工作区，但工具受限
  - 独立 worktree / 临时工作区
- [ ] 增加 parent-child observation graph 与 merge summary。
- [ ] 限制 fan-out 与总 token / step budget，避免无限扩散。
- [ ] 为以下场景增加委派启发式：
  - 并行代码搜索
  - 多源调研
  - 测试矩阵验证

### 退出标准

- 主 Agent 能把一个大任务拆成多个有边界的子任务，并确定性地合并结果。

## Phase 6：安全、Checkpoint 与恢复

目标：让更强自治能力在可控范围内足够安全。

### 工作项

- [ ] 在 destructive 或大规模编辑前自动创建 pre-edit checkpoint。
- [ ] 在 UI 与 planner 中增加 rollback 支持。
- [ ] 增加策略档位：
  - conservative
  - balanced
  - autonomous
- [ ] 为每个项目增加 trust settings，用于控制命令类别、编辑规模、网络访问与 MCP scopes。
- [ ] 增加审计日志，覆盖：
  - tool invocation
  - approvals
  - memory writes
  - skill creation
  - delegated runs

### 退出标准

- 用户可以检查、回放、或回滚高影响自治动作。

## Phase 7：远程与长时运行环境

目标：摆脱“Agent 只能依附本地桌面进程”的限制。

### 工作项

- [ ] 抽象执行后端：
  - local
  - remote SSH
  - container
  - ephemeral cloud runner
- [ ] 按项目持久化环境描述符。
- [ ] 允许后台任务与委派任务运行在选定后端中。
- [ ] 增加长任务 heartbeat / reconnect 能力。

### 退出标准

- 即使桌面应用不在前台，Agent 仍能持续推进无人值守任务。

## 建议实施顺序

1. Phase 0：先稳住当前 runtime
2. Phase 1：持久记忆
3. Phase 6：安全与回滚
4. Phase 3：定时任务
5. Phase 4：原生浏览器 / 桌面执行
6. Phase 5：子代理委派
7. Phase 2：学习闭环与 skill 自动沉淀
8. Phase 7：远程执行环境

原因：

- 如果当前 runtime 还不稳，后续能力都建立在不可靠基础上。
- 如果没有 memory 和 safety，自治能力会脆弱且风险高。
- 定时任务与原生浏览器能力会率先解锁真正的无人值守工作流。
- 子代理与 skill 自动沉淀应建立在一个已经可信的基础 Agent 上。

## 实际里程碑

### M1：可信的 Agent Core

- 全绿的 runtime tests
- 统一的工具执行 loop
- 可配置的任务预算
- checkpoint 与 rollback

### M2：跨会话 Agent

- 可搜索的持久记忆
- 项目级 / 全局级记忆隔离
- planner 可直接使用 memory actions

### M3：无人值守 Worker

- scheduler
- 通知系统
- policy profiles
- 后台执行状态面板

### M4：真正接近自治

- 原生浏览器 actions
- 委派子代理
- 基于反思自动沉淀 learned skills

## 对 DeepSeekX 而言，什么才叫“达到 Hermes 标准”

对这个产品来说，“达到 Hermes 标准”至少应该意味着：

- 它不只是“会调工具”，而是“具备连续性”
- 它不只是记住这次聊天，而是记住整个项目
- 它不只是会改文件，而是能安全地从改动中恢复
- 它不只是会回答任务，而是能持续推进任务
- 它不只是会调用 MCP，而是能编排多个有边界的 worker
- 它不只是会加载 skills，而是能从经验中学出新的 skills

在这些条件都满足之前，DeepSeekX 更准确的描述仍然应该是：一个能力较强、但仍需要监督的编码 / 调研 Agent，而不是 Hermes 级自治 Agent。
