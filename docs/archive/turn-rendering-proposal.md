# 技术提案：GUI 多轮消息渲染（using-superpowers 版）

> 日期：2026-05-23  
> 状态：待 review  
> 版本：v2 — using-superpowers 重写

---

## 0. 前置：技能扫描

实施前扫描可用技能，避免重复造轮：

| 技能 | 相关性 | 结论 |
|------|--------|------|
| `using-superpowers` | 本次实施纪律 | ✅ 本提案即按此技能编写 |
| `delegate` | 子任务拆分 | Phase 3 组件开发可委托子 agent |
| `v4-best-practices` | 防止计划假设过时 | 分组算法验证必须用真实 session 文件 |

> **规则**：任何实施步骤开始前，先调用对应的 skill 检查。不跳过这步。

---

## 1. 问题分析

### 1.1 根因

TUI session 中 **433 条消息对应 13 次真实对话**。每次对话中 assistant 执行多轮工具调用：

```
user      -> "帮我写一个技术提案"
assistant -> thinking + tool_use           <- 第1轮
user      -> tool_result
assistant -> thinking + tool_use           <- 第2轮
user      -> tool_result
...
assistant -> thinking + text               <- 最终回复
```

当前 `extractText` 只取 `type: "text"` block。结果：
- `thinking` ❌ 丢弃
- `tool_use` ❌ 丢弃
- `tool_result` ❌ 丢弃
- 中间轮消息渲染为空

### 1.2 消息类型全景

| Block 类型 | 出现方 | 当前 | 提案 |
|---|---|---|---|
| `text` | user / assistant | ✅ | ✅ 保持 |
| `thinking` | assistant | ❌ | ✅ 折叠区内显示 |
| `tool_use` | assistant | ❌ | ✅ 工具名 + 参数摘要 |
| `tool_result` | user | ❌ | ✅ 成功/失败 + 摘要 |

### 1.3 数据验证

> **反模式警示**：不要根据记忆或假设设计分组算法。必须用真实 session 文件验证。

```
433 messages -> 13 user turns
  text: 132 | thinking: 193 | tool_use: 227 | tool_result: 227
  Max tool rounds/turn: 24 | Avg: ~15
```

---

## 2. 设计方案

### 2.1 核心概念：Turn

一个真实用户输入到最终回复之间的完整交互链。

```
Turn {
  userInput       <- 用户问题
  thinkingSteps[] <- 思考+工具链（可折叠）
  finalResponse   <- 最终文本
}
```

### 2.2 分组算法（后端实现）

> **决策门禁**：选后端而非前端，理由见 §4 决策表。

```
输入: SessionMessage[]
输出: Turn[]

1. 遍历消息，维护 currentTurn
2. role=user 且不含 tool_result -> 提交 currentTurn，开始新 Turn
3. role=assistant:
   a. thinking -> currentTurn.thinkingSteps 追加
   b. tool_use -> 同上，记录 toolCall
   c. text:
      - 若后续有 tool_use -> 忽略（中间过程的 text）
      - 否则 -> currentTurn.finalResponse
4. role=user 且含 tool_result -> 匹配上一轮 toolCall
5. 遍历结束 -> 提交最后一个 Turn
```

### 2.3 类型定义

```typescript
// src/types.ts

interface Turn {
  id: string;
  userInput: string;
  thinkingSteps: ThinkingStep[];
  finalResponse: string | null;
}

interface ThinkingStep {
  thinking: string;           // 截断至 200 字符
  toolCalls: ToolCallSummary[];
  toolResults: ToolResultSummary[];
}

interface ToolCallSummary {
  toolCallId: string;
  toolName: string;
  toolInput: string;          // 截断至 100 字符
}

interface ToolResultSummary {
  toolCallId: string;
  isError: boolean;
  summary: string;            // 截断至 200 字符
}
```

### 2.4 渲染布局

```
+---------------------------------------------+
|  👤 用户: "帮我写一个技术提案"               |
+---------------------------------------------+
|  🧠 思考过程 (3 轮)  [▶ 展开]               |  <- 默认折叠
|  +-----------------------------------------+ |
|  | 💭 用户想知道 codex 有哪些功能...       | |  <- italic text-neutral-400
|  | 🔧 web_search "...Codex 功能"           | |  <- text-blue-400
|  |    📋 (无结果)                          | |  <- text-neutral-500
|  | --------------------------------------- | |
|  | 💭 换关键词搜索...                      | |
|  | 🔧 web_search "Codex Desktop..."        | |
|  |    📋 (无结果)                          | |
|  | --------------------------------------- | |
|  | 💭 基于已有知识回答...                  | |
|  +-----------------------------------------+ |
+---------------------------------------------+
|  🤖 关于 OpenAI Codex，目前通常指...        |  <- 完整 Markdown
+---------------------------------------------+
```

### 2.5 样式速查

| 元素 | Tailwind |
|------|----------|
| 折叠标题 | `text-xs text-neutral-500 cursor-pointer` |
| 思考内容 | `italic text-neutral-400 text-xs` |
| 工具调用 | `text-blue-400 text-xs` |
| 工具结果成功 | `text-green-400 text-xs` |
| 工具结果失败 | `text-red-400 text-xs` |
| 面板背景 | `bg-neutral-900 rounded p-2` |

### 2.6 组件树

```
MessageList.tsx (改造)
|-- 旧 Message[] -> 原 MessageContent（兼容路径）
|-- 新 Turn[] -> TurnList.tsx（新增）
    └── TurnItem.tsx（新增）
        |-- UserBubble       <- 复用现有
        |-- ThinkingPanel    <- 可折叠，新增
        └── AssistantBubble  <- 复用现有
```

---

## 3. 实施清单

> **纪律**：以下清单是刚性约束。每步完成后验证，不得跳过。

### Phase 1 — 后端 Turn 分组

- [ ] 1.1 用真实 session JSON 写 `group_into_turns()` 的单元测试（Python 脚本验证）
- [ ] 1.2 `session.rs` 实现 `group_into_turns()`
- [ ] 1.3 `lib.rs` 新增 `read_tui_session_turns` command
- [ ] 1.4 `cargo build` 通过

> **反模式警示**：不要"边写边试"。先写测试用例，再写实现。真实 session JSON 就是测试数据。

### Phase 2 — 前端数据流

- [ ] 2.1 `types.ts` 新增 Turn 类型
- [ ] 2.2 `api.ts` 新增 `readTuiSessionTurns()`
- [ ] 2.3 `App.tsx` 切换消息加载到 Turns
- [ ] 2.4 `tsc --noEmit` 通过

### Phase 3 — Turn 渲染组件

> **委托提示**：`TurnList.tsx` 和 `TurnItem.tsx` 可并行开发，考虑用 delegate 技能拆分子任务。

- [ ] 3.1 新建 `TurnItem.tsx` — 单 Turn 渲染（用户气泡 + 思考面板 + 回复气泡）
- [ ] 3.2 新建 `TurnList.tsx` — Turn 列表容器
- [ ] 3.3 实现 `ThinkingPanel` 折叠/展开（`useState`）
- [ ] 3.4 `MessageList.tsx` 适配：Turn[] 走新渲染，Message[] 走旧渲染
- [ ] 3.5 Tailwind 样式适配暗色主题

### Phase 4 — 边界情况 + 验证

- [ ] 4.1 无工具调用的简单对话 -> 不显示思考面板
- [ ] 4.2 未完成的 tool_use（无 tool_result）-> 显示"等待中"
- [ ] 4.3 thinking 为空 -> 不显示该行
- [ ] 4.4 超长结果截断验证
- [ ] 4.5 用真实 session 文件完整回归

---

## 4. 决策表

| # | 决策点 | 选项 A | 选项 B | 结论 | 理由 |
|---|--------|--------|--------|------|------|
| 1 | 分组在哪层 | 后端 Rust | 前端 TS | **后端** | 减少传输 400+ 条消息；前端纯渲染 |
| 2 | 默认状态 | 折叠思考 | 展开思考 | **折叠** | 95% 用户只看最终回复 |
| 3 | 原始消息流视图 | 提供 | 不提供 | **不提供** | v1 简化；有需求再加 |
| 4 | 旧消息兼容 | Turn 渲染 | 原样渲染 | **原样** | 旧消息无 tools，简单问答 |

---

## 5. 验收标准

- [ ] 包含 24 轮工具调用的 session 渲染，不超过 3 秒
- [ ] 思考面板默认折叠，点击可展开
- [ ] 无工具调用的对话不显示思考面板
- [ ] 旧格式消息（纯文本）正常渲染，不崩溃
- [ ] `cargo build` + `tsc --noEmit` 零错误零警告
