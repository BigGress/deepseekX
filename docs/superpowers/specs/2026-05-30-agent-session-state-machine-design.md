# DeepSeekX Agent 会话级混合状态机设计

> 日期：2026-05-30
> 状态：Phase 1 Implemented
> 主题：为 Agent 引入会话级内部状态机，用于维护阶段状态与结构化中间记忆

---

## 1. 背景

当前 DeepSeekX 的 Agent 主要依赖以下几类运行时信息：

- `goal`
- `observations`
- `plan_summary`
- `report_draft`
- `source_usage`
- `knowledge_gaps`

这些信息虽然已经能支撑单次 `agent loop` 工作，但仍存在两个结构性问题：

1. 每轮 planner 都要从长 observation 和上下文文本中重新推断“我已经知道什么、验证了什么、下一步要做什么”
2. 会话内缺少一个稳定的、非用户可见的内部状态载体，导致“阶段状态”“已验证来源”“待解决问题”“下一步计划”只能零散散落在 observation 或最终文本中

用户本次明确提出希望：

- 在 Agent 内部增加一个状态机
- 大模型返回的“验证来源”“下一步计划”等内容维护在状态机里
- 该状态机不展示给用户
- 状态机应在同一会话内持续存在，供后续同会话 agent 运行继续使用

同时，用户希望该机制兼顾两种目标：

- 提高 agent 内部稳定性
- 提高调试可观测性

但优先级以“提高稳定性”为先。

---

## 2. 目标

本方案的目标是：

1. 为同一会话内的多次 agent 运行提供稳定的内部状态载体
2. 将“客观事实”和“模型主观中间态”分层管理，避免相互污染
3. 减少 planner 每轮对长 observation 的重复总结，提升下一步决策稳定性
4. 在不暴露给普通用户的前提下，为调试模式提供更清晰的内部上下文

---

## 3. 非目标

本方案第一阶段不处理以下内容：

- 不将状态机直接展示在主聊天窗口
- 不将状态机升级为数据库正式 schema
- 不做事件溯源或完整状态回放系统
- 不允许模型直接修改客观事实
- 不允许模型直接裁决最终阶段状态

---

## 4. 方案结论

本方案采用**会话级混合状态机**：

- 生命周期：绑定 `conversation_id`，覆盖同一会话内的多次 agent 运行
- 可见性：默认仅供 agent runtime 与 debug 使用，不进入普通用户 UI
- 维护方式：
  - `facts` 由系统根据 action outcome 自动维护
  - `working_state` 允许模型给出结构化建议，由系统校验后写入
- 使用方式：
  - planner 每轮读取压缩后的状态摘要
  - orchestrator 每轮执行后更新状态机
  - session 文件负责持久化与恢复

---

## 5. 核心模型

### 5.1 顶层结构

```ts
type AgentPhase =
  | "idle"
  | "retrieving"
  | "reading"
  | "verifying"
  | "synthesizing"
  | "awaiting_confirmation"
  | "blocked"
  | "completed";

interface AgentSessionStateMachine {
  conversation_id: string;
  version: number;
  phase: AgentPhase;
  facts: AgentFacts;
  working_state: AgentWorkingState;
  timestamps: AgentStateTimestamps;
}
```

### 5.2 `facts`：系统维护的客观事实

```ts
interface AgentFacts {
  verified_sources: string[];
  retrieved_refs: string[];
  verification_evidence: string[];
  source_usage: Record<string, number>;
  applied_file_operations: FileOperationFact[];
  knowledge_gaps: string[];
  last_error?: string | null;
  last_confirmation_request?: string | null;
  last_completed_action?: string | null;
}
```

含义如下：

- `verified_sources`：已被实际采信或交叉验证的来源
- `retrieved_refs`：检索命中的路径、URL、文档引用
- `verification_evidence`：已执行的验证证据摘要
- `source_usage`：不同来源类型的使用次数
- `applied_file_operations`：已经发生的文件改动事实
- `knowledge_gaps`：系统确认缺失的信息
- `last_error`：最近一次客观失败
- `last_confirmation_request`：最近一次确认请求原因
- `last_completed_action`：最近一次成功动作

### 5.3 `working_state`：模型建议的主观中间态

```ts
interface AgentWorkingState {
  working_hypothesis?: string | null;
  open_questions: string[];
  next_plan: string[];
  draft_summary?: string | null;
  confidence_notes?: string | null;
}
```

含义如下：

- `working_hypothesis`：当前工作假设或主线判断
- `open_questions`：尚未解决的问题
- `next_plan`：下一步计划
- `draft_summary`：阶段性总结草稿
- `confidence_notes`：模型自报的不确定性说明

### 5.4 时间戳

```ts
interface AgentStateTimestamps {
  updated_at: string;
  facts_updated_at?: string | null;
  working_state_updated_at?: string | null;
}
```

---

## 6. 设计原则

### 6.1 `facts` 与 `working_state` 强分层

`facts` 只记录系统可证明的内容，不能由模型直接写入。  
`working_state` 只记录模型主观中间态，不能覆盖 `facts`。

### 6.2 `phase` 由系统主导

模型可以给出 `suggested_phase`，但系统不直接采纳。  
最终阶段推进由 orchestrator 根据 action outcome 决定。

### 6.3 状态机是 observation 的压缩摘要，不是替代品

observation 仍然保留完整执行记录。  
状态机的作用是：

- 提供稳定上下文
- 提供决策摘要
- 提供调试快照

而不是替代原始日志。

---

## 7. 阶段状态设计

### 7.1 阶段枚举

- `idle`
- `retrieving`
- `reading`
- `verifying`
- `synthesizing`
- `awaiting_confirmation`
- `blocked`
- `completed`

### 7.2 阶段推进规则

系统按照 action 执行结果推进：

- `retrieve_context` 成功后 → `retrieving`
- `read_files` 成功后 → `reading`
- `verify_checks` / 用于验证的 `run_command` / `fetch_url` 成功后 → `verifying`
- `summarize_findings` 成功后 → `synthesizing`
- `ask_user` 或权限门拦截 → `awaiting_confirmation`
- 连续失败或 planner 判定无法继续 → `blocked`
- `finish(goal_status=done)` → `completed`

---

## 8. 状态更新规则

### 8.1 系统维护的 `facts`

#### `retrieve_context`

自动更新：

- `retrieved_refs`
- `source_usage`
- `verified_sources`（仅当后续被采信时）
- `knowledge_gaps`（例如 `web_search unavailable`）

#### `verify_checks`

自动更新：

- `verification_evidence`

#### `run_command`

当该命令属于验证型命令时，自动更新：

- `verification_evidence`

#### `write_files / apply_patch / create_files / rename_files / delete_files`

自动更新：

- `applied_file_operations`

#### `fetch_url / call_mcp_tool`

如结果被后续实际引用，可更新：

- `verified_sources`

#### 失败 observation

自动更新：

- `last_error`

#### 需要确认的 observation

自动更新：

- `last_confirmation_request`

#### 任意成功 action

自动更新：

- `last_completed_action`

### 8.2 模型建议的 `working_state`

模型允许通过结构化 `state_update` 建议更新以下字段：

- `working_hypothesis`
- `open_questions`
- `next_plan`
- `draft_summary`
- `confidence_notes`

系统处理规则：

1. 仅接受白名单字段
2. 字段类型必须严格匹配
3. `open_questions` 与 `next_plan` 限长，例如最多 8 条
4. 过长字符串截断
5. 空字符串转 `null`
6. 与 `facts` 冲突的内容不写入

### 8.3 `state_update` 示例

```json
{
  "action": "summarize_findings",
  "reason": "整理研究结果",
  "focus": "Snowflake 研究结论",
  "output_format": "research_brief",
  "state_update": {
    "working_hypothesis": "Snowflake 短期受估值压制，但长期平台价值仍在",
    "open_questions": ["下一季度收入指引是否继续下修"],
    "next_plan": ["补充最近一季财报电话会观点"],
    "draft_summary": "当前研究已覆盖主营业务、盈利模式和创办路径"
  }
}
```

---

## 9. Planner 使用方式

planner 不再只依赖长 observation，而是额外读取一份压缩后的状态摘要，例如：

```text
当前阶段: verifying
已验证来源: 36kr, InfoQ, 腾讯云开发者社区
已完成验证: web_search x8, cross-source consistency checked
未解决问题: 财报最新指引是否下修
当前计划: 补充财报电话会信息后生成最终结论
```

目标是让 planner 每轮都能明确知道：

- 当前处于哪一阶段
- 哪些来源已经足够可信
- 哪些问题还未解决
- 下一步优先做什么

---

## 10. 落地位置

### 10.1 `state.rs`

新增：

- `AgentSessionStateMachine`
- `AgentPhase`
- `AgentFacts`
- `AgentWorkingState`
- `AgentStateTimestamps`

并挂入：

```rust
pub struct AgentTaskState {
    ...
    pub session_state: AgentSessionStateMachine,
    pub observations: Vec<AgentObservation>,
}
```

### 10.2 `actions.rs`

新增：

- `StateUpdateSuggestion`

并为以下 action 增加可选 `state_update`：

- `RetrieveContext`
- `ReadFiles`
- `VerifyChecks`
- `FetchUrl`
- `SummarizeFindings`
- `Finish`

### 10.3 `planner.rs`

改动：

- planner prompt 中加入状态摘要
- planner schema 支持可选 `state_update`
- 解析器对白名单字段做严格校验
- 无效 `state_update` 被丢弃，但不应打断整个 action

### 10.4 `orchestrator.rs`

改动：

- 初始化或恢复会话状态机
- 每轮 action 后更新 `facts` 与 `phase`
- 合并合法的 `working_state`
- 为 planner 生成压缩状态摘要

### 10.5 `session.rs`

改动：

- 在 session 持久化结构中新增可选 `agent_session_state`
- 同一会话后续 agent 运行自动恢复
- 老 session 缺少该字段时按空状态初始化

---

## 11. 持久化策略

第一阶段不进入数据库，先写入 session 文件。

原因：

1. 改动范围更小
2. 兼容性风险更低
3. 更容易观察真实字段会如何演化
4. 后续如字段稳定，再决定是否迁移到 SQLite

因此：

- 生命周期：会话级
- 存储位置：session 文件
- 恢复方式：同会话下次 agent 运行时加载

---

## 12. 风险与控制

### 12.1 planner schema 漂移

风险：模型新增 `state_update` 后输出更脆弱。  
控制：

- 第一阶段不要求模型写 `state_update`
- 先落系统维护状态
- 第二阶段再扩 schema

### 12.2 状态污染

风险：模型把不可靠内容写入状态机。  
控制：

- `facts` 完全系统维护
- `working_state` 白名单字段 + 类型校验 + 长度限制

### 12.3 session 兼容性

风险：老 session 无法恢复。  
控制：

- 新字段全部 optional
- 缺失字段按默认空状态初始化

### 12.4 状态与 observation 矛盾

风险：状态机与真实执行记录冲突。  
控制：

- 以 observation 和 `facts` 为准
- `working_state` 仅作为辅助上下文，不作为事实来源

---

## 13. 实施顺序

### Phase 1：只做系统维护状态

做：

- `facts`
- `phase`
- planner 状态摘要注入

不做：

- `state_update`
- session debug 展示

目标：

- 先验证状态机是否能提高下一步决策稳定性

### Phase 2：允许模型写 `working_state`

做：

- `state_update`
- 白名单解析与校验
- 合并 `working_hypothesis / open_questions / next_plan / draft_summary`

目标：

- 引入主观中间态，但仍保持稳定边界

### Phase 3：会话恢复与调试可见性

做：

- session 文件持久化
- 同会话恢复
- Debug Mode 可选显示状态机快照

目标：

- 提高跨轮稳定性与调试效率

---

## 14. 推荐实施策略

如果只问“现在最该先做什么”，推荐是：

1. 先做 **Phase 1**
2. 先只让系统维护 `facts + phase`
3. 先把状态摘要喂给 planner
4. 暂时不要让模型写 `state_update`

原因：

- 风险最低
- 收益最直接
- 不会立即加重 planner JSON 脆弱性

---

## 15. 验收标准

当本方案实现完成后，应满足：

1. 同一会话内的后续 agent 运行可读取前一次状态机内容
2. planner 在 prompt 中能看到状态摘要，而不是只看 observation
3. `facts` 能稳定维护来源、验证、错误与确认请求
4. `working_state` 只允许白名单字段进入
5. 用户主 UI 不直接展示状态机
6. Debug Mode 可在后续阶段查看状态机快照
