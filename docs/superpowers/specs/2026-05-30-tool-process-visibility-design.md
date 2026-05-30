# DeepSeekX Tool 过程展示优化设计

> 日期：2026-05-30
> 状态：Draft
> 主题：弱化 tool 调用过程展示，强化最终结果优先级

---

## 1. 背景

当前 DeepSeekX 的聊天结果区已经具备统一的 `Turn` 展示骨架：

- `User Intent`
- `Primary Result`
- `Execution Detail`
- `Artifacts`
- `State & Actions`

但在实际使用中，`Execution Detail` 里的过程性内容仍然偏重，尤其是以下两类：

1. `retrieve_context` 带来的原始检索命中摘要
2. `summarize_findings` 带来的整理结果正文

在研究类问题中，这些内容容易和最终答案竞争注意力，导致用户需要在“过程信息”和“最终结论”之间来回筛选。

用户本次明确提出的目标是：

- tool 调用的结果属于过程性内容，不应抢占主展示区域
- 这类内容不要求删除，但应默认折叠
- 对 `summarize_findings` 这一步，正常聊天窗口里只应弱展示摘要，完整正文仅在用户展开时查看

---

## 2. 目标

本方案的目标是：

1. 让 `Primary Result` 明确成为唯一的默认主内容区
2. 让 tool 调用过程默认折叠，避免干扰普通阅读
3. 让 `summarize_findings` 从“重过程卡片”收敛为“轻摘要入口”
4. 保留高级用户查看过程细节的能力，但把它降到二级甚至三级层级

---

## 3. 非目标

本方案不处理以下问题：

- 改写 planner / retrieval / summarize_findings 的核心生成能力
- 改变 Debug Mode 的日志粒度
- 新增独立的搜索结果工作台
- 完整重做 `Turn` 展示模型

本方案只聚焦“正常聊天窗口里，tool 过程应该如何展示”。

---

## 4. 核心设计结论

### 4.1 最终结果优先

正常聊天窗口中，用户首先看到的应该始终是：

- 最终答案
- 简短验证
- 可选下一步

而不是：

- 检索过程
- 工具调用结果
- 整理过程全文

### 4.2 过程内容默认折叠

`Execution Detail` 默认保持折叠，仅在以下情况自动展开：

- `blocked`
- `failed`
- `needs_confirmation`

在成功完成的普通研究 / 问答场景中，执行过程默认不展开。

### 4.3 `summarize_findings` 采用“两级展示”

`summarize_findings` 在正常聊天窗口中应采用：

1. 一级：只显示一行摘要
2. 二级：展开该步骤后，才允许查看完整整理正文

也就是说，这一步不再默认把完整整理结果暴露在执行区第一层。

### 4.4 `summarize_findings` 正文不得回流原始 retrieval 明细

`summarize_findings` 的完整正文应被视为“整理后的结果”，而不是“再包装一次原始 observation dump”。

因此在展示层应明确约束：

- 正文可展示整理后的研究简报
- 正文不应再混入大段 `retrieve_context` 原始命中
- 原始命中仍保留在对应的 `retrieve_context` 步骤或 Debug 面板中

---

## 5. 当前问题分析

结合 `request_id = fafdfb74-c9c8-4313-8881-0a9df8c5c723` 的原始返回，本次问题主要有三层：

### 5.1 `finish.summary` 已足够交付

这次 LLM 的最终 `finish` 已经包含：

- Snowflake 是做什么的
- 主营业务
- 盈利方式
- 如何创办类似公司

对于普通用户，这已经是应直接看到的最终答案。

### 5.2 `summarize_findings` 结果过重

当前 `summarize_findings` 的 `result_summary` 中不仅有“整理后的主题和结论”，还包含：

- `retrieve_context` 命中条目
- 原始 URL
- 原始命中摘要
- 预览内容重复

这导致 `summarize_findings` 变成了“重复展示 retrieval 过程”的大卡片。

### 5.3 展示层级不够硬

虽然结果区已经有 `Primary Result` 与 `Execution Detail` 的分层，但当前实现中：

- `Execution Detail` 仍然可以轻易抢占用户注意力
- `ExecutionCard` 对成功态步骤也以较高信息密度展开
- 过程信息的默认权重仍然偏高

---

## 6. 目标信息架构

### 6.1 正常成功场景

```text
Turn
  User Intent
  Primary Result
    final answer
    verification
    next step (optional)
  State & Actions
    done
  Execution Detail (collapsed by default)
    retrieve_context
    summarize_findings
    other tool steps
```

### 6.2 异常或待确认场景

```text
Turn
  User Intent
  Primary Result
    blocked / failed / approval summary
  State & Actions
    failed | blocked | needs_confirmation
  Execution Detail (expanded by default)
    failing step
    blocked reason
    relevant tool output
```

---

## 7. `summarize_findings` 的展示模型

### 7.1 默认显示

在执行区中，该步骤默认只显示：

- action name: `summarize_findings`
- status badge
- reason（可选）
- 一行摘要，例如：
  - `已整理研究简报，提炼为 4 个结论`

### 7.2 展开后显示

用户展开该步骤后，才展示：

- 整理后的完整正文

### 7.3 不应展示

在该步骤正文中，不应继续展示：

- 原始 `retrieve_context` 命中列表
- 大段 URL 集合
- 与上一工具步骤重复的原始 observation dump

---

## 8. 组件级改动建议

### 8.1 `ExecutionPanel`

当前问题：

- `ExecutionPanel` 只区分整体折叠，不区分“卡片级摘要”和“卡片级正文”

建议：

- 整个 `Execution Detail` 保持默认折叠
- 每个 `ExecutionCard` 增加二级展开能力
- 对 `summarize_findings` 启用“摘要 + 正文”双层展示

### 8.2 `turnPresentation.ts`

建议新增展示策略函数，例如：

- `shouldExpandExecutionByDefault(turn)`
- `shouldShowFullExecutionCard(step, turn)`
- `isSummaryLikeStep(step)`

用于统一约束：

- 成功态默认折叠
- 异常态自动展开
- `summarize_findings` 默认只露摘要

### 8.3 `ResultCardGroup`

建议保持现在的主职责不变：

- 只消费 `turn.final_response`
- 不直接拼接 `agent_steps[*].result_summary`

这样可以确保主结果区不会再次被过程数据污染。

---

## 9. 展示规则

### 9.1 主结果区规则

主结果区允许显示：

- 最终回答
- 简短验证
- 可选下一步

主结果区不允许直接显示：

- tool 过程全文
- 原始检索命中列表
- 过程性 URL 明细

### 9.2 执行区规则

执行区默认折叠。

只有以下状态下自动展开：

- `blocked`
- `failed`
- `needs_confirmation`

### 9.3 `summarize_findings` 规则

在正常聊天窗口中：

- 默认只显示一行摘要
- 完整整理正文仅在用户展开该步骤时显示
- 正文不得再混入 retrieval 原始命中明细

---

## 10. 实施顺序

### Phase 1

- 为 `Execution Detail` 增加“成功态默认折叠，异常态默认展开”策略

### Phase 2

- 为 `ExecutionCard` 增加二级展开能力

### Phase 3

- 对 `summarize_findings` 启用“摘要 / 正文”双层展示

### Phase 4

- 清理 `summarize_findings` 正文中的 retrieval 重复明细展示

---

## 11. 验收标准

满足以下条件时，认为方案达成：

1. 成功完成的研究类对话中，用户默认只看到最终答案和状态，不会先看到工具过程
2. `Execution Detail` 在成功态默认折叠
3. `blocked / failed / needs_confirmation` 场景仍会自动暴露必要过程，方便定位问题
4. `summarize_findings` 在默认状态下只展示一行摘要
5. 展开 `summarize_findings` 后可以看到完整整理正文
6. 该正文不再重复展示 retrieval 原始命中明细

---

## 12. 结论

本次优化不是“隐藏所有过程”，而是重新定义过程内容的展示层级：

- 最终答案是一级内容
- 执行过程是二级内容
- 原始检索与调试细节是更低优先级内容

其中，`summarize_findings` 的关键改动是：

**从“默认展开的大段整理结果”收敛为“默认一行摘要，展开后查看正文，且正文不再混入 retrieval 原始明细”。**
