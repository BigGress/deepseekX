# DeepSeekX 联网搜索闭环设计

> 日期：2026-05-23
> 状态：Draft
> 主题：方案 B - 两阶段调用 + 应用侧搜索编排

---

## 1. 背景

当前 DeepSeekX 在 GUI 主链路中通过 `send_message_via_api` 调用 DeepSeek Chat Completions API，并依赖：

- system prompt 中的“你拥有联网搜索能力”提示
- 请求体中的 `web_search: {"enable": true}`

来让模型自动完成实时信息检索。

该方案存在明显不稳定性：

- 模型有时直接返回最终答案
- 模型有时直接回答自己无法联网
- 模型有时只输出类似 `<search>SOXL stock price today</search>` 的中间协议文本，然后流程结束

根因是：当前应用没有把“搜索”实现为本地可控的工具闭环，而只是依赖模型和上游 API 自行决定是否完成搜索。

---

## 2. 目标

本方案的目标是将“联网搜索”改为应用侧可控流程，确保实时问题能够稳定走完：

1. 判断是否需要搜索
2. 生成搜索词
3. 执行搜索
4. 将搜索结果回填给模型
5. 生成最终答案

同时满足以下要求：

- 默认对所有用户零配置可用
- 提供一个内置免费搜索源
- 在免费源失效时自动回退到第二层免费方案
- 支持第三方搜索提供商，供用户在设置中切换
- 不再将 `<search>` 等中间协议暴露为最终回答

---

## 3. 非目标

本方案第一版不解决以下问题：

- 多轮自动搜索和复杂 agent 循环
- 搜索结果可信度评分系统
- 网页正文深度抓取与全文阅读
- 第三方 provider 的全量接入
- 搜索结果来源卡片、富文本来源引用等高级 UI

第一版只保证：一次判定、一次搜索、一次综合、一次收敛。

---

## 4. 方案概述

采用两阶段模型调用和应用侧搜索编排：

### 阶段一：Planner

模型只负责判断：

- 是否需要联网搜索
- 如果需要，应该搜索什么关键词

Planner 只输出结构化结果，不直接产出最终答案。

### 阶段二：Search Orchestrator

应用收到搜索请求后自行执行搜索，优先走默认免费源：

1. `PublicSearchProvider`
2. `WebScrapeProvider` 兜底

### 阶段三：Synthesizer

应用将裁剪后的搜索结果回填给模型，由模型生成最终中文答复。

---

## 5. 高层架构

```text
User Question
   |
   v
send_message_via_api
   |
   +--> Planner Completion
   |       |
   |       +--> action = answer ------> Final Response
   |       |
   |       +--> action = search
   |                    |
   |                    v
   |              Search Router
   |                    |
   |         +----------+-----------+
   |         |                      |
   |         v                      v
   |  PublicSearchProvider   WebScrapeProvider
   |         |                      |
   |         +----------+-----------+
   |                    |
   |                    v
   +------------> Synthesis Completion
                         |
                         v
                   Final Response
                         |
                         v
                  Session Persistence
```

---

## 6. 核心数据流

### 6.1 用户输入

示例：

```text
帮我查询下 SOXL 的今天的价格
```

### 6.2 Planner 输出

Planner 强制输出 JSON，仅允许两种形态。

直接回答：

```json
{"action":"answer","content":"..."}
```

请求搜索：

```json
{"action":"search","query":"SOXL stock price today","reason":"需要实时价格"}
```

### 6.3 搜索执行

应用执行搜索并得到统一结构：

```json
{
  "provider": "builtin_free",
  "query": "SOXL stock price today",
  "results": [
    {
      "title": "...",
      "url": "...",
      "snippet": "...",
      "source": "...",
      "score": 0.92
    }
  ],
  "status": "success"
}
```

### 6.4 综合回答

应用将问题与搜索结果摘要一起发给 Synthesizer：

```text
[User Question]
帮我查询下 SOXL 的今天的价格

[Search Results]
1. Title: ...
   URL: ...
   Snippet: ...
2. Title: ...
   URL: ...
   Snippet: ...
```

Synthesizer 必须输出最终可读中文答复，禁止再次输出 `<search>`。

---

## 7. 搜索提供商设计

### 7.1 默认提供商：Built-in Free

默认 provider 为 `builtin_free`，对所有用户零配置可用。

它不是单一搜索实现，而是一个带回退策略的 provider：

1. 优先：`PublicSearchProvider`
2. 回退：`WebScrapeProvider`

### 7.2 PublicSearchProvider

职责：

- 请求预置公共 `searxng` 风格实例
- 获取通用网页搜索结果
- 统一转成 `SearchResult[]`

要求：

- 短超时，避免界面长时间等待
- 支持多个公共实例轮询或备用列表
- 对失败、空结果、低质量结果给出明确状态

### 7.3 WebScrapeProvider

职责：

- 当公共实例失败时抓取公开搜索结果页
- 提取标题、链接、摘要
- 作为兜底免费能力

限制：

- 解析逻辑脆弱
- 不保证长期稳定
- 第一版只做轻量结果抽取，不做复杂反爬对抗

### 7.4 第三方 Provider

支持用户选择第三方提供商，例如：

- Tavily
- Exa
- SerpAPI
- Firecrawl
- Perplexity

这些 provider 通过统一接口适配，在未配置 API Key 时不可激活，并自动回退为 `builtin_free`。

---

## 8. 配置设计

### 8.1 应用级配置

联网搜索默认作为应用级能力配置，放在现有设置体系中，而不是项目表中。

建议新增字段：

- `search_enabled`
- `search_provider`
- `search_timeout_ms`
- `search_max_results`
- `search_api_keys`

其中：

- `search_provider` 默认值为 `builtin_free`
- `search_timeout_ms` 建议默认 8000-12000ms
- `search_max_results` 建议默认 3-5

### 8.2 项目级配置

项目只保留轻量控制项：

- `search_mode = inherit | disabled | force_enabled`

含义：

- `inherit`：使用全局搜索设置
- `disabled`：该项目禁用联网搜索
- `force_enabled`：即使全局关闭，该项目仍启用联网搜索

---

## 9. 会话与持久化设计

### 9.1 用户可见消息

最终 assistant 消息只保存最终答复文本。

禁止将以下内容写为最终消息：

- `<search>...</search>`
- planner 原始 JSON
- 原始网页抓取文本

### 9.2 中间过程记录

搜索步骤记录为 turn 内工具调用摘要，映射到现有 Turn 模型：

- `tool_name = "web_search"`
- `tool_input = query`
- `tool_result.summary = provider + 结果数 + 错误摘要`

这样可以复用现有 Turn UI 进行展示。

### 9.3 调试信息

默认不把完整搜索结果写入 session 主消息，避免 session 文件膨胀。

如需调试，可保留轻量摘要：

- 搜索 provider
- 查询词
- 结果数量
- 失败原因

---

## 10. 提示词设计

### 10.1 Planner Prompt

Planner 只做搜索决策，不做最终回答。

要求：

- 强制 JSON 输出
- 不允许输出 `<search>` 标签
- 不允许输出自然语言前奏
- 只允许 `answer` 或 `search`

### 10.2 Synthesizer Prompt

Synthesizer 根据搜索结果生成最终用户可见回答。

要求：

- 使用简体中文
- 直接回答用户问题
- 如果结果中有时效信息，优先标注“截至某时点”
- 不允许再次发起搜索
- 不允许输出 `<search>` 或中间协议

---

## 11. 失败状态设计

系统必须在单次请求中收敛，不允许悬挂或无限等待。

定义以下失败状态：

- `planner_failed`
- `search_timeout`
- `search_no_results`
- `search_provider_unavailable`
- `synthesis_failed`

用户可见失败文案原则：

- 简洁
- 可理解
- 可重试
- 不暴露内部协议

示例：

- `这次联网搜索超时了，请稍后重试。`
- `没有找到足够可靠的实时结果。`
- `当前搜索服务暂时不可用，已回退为普通回答。`

---

## 12. 模块拆分

建议按以下方式拆分：

### Rust 后端

- `src-tauri/src/search/mod.rs`
  - 搜索模块入口
  - 定义 trait 和通用类型
- `src-tauri/src/search/router.rs`
  - provider 选择与回退逻辑
- `src-tauri/src/search/settings.rs`
  - 搜索配置读取与默认值
- `src-tauri/src/search/providers/builtin_free.rs`
  - 免费双回退 provider
- `src-tauri/src/search/providers/public_instance.rs`
  - 公共实例 provider
- `src-tauri/src/search/providers/web_scrape.rs`
  - 网页抓取 provider
- `src-tauri/src/search/providers/third_party.rs`
  - 第三方 provider 适配
- `src-tauri/src/api.rs`
  - 增加 `planner_completion()` 和 `synthesis_completion()`
- `src-tauri/src/lib.rs`
  - 将 `send_message_via_api()` 改造为 orchestrator
- `src-tauri/src/session.rs`
  - 搜索步骤写入 turn 结构

### 前端

- `src/components/SettingsDialog.tsx`
  - 新增搜索 provider 设置项
- `src/components/ProjectSettings.tsx`
  - 新增 `search_mode`
- `src/api.ts`
  - 新增搜索设置读写接口
- `src/types.ts`
  - 新增 provider、settings、mode 类型

---

## 13. 实施顺序

### Phase 1：后端最小闭环

- 增加 `SearchPlan`、`SearchResult`、`SearchProvider`
- 实现 `planner_completion()`
- 实现 `synthesis_completion()`
- 在 `send_message_via_api()` 中串起两阶段调用

### Phase 2：默认免费搜索源

- 实现 `PublicSearchProvider`
- 实现 `WebScrapeProvider`
- 封装 `builtin_free`

### Phase 3：设置系统

- 应用级搜索设置
- 项目级 `search_mode`
- 默认值与配置回退规则

### Phase 4：会话和 UI

- 搜索步骤写入 session
- Turn 视图展示 `web_search`
- 设置页增加 provider 选择

### Phase 5：第三方 Provider

- 先接入 Tavily
- 再按统一接口逐步补其他 provider

---

## 14. 测试策略

### 14.1 单元测试

- Planner JSON 解析
- `<search>关键词</search>` 兼容解析
- provider 回退逻辑
- 搜索结果裁剪和摘要格式

### 14.2 集成测试

- Planner 返回 `answer`
- Planner 返回 `search`
- 公共实例失败时自动走抓取兜底
- Synthesizer 失败时返回确定性错误消息

### 14.3 回归测试

- `帮我查询下 SOXL 的今天的价格`
- `今天上海天气怎么样`
- `OpenAI 今天有什么新闻`
- `帮我解释 Go 的 interface`

回归验证重点：

- 实时问题能触发搜索
- 非实时问题不误触发搜索
- 最终消息不再出现 `<search>`
- session 中存在 `web_search` 工具调用摘要

---

## 15. 风险与取舍

### 风险

- 公共搜索实例稳定性不可控
- 网页抓取受目标页面结构变化影响
- 两阶段调用增加时延和 token 成本
- 第三方 provider 接入后测试矩阵增大

### 取舍

- 选择两阶段调用，是以额外成本换确定性
- 选择免费双回退，是以更高实现复杂度换零配置体验
- 第三方 provider 延后接入，是为了优先修复现有“卡死在 `<search>`”的问题

---

## 16. 验收标准

- 输入“帮我查询下 SOXL 的今天的价格”时，不再出现 `<search>...</search>` 作为最终回复
- 默认安装后无需额外配置即可触发联网搜索
- 公共实例失败时自动走抓取兜底
- 第三方 provider 配置后可替换默认 provider
- Turn 视图可显示一次 `web_search` 调用及摘要
- 普通问题不强制进入两阶段调用
- 任一失败场景都能在一次请求内返回确定性结果

---

## 17. 结论

本方案通过“Planner -> Search Router -> Synthesizer”的应用侧闭环，将联网搜索从不稳定的提示词能力，升级为 DeepSeekX 可控、可回退、可扩展的核心能力。

该方案优先解决当前用户最明显的问题：

- 不再卡在 `<search>...</search>`
- 实时问题具备更高成功率
- 默认对所有用户零配置可用

在此基础上，再逐步开放第三方搜索 provider，实现“默认免费可用 + 高级用户可扩展”的整体体验。
