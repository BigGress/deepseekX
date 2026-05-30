# Primary Result Markdown Rendering Design

Status: Archived
Date: 2026-05-30
Owner: Codex

## 背景

当前聊天窗口中 `PRIMARY RESULT` 底下的结果文本虽然通常是 Markdown，但渲染仍依赖仓库内手写解析器 [MessageMarkdown.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.tsx)。这导致两个问题：

1. Markdown 支持不完整，尤其是 GFM 能力缺失或不稳定，例如表格、任务列表、复杂列表、边界 case。
2. 渲染逻辑需要继续手动维护，后续每遇到一种新 Markdown 结构都要补规则，成本持续上升。

仓库已经安装了 `react-markdown` 与 `remark-gfm`，但当前没有在 `PRIMARY RESULT` 的 Markdown 主渲染链路上使用它们。

## 目标

将 `PRIMARY RESULT` 的 Markdown 渲染切换为基于现有依赖的标准实现，以获得更完整、稳定的 Markdown/GFM 支持，同时保留现有视觉风格和交互行为。

## 非目标

本次设计不包含以下内容：

- 不修改 `Execution Detail`、`Artifacts`、`Debug Mode` 等其它富文本区域的渲染实现
- 不引入新的 Markdown 渲染库
- 不增加 HTML 原样注入或 unsafe HTML 渲染
- 不在本次范围内新增数学公式、Mermaid、脚注、语法高亮库等增强能力

## 方案概述

采用仓库已有依赖：

- `react-markdown`
- `remark-gfm`

替换 `PRIMARY RESULT` 使用的 `MessageMarkdown` 内部实现。上层调用关系保持不变，外部组件仍然只依赖一个 `MessageMarkdown` 组件，不感知内部实现切换。

## 设计原则

### 1. 最小替换

只替换 [MessageMarkdown.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.tsx) 的内部解析实现，不重构 `TurnItem`、`ResultCardGroup` 或结果区信息架构。

### 2. 样式连续性

虽然渲染引擎切换到库，但输出视觉风格应尽量延续当前产品样式，尤其包括：

- 标题层级
- 段落间距
- 链接颜色与下划线
- 引用块样式
- 行内代码与代码块
- 列表缩进与节奏

### 3. 安全优先

默认不启用 HTML 原样渲染，不接入 `rehype-raw`。`PRIMARY RESULT` 只渲染 Markdown AST 支持的安全子集，避免让模型输出的原始 HTML 直接进入 DOM。

### 4. GFM 完整支持

启用 `remark-gfm`，至少覆盖：

- 表格
- 任务列表
- 删除线
- 自动链接

## 组件设计

### 保持单一入口

`PRIMARY RESULT` 继续通过 `MessageMarkdown` 组件渲染，不修改上层接口。

```tsx
<MessageMarkdown content={finalResponse} />
```

### MessageMarkdown 的内部结构

`MessageMarkdown` 改为：

1. 使用 `ReactMarkdown`
2. 注册 `remarkGfm`
3. 通过 `components` 映射自定义常见节点样式

需要覆盖的节点包括：

- `h1` ~ `h6`
- `p`
- `ul`
- `ol`
- `li`
- `blockquote`
- `a`
- `code`
- `pre`
- `table`
- `thead`
- `tbody`
- `tr`
- `th`
- `td`
- `input`（任务列表复选框的只读显示）

## 支持范围

### 首版必须正确支持

- 标题
- 段落
- 无序列表
- 有序列表
- 引用
- 行内代码
- fenced code block
- 链接
- GFM 表格
- GFM 任务列表
- 删除线

### 首版可接受的限制

- 不处理数学公式
- 不处理 Mermaid
- 不处理脚注
- 不额外引入语法高亮器，仅保留现有代码块视觉样式

## 样式约束

新实现不应退化为浏览器默认 Markdown 样式。应通过 `components` 映射保留产品内一致的视觉语义。

具体要求：

- 代码块继续使用深色背景和横向滚动
- 行内代码继续使用轻量背景高亮
- 链接继续新开窗口，并保留 `rel="noreferrer"`
- 表格需要可横向滚动，避免挤爆消息宽度
- 任务列表复选框只读，不允许用户交互修改

## 测试设计

测试继续使用 Vitest 与 Testing Library，更新 [MessageMarkdown.test.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.test.tsx)。

至少覆盖：

1. 标题、列表、链接、粗体、斜体、行内代码
2. fenced code block
3. GFM 表格
4. 任务列表
5. 删除线

额外要求：

- 回归测试要确保现有基础 Markdown 能力不退化
- 若表格采用外层滚动容器，测试至少验证表格结构真实存在

## 实施步骤

### Phase 1

替换 `MessageMarkdown` 内部实现为 `react-markdown + remark-gfm`，完成最小样式映射。

### Phase 2

更新与扩展单元测试，覆盖 GFM 能力与现有回归项。

### Phase 3

运行前端构建与测试，确认 `PRIMARY RESULT` 在现有调试预览页和真实聊天结果中显示正常。

## 验收标准

完成后应满足：

1. `PRIMARY RESULT` 中的 Markdown 由 `react-markdown + remark-gfm` 渲染
2. 现有标题、列表、引用、链接、代码块样式不明显退化
3. 表格与任务列表可以正确渲染
4. `npm test` 中相关渲染测试通过
5. `npm run build` 通过

## 风险与控制

### 风险 1：样式回退到浏览器默认

控制方式：
通过 `components` 映射明确覆盖关键标签，不依赖默认 UA 样式。

### 风险 2：代码块和行内代码样式退化

控制方式：
保留现有 `pre/code` 的视觉样式约束，只替换解析逻辑，不轻易改 CSS 语义。

### 风险 3：任务列表默认变成可交互控件

控制方式：
统一把复选框渲染为只读、禁用态。

### 风险 4：模型输出原始 HTML 带来安全问题

控制方式：
首版不启用 raw HTML 渲染。
