# DeepSeekX Agent 能力模型设计

> 日期：2026-05-23
> 状态：Draft
> 主题：统一检索层 + 执行动作层

---

## 1. 背景

当前 DeepSeekX 已经有两条相关设计方向：

- 一条是应用内自动规划的 agent loop
- 一条是应用侧搜索编排

它们分别解决“如何多步执行”和“如何稳定触发搜索”的问题，但还没有回答一个更基础的问题：

- 如果 Agent 未来既要帮用户写代码，又要查本地资料，还要整理报告，它到底应该具备哪些能力？
- 在当前没有联网能力的情况下，应该如何先把离线能力设计对？
- 未来恢复联网后，如何不推翻现有架构，平滑接入外部搜索？

如果这个问题不先厘清，后续很容易把“代码操作”“文档检索”“联网搜索”“报告整理”做成几套互相平行、协议不统一的能力，最终导致：

- Planner 难以稳定决策
- 中间 observation 结构不一致
- 前端展示碎片化
- 后续扩展联网能力时需要重做协议

因此，需要单独定义一份 Agent 能力模型设计，明确能力边界、模块职责和第一版的实施优先级。

---

## 2. 目标

本方案目标是为 DeepSeekX 定义一套统一的 Agent 能力模型，使其能够同时支持以下三类任务：

1. 写代码
2. 查询资料
3. 整理报告

并满足以下约束：

- 当前没有联网能力时，离线能力仍然完整可用
- 项目内资料与用户导入资料可以统一检索
- 未来恢复联网时，只需要新增一个 source，不需要重做主循环
- Planner 只负责决策动作，不直接决定底层检索实现
- 工具内可以直接预览代码文件内容与修改结果
- 最终形成一条统一的任务闭环，而不是多套模式切换

---

## 3. 非目标

第一版不解决以下问题：

- 多 Agent 协同分工
- 多任务并行调度
- 长时后台任务与跨会话常驻执行
- 高级向量检索、复杂 rerank、知识图谱
- OCR、扫描件识别、复杂文档解析
- 报告模板市场或自定义插件系统
- 联网搜索的具体 provider 接入细节

第一版重点是：先把统一能力模型和离线主链路设计正确。

---

## 4. 核心结论

整体采用如下能力结构：

- 一个统一任务 loop
- 一个统一检索入口
- 一组受控执行动作
- 一个独立报告装配层
- 多种 source 逐步扩展

推荐架构不是：

- 代码 Agent
- 搜索 Agent
- 报告 Agent

而是一个更通用的任务型 Agent，由它统一调度“取上下文”“执行动作”“整理结果”三类能力。

---

## 5. 任务模型

### 5.1 三类核心任务

Agent 需要覆盖的任务分为三类：

1. 写代码
2. 查资料
3. 整理报告

这三类任务不应被设计为三套独立系统，而应共享同一条执行闭环：

```text
Plan -> Retrieve/Act -> Observe -> Replan -> Finish
```

### 5.2 典型任务路径

写代码：

```text
retrieve_context -> read_files -> write_files -> run_command -> finish
```

查资料：

```text
retrieve_context -> summarize_findings -> finish
```

整理报告：

```text
retrieve_context -> read_files/补检索 -> summarize_findings -> finish
```

这种设计的关键点是：不同任务只是在动作组合上不同，不需要切换到底层系统。

---

## 6. 能力分层

建议将 Agent 能力分为以下五层：

### 6.1 规划层

职责：

- 根据用户目标决定下一步动作
- 判断当前是应继续检索、执行、整理还是收敛
- 在预算和权限约束下动态调整路径

代表能力：

- `plan_task`

### 6.2 检索层

职责：

- 统一获取上下文
- 屏蔽不同资料来源的差异
- 给 Planner 返回可消费的结构化命中结果

代表能力：

- `retrieve_context`

### 6.3 执行层

职责：

- 读文件
- 写文件
- 运行命令
- 生成统一执行结果
- 为代码和文档变更生成可预览结果

代表能力：

- `read_files`
- `write_files`
- `run_command`

### 6.4 整理层

职责：

- 对检索和执行结果做摘要、归纳、对比
- 生成用户可读的报告或结论
- 为最终 finish 输出做准备

代表能力：

- `summarize_findings`

### 6.5 权限层

职责：

- 拦截高风险动作
- 判断是否需要用户确认
- 识别缺失信息和任务范围不清问题

代表能力：

- `ask_user`

---

## 7. 最小动作集设计

推荐的最小动作集如下：

1. `retrieve_context`
2. `read_files`
3. `write_files`
4. `run_command`
5. `summarize_findings`
6. `ask_user`
7. `finish`

其中：

- `retrieve_context` 负责找哪里值得看
- `read_files` 负责把文件读明白
- `write_files` 负责代码或文档修改
- `run_command` 负责验证或信息提取
- `summarize_findings` 负责把已有 observation 整理成用户可读输出
- `ask_user` 负责风险确认和补充信息
- `finish` 负责任务收敛

### 7.1 为什么不单独保留 `list_files`

原有 loop 中的 `list_files` 可以并入 `retrieve_context`。

原因：

- 列目录、本地搜文件、关键词命中，本质上都属于“找上下文”
- 单独暴露 `list_files` 会增加 Planner 的动作选择复杂度
- 把“找什么”和“怎么找”分开后，模型更容易稳定决策

### 7.2 为什么保留 `summarize_findings`

`summarize_findings` 不应完全依附于 `finish`，而应作为显式动作存在。

原因：

- 很多任务的目标本身就是“整理成一份能看的输出”
- 如果没有这个动作，模型容易过早 finish，导致结果过粗
- 报告能力未来需要独立扩展，不适合塞进收敛逻辑里

---

## 8. 统一检索层设计

### 8.1 设计原则

检索层对 Planner 暴露的是“统一取上下文”能力，而不是“底层搜索工具集合”。

也就是说，Planner 不需要决定：

- 用 grep
- 读 README
- 扫 docs
- 查导入资料
- 未来是否联网搜索

Planner 只需要表达：

- 我要查什么
- 为什么要查
- 优先偏向哪类资料
- 需要多少结果

### 8.2 推荐 source 划分

统一检索层底下建议包含以下 source：

1. `workspace_code`
2. `workspace_docs`
3. `user_knowledge_base`
4. `web_search`

各自职责如下。

#### `workspace_code`

覆盖：

- 源码
- 配置文件
- 脚本
- 日志
- 注释

适用场景：

- 找入口文件
- 查实现逻辑
- 找某个符号、模块或调用路径

#### `workspace_docs`

覆盖：

- `README`
- `docs/`
- 设计文档
- 规范说明
- 变更记录

适用场景：

- 查架构说明
- 查产品约束
- 查项目约定

#### `user_knowledge_base`

覆盖：

- 用户导入的 PDF
- markdown
- txt
- 网页快照
- 笔记

适用场景：

- 查离线资料
- 查参考规范
- 查用户提供的背景材料

#### `web_search`

覆盖：

- 未来联网后的实时检索能力

当前阶段要求：

- 接口保留
- 当前无联网能力时返回结构化 unavailable
- 不允许让模型自行假设自己已经联网

### 8.3 统一输入格式

建议 `retrieve_context` 至少包含以下字段：

```json
{
  "action": "retrieve_context",
  "reason": "需要先理解现有 agent 相关实现和设计文档",
  "query": "agent orchestrator planner permissions",
  "intent": "understand_existing_system",
  "preferred_sources": ["workspace_code", "workspace_docs"],
  "max_results": 8
}
```

### 8.4 统一输出格式

建议所有 source 统一返回 `ContextHit[]`，每条至少包含：

- `source_type`
- `title`
- `location`
- `snippet`
- `confidence`
- `freshness`
- `next_hint`

这样后续的：

- 二次精读
- 结果排序
- observation 记录
- 报告整理

都可以复用同一套结构。

### 8.5 与 `read_files` 的分工

两者职责必须明确区分：

- `retrieve_context`：找哪里值得看
- `read_files`：把具体文件读明白

不建议让 `retrieve_context` 直接返回大段原文，否则会带来：

- 与 `read_files` 重叠
- observation 体积迅速膨胀
- 上下文预算浪费

---

## 9. 检索路由规则

统一检索层需要在应用侧完成 source 路由，而不是让模型硬编码底层实现。

建议路由规则如下：

- 如果 query 明显是项目实现细节，优先 `workspace_code`
- 如果 query 明显是设计说明、产品规则、项目约定，优先 `workspace_docs`
- 如果 query 涉及用户导入的规范、资料、参考文档，优先 `user_knowledge_base`
- 如果本地 source 结果不足，再考虑 `web_search`
- 当前无联网能力时，`web_search` 返回 unavailable，而不是 silently 忽略

这意味着联网只是统一检索层中的一个 source，而不是一条独立系统。

---

## 10. 模块边界

### 10.1 AgentOrchestrator

职责：

- 驱动主循环
- 管理步数、超时、失败阈值
- 协调 Planner、RetrievalRouter、ActionExecutor、PermissionGate
- 在合适时机结束任务

### 10.2 Planner

职责：

- 只输出下一步结构化动作
- 不直接执行动作
- 不决定底层检索细节

关键约束：

- 每轮只输出一个动作
- 只使用受支持动作
- 目标足够完成时输出 `finish`
- 需要确认或信息不足时输出 `ask_user`

### 10.3 RetrievalRouter

职责：

- 接收 `retrieve_context`
- 根据 `query`、`intent`、任务阶段选择 source
- 合并不同 source 的结果并去重排序
- 返回统一 `ContextHit[]`

它不负责：

- 返回超长原文
- 生成最终摘要
- 把具体底层实现暴露给 Planner

### 10.4 ActionExecutor

职责：

- 执行 `read_files`
- 执行 `write_files`
- 执行 `run_command`
- 生成统一执行结果

对 `write_files`，除写入落盘外，还应返回结构化变更信息，供工具内直接预览：

- 修改了哪些文件
- 新增、删除、修改了哪些片段
- 是否属于 create / overwrite / patch
- 是否可以生成简化 diff 预览

### 10.5 ObservationBuilder

职责：

- 把检索结果和执行结果压缩成统一 observation
- 控制 observation 的长度与结构
- 为下一轮 Planner 提供可消费上下文

### 10.6 PermissionGate

职责：

- 判定动作风险
- 识别越界写文件、高风险命令、外部副作用
- 将需要确认的动作改写为 `ask_user`

### 10.7 AgentStateStore

职责：

- 保存任务目标
- 保存执行步骤
- 保存 observation
- 保存失败原因
- 保存待确认动作

建议新增字段：

- `collected_context_refs`
- `knowledge_gaps`
- `report_draft`
- `source_usage`
- `web_capability`

### 10.8 ReportAssembler

职责：

- 把 observation 整理成用户可读的结果
- 生成简报、结构化报告、变更说明、研究摘要
- 让报告能力从 `finish` 中独立出来

---

## 11. 代码预览与改动可视化

为了支持“在工具里直接看到改动的代码”，Agent 除了具备执行修改能力，还应具备代码文件预览和变更可视化能力。

### 11.1 文件预览能力

工具内应支持对代码文件进行直接预览，至少包括：

- 当前文件内容预览
- 命中文件片段预览
- 根据检索结果快速跳转到对应文件或片段
- 在修改前查看原始内容

这项能力既服务于：

- `retrieve_context` 之后的精读
- `read_files` 的结果展示
- `write_files` 前的修改确认

### 11.2 改动预览能力

当 Agent 执行 `write_files` 后，工具内应能直接展示修改结果，而不要求用户再去外部编辑器手动比对。

建议最小展示内容包括：

- 文件路径
- 变更类型：`create | modify | overwrite`
- 修改前摘要
- 修改后摘要
- 可读的 diff 或 patch 片段

对于代码文件，优先展示：

- 变更片段级预览
- 上下文若干行
- 新增和删除的高亮区分

### 11.3 为什么这项能力很重要

如果 Agent 可以改代码，但用户无法在工具内直接看到改动，会带来几个问题：

- 用户难以快速判断改动是否符合预期
- 确认流程会变重
- “整理报告”只能停留在文字描述，缺少真实变更证据
- 对多步修改任务来说，信任成本会明显上升

因此，代码预览不是附属 UI 细节，而是 Agent 执行闭环中的关键反馈能力。

### 11.4 与动作层的关系

这项能力不需要新增一个独立动作，而应作为以下动作的标准输出能力存在：

- `read_files`：返回可预览文件内容或片段
- `write_files`：返回可预览改动结果
- `summarize_findings`：可引用改动摘要生成变更报告

也就是说，Planner 仍然不需要额外学习“如何预览”，预览由应用侧自动生成并渲染。

### 11.5 统一 observation 建议

建议对文件类 observation 增加以下字段：

- `preview_type`: `full | snippet | diff`
- `before_preview`
- `after_preview`
- `diff_preview`
- `changed_ranges`

这样可以保证：

- 检索命中可以展示 snippet
- 精读文件可以展示 full 或 snippet
- 写文件后可以展示 diff

### 11.6 第一版建议范围

第一版不需要做复杂编辑器级体验，但建议至少具备：

- 代码文件文本预览
- 修改后 diff 预览
- 文件级变更列表
- 在最终结果中引用本次改动摘要

这样用户在工具里就能直接看到 Agent 改了什么，而不是只看到一句“我已经修改完成”。

---

## 12. 状态与收敛语义

在现有 loop 基础上，建议继续沿用受控收敛：

- `planning`
- `awaiting_approval`
- `executing`
- `observing`
- `completed`
- `blocked`
- `failed`

针对资料检索和报告整理场景，收敛语义建议保持统一：

- `done`
- `blocked`
- `needs_confirmation`

这三种结束状态同时适用于：

- 写代码任务
- 查资料任务
- 整理报告任务

从而避免每类任务再定义独立终态。

---

## 13. 第一版能力范围

### 13.1 第一版必须具备

- `retrieve_context` 可查询 `workspace_code`
- `retrieve_context` 可查询 `workspace_docs`
- `read_files` 可精读命中的项目文件
- `write_files` 可修改工作区文件
- `run_command` 可执行低风险验证命令
- `summarize_findings` 可输出基础结构化报告
- `ask_user` 可处理高风险动作和缺失信息
- `finish` 可输出 `done | blocked | needs_confirmation`

### 13.2 第一版可以后置

- 真正的联网搜索执行
- 复杂向量检索和 rerank
- PDF OCR
- 多任务并行
- 多 Agent 协同
- 长时后台运行
- 复杂报告模板系统

第一版追求的不是“什么都能查”，而是：

- 能查项目内代码和文档
- 能统一组织结果
- 能平滑扩展到离线资料库和未来联网 source

---

## 14. 实施顺序

### Phase 1：统一检索层接入现有 loop

- 调整动作集为统一能力集合
- 接入 `retrieve_context`
- 先支持 `workspace_code` 与 `workspace_docs`
- 保持主循环仍然是单任务、单动作、受控收敛

### Phase 2：补齐资料整理能力

- 引入 `ReportAssembler`
- 让 `summarize_findings` 支持基础报告格式
- 区分中间步骤展示和最终报告展示

### Phase 3：接入用户离线资料库

- 增加 `user_knowledge_base`
- 支持导入本地文档与笔记
- 做基础分块、索引和元数据管理

### Phase 4：接入联网 source

- 增加 `web_search` 的实际 provider
- 保持 Planner 协议不变
- 保持状态机不变
- 保持 UI 主结构不变

这个顺序可以确保：

- 当前无联网时就有明确价值
- 未来联网时不需要推翻前面的设计

---

## 15. 与现有设计文档的关系

本设计文档建议作为一份独立能力模型说明，与已有文档形成分层关系：

- `2026-05-23-agent-loop-design.md`
  - 负责描述多步执行 loop、状态机、权限门和动作闭环
- `2026-05-23-search-orchestrator-design.md`
  - 可逐步收敛为未来 `web_search` source 的子设计
- 本文档
  - 负责描述 Agent 的整体能力边界与统一检索模型

也就是说，未来不是“再做一套搜索系统”，而是把搜索纳入统一检索层。

---

## 16. 验收标准

- Agent 可以在一个任务中同时完成“取上下文、执行动作、整理结果”
- 写代码、查资料、整理报告三类任务共享同一条主循环
- `retrieve_context` 能统一面向多个 source 返回结构化结果
- Planner 不需要直接感知底层检索实现
- 当前无联网时，离线能力仍然可用
- 未来联网时，只需新增 `web_search` source 即可接入
- 报告整理能力不依附于 `finish`，而是显式能力
- 工具内可以直接预览代码文件与写入后的变更结果

---

## 17. 结论

DeepSeekX 的 Agent 更适合被定义为一个任务型知识与执行 Agent，而不是单纯的代码 Agent 或搜索 Agent。

它的核心能力应建立在以下结构之上：

- 用统一检索层获取上下文
- 用受控动作层执行副作用
- 用独立报告层整理结果
- 用同一条 loop 完成写代码、查资料、整理报告三类任务

这种设计的最大收益是：

- 当前离线即可落地
- 后续联网可平滑接入
- 能力边界清晰
- 协议统一
- 易于扩展和展示

对于 DeepSeekX 的第一版，最值得优先追求的体验不是“什么都能做”，而是：

- 能查项目代码和文档
- 能在必要时修改和验证
- 能把结果稳定整理给用户

这会比单独堆叠搜索工具或报告工具，更接近一个真正可用的应用内 Agent。
