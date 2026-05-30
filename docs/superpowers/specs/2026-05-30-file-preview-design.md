# DeepSeekX 文件预览系统设计

> 日期：2026-05-30
> 状态：Draft
> 主题：为工作区增加统一文件预览能力，覆盖浏览器原生可预览类型并对常见办公文档提供增强预览

---

## 1. 背景

DeepSeekX 当前已经具备：

- 工作区文件树浏览
- `read_file_content` 文本读取
- 聊天结果中的文件引用、diff 与 artifact 卡片
- 聊天结果区的统一 `Turn` 展示骨架

但仍缺少一个统一、可扩展的“文件预览系统”。

当前缺口主要体现在：

1. 文件树只能浏览，不能直接预览绝大多数文件类型
2. 聊天结果里的 `@文件`、diff、文件操作路径无法统一跳转到同一个预览视图
3. 浏览器原生可预览类型与需要转换的文档类型没有统一模型
4. 文本、Markdown、图片、PDF、Office 文档的预览逻辑没有形成系统

用户希望：

- 增加文件预览能力
- 最好支持浏览器能预览的所有文件类型
- 文件树作为主入口，聊天结果中的文件引用作为次入口
- 默认使用右侧预览面板，同时复杂文件支持放大到独立预览视图
- `.docx / .xlsx / .pptx / .csv / .pdf` 这类文档既能结构化快速预览，也能尽量提供原始外观预览

---

## 2. 目标

本方案的目标是：

1. 提供一个统一的文件预览系统，而不是零散的格式特判
2. 让文件树和聊天结果都能打开同一个预览器
3. 对浏览器原生强支持的类型提供高质量直出体验
4. 对 Office / PDF / 表格类文件提供“结构化预览 + 原始外观预览”的双模式
5. 通过前后端分层，控制复杂度并为后续扩展更多格式预留接口

---

## 3. 非目标

首版不追求以下能力：

- 完整等价于专业 IDE 的编辑器能力
- 复杂设计文件（如 PSD / AI / Figma）的高保真预览
- 每种二进制文件都提供强行可视化展示
- 任意本地路径的预览访问
- 大规模全文检索式文档解析工作台

首版重点是：**统一预览模型 + 高质量常见文件预览 + 合理的重转换与降级策略**。

---

## 4. 方案结论

本方案采用**混合分层预览器（Hybrid Layered Previewer）**：

- 前端直接处理轻量、浏览器原生支持强的类型
- 后端负责文件识别、预览描述、重转换、缓存和安全边界
- 文件树与聊天结果共用一套 `Preview Controller`
- 默认在右侧预览面板展示，必要时进入独立放大视图
- 文档类文件默认优先显示结构化预览，并允许切换到原始外观预览

这是在以下三种备选方案中选择的结果：

### 方案 A：纯前端预览器

- 前端直接按扩展名处理所有格式
- 优点：实现快，链路简单
- 缺点：Office 文档体验断层，无法达到“尽量全覆盖”

### 方案 B：后端统一转换所有格式

- 所有文件都先走后端转换，再统一返回给前端
- 优点：能力统一，前端轻
- 缺点：转换系统过重，复杂度和调试成本高

### 方案 C：混合分层预览器（推荐）

- 浏览器擅长的由前端直接渲染
- 类型识别、安全控制和重转换交给后端
- 通过统一 descriptor 抹平前后端差异

选择理由：

- 覆盖面足够广
- 与当前 Tauri 架构匹配
- 不会一开始就把系统做成纯文档转换平台
- 后续可以增量增强 Office 与富文档能力

---

## 5. 信息架构

### 5.1 统一预览工作区

预览系统不应拆成两套逻辑，而应是一个统一能力：

```text
FileTree click / Chat artifact click
  -> Preview Controller
    -> resolve preview descriptor
    -> render in Right Preview Panel
    -> optional Open in Focused Preview
```

用户有两个入口，但底层只有一个预览器。

### 5.2 主入口：文件树

文件树是主路径，行为定义如下：

- 单击：选中文件
- 双击：打开右侧预览
- Hover 或行尾预览按钮：显式打开预览
- 已打开预览的文件在树中高亮

这里不采用“单击直接预览”，是为了避免文件浏览与预览动作混在一起，降低跳动感。

### 5.3 次入口：聊天结果

聊天区以下位置应可打开同一个预览器：

- `@文件` 附件
- `file_operations.path`
- diff card 中的文件路径
- artifact / preview 里的原文件引用

行为统一为：

- 点击文件名或“查看文件”
- 调起 `openPreview(target)`
- 自动复用右侧预览面板
- 对复杂文件可再点“放大预览”

### 5.4 默认视图：右侧预览面板

右侧预览面板作为默认工作流形态，适合在聊天和文件树之间连续切换。

建议结构：

- 顶部栏
  - 文件名
  - 类型标签
  - 模式切换
  - 刷新
  - 放大
  - 关闭
- 中央内容区
  - 实际预览内容
- 底部状态区（可选）
  - 大小
  - MIME
  - 编码 / 页数 / sheet
  - 转换时间

### 5.5 放大视图：独立预览模式

放大视图不是新的系统，只是右侧面板的专注模式。

适合：

- PDF
- PPT
- 大图
- 长 Markdown / HTML
- 多 sheet 的表格

建议行为：

- 从右侧面板点“放大预览”
- 进入覆盖层（overlay），不新开系统窗口
- 关闭后回到当前右侧预览状态

---

## 6. 预览数据模型

前端不应该到处按扩展名散落判断，而应围绕统一 descriptor 渲染。

### 6.1 `PreviewTarget`

```ts
interface PreviewTarget {
  path: string;
  source: "file_tree" | "chat_attachment" | "artifact" | "diff";
  title?: string;
}
```

含义：

- `path`：工作区内相对或受控路径
- `source`：用户从哪里触发的预览
- `title`：可选显示名

### 6.2 `PreviewDescriptor`

```ts
interface PreviewDescriptor {
  path: string;
  file_name: string;
  extension: string | null;
  mime_type: string | null;
  size_bytes: number | null;

  category:
    | "text"
    | "markdown"
    | "code"
    | "image"
    | "audio"
    | "video"
    | "pdf"
    | "html"
    | "csv"
    | "spreadsheet"
    | "document"
    | "presentation"
    | "archive"
    | "binary"
    | "unknown";

  default_mode:
    | "structured"
    | "rendered"
    | "raw"
    | "metadata";

  available_modes: Array<
    "structured" | "rendered" | "raw" | "metadata"
  >;

  capabilities: {
    can_render_inline: boolean;
    can_open_focused: boolean;
    can_download: boolean;
    can_show_text_extract: boolean;
    can_show_original_appearance: boolean;
  };

  content?: PreviewContent;
  metadata?: PreviewMetadata;
  warnings?: string[];
}
```

### 6.3 `PreviewContent`

```ts
type PreviewContent =
  | { kind: "text"; text: string; language?: string }
  | { kind: "markdown"; markdown: string }
  | { kind: "html"; html: string; sandboxed: boolean }
  | { kind: "table"; columns: string[]; rows: string[][] }
  | { kind: "media"; url: string; media_type: "image" | "audio" | "video" | "pdf" }
  | { kind: "pages"; pages: Array<{ page: number; image_url: string }> }
  | { kind: "fallback"; message: string };
```

### 6.4 `PreviewMetadata`

```ts
interface PreviewMetadata {
  detected_encoding?: string;
  line_count?: number;
  page_count?: number;
  sheet_names?: string[];
  dimensions?: { width: number; height: number };
  duration_seconds?: number;
  generated_by?: "frontend" | "backend" | "converter";
  generated_at?: string;
}
```

### 6.5 数据模型原则

1. 前端按 descriptor 渲染，不按扩展名散落判断
2. 模式切换由数据驱动，不由 UI 写死
3. 后端可逐步增强而不破坏前端结构
4. 无法高质量预览时，必须有清晰 fallback

---

## 7. 文件类型分层策略

### 7.1 Layer 1：前端直接渲染

这层优先处理浏览器原生支持强、后端无需重转换的类型。

包含：

- 纯文本：`.txt`, `.log`
- 代码与配置：`.ts`, `.tsx`, `.js`, `.jsx`, `.rs`, `.json`, `.yml`, `.yaml`, `.toml`, `.css`, `.xml`, `.sh`, `.py`, `.sql`
- Markdown：`.md`, `.mdx`
- 图片：`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`
- 音视频：`.mp3`, `.wav`, `.ogg`, `.mp4`, `.webm`, `.mov`
- PDF：浏览器内嵌
- CSV：表格化渲染
- HTML：源码或沙箱渲染

默认模式建议：

- 代码/配置：`raw`
- Markdown：`structured`
- HTML：`rendered`
- CSV：`structured`
- PDF：`rendered`

### 7.2 Layer 2：后端轻转换

用于浏览器不擅长直接展示，但转换成本相对可控的类型。

包含：

- `.docx`
- `.xlsx`
- `.pptx`
- `.rtf`（可选）
- 复杂编码或大型 `.csv`
- `.pdf` 的文本抽取模式

策略：

- `.docx`
  - 结构化：HTML
  - 原始外观：后续可转换为页图或 PDF
- `.xlsx`
  - 结构化：sheet + table
  - 原始外观：首版可缺省
- `.pptx`
  - 结构化：slide outline
  - 原始外观：后续页图渲染
- `.pdf`
  - 原始外观：浏览器嵌入
  - 结构化：文本抽取

### 7.3 Layer 3：后端重转换

用于复杂富文档，只在需要时触发。

包含：

- 大型 `.pptx`
- 复杂 `.xlsx`
- 图文复杂 `.docx`
- 未来支持的 `.pages / .numbers / .key`

策略：

- 首次不预热
- 用户切到 `rendered` 模式时才触发
- 结果必须缓存
- 更适合进入放大视图

### 7.4 Layer 4：明确降级

这些类型不追求强行可视化：

- 压缩包：`.zip`, `.tar`, `.gz`
- 可执行：`.app`, `.exe`, `.dylib`
- 数据库或其他二进制：`.db`, `.sqlite`, `.bin`
- 未识别类型

策略：

- 展示元数据
- 提示不支持直接预览
- 提供下载 / 系统打开 / 十六进制摘要（可选）

### 7.5 首版覆盖矩阵

#### 必须高质量支持

- text / code / markdown / json / yaml / toml
- image
- audio / video
- pdf
- csv
- html
- svg

#### 首版应支持，但允许结构化优先

- docx
- xlsx
- pptx

#### 首版可明确降级

- zip / binary / unknown

---

## 8. 后端接口与转换管线

### 8.1 命令分层

不应把文件预览做成 `read_file_content(path)` 的扩展版，而应拆为两层：

#### `describe_file_preview(path)`

职责：

- 校验路径
- 识别扩展名 / MIME / 大小
- 决定 `category`
- 决定 `default_mode`
- 决定 `available_modes`
- 决定是否需要后续转换

返回：

- 轻量 `PreviewDescriptor`
- `content` 可为空

#### `resolve_file_preview(path, mode)`

职责：

- 真正生成指定 mode 的可渲染内容
- 命中缓存时直接返回
- 需要转换时触发转换

返回：

- 完整 `PreviewDescriptor`

### 8.2 受控资源访问

对以下内容不能直接给前端裸磁盘路径：

- PDF 临时文件
- 转换后的 PPT 页图
- DOCX 页图
- 临时 HTML / 大图缓存

应通过受控资源引用返回，例如：

- `app-preview://...`
- 或 Tauri 受控本地资源 URL/token

### 8.3 转换管线

后端内部拆成四步：

#### Step 1：类型识别

输入：文件路径  
输出：

- 扩展名
- MIME
- 是否文本
- 文件大小
- 是否适合前端直接渲染

#### Step 2：预览策略决策

根据：

- 文件识别结果
- 用户请求的 mode

决定使用哪条 pipeline。

#### Step 3：内容生成

建议的内部子管线：

- `text_loader`
- `markdown_loader`
- `csv_to_table`
- `pdf_embed`
- `docx_to_html`
- `xlsx_to_sheets`
- `pptx_to_outline`
- `pptx_to_slide_images`
- `office_to_rendered_pages`

#### Step 4：缓存与返回

缓存 key 至少包含：

- `path`
- `mtime`
- `mode`

示例：

```text
preview-cache/<hash(path + mtime + mode)>
```

### 8.4 缓存策略

- 轻量文本：可不落磁盘缓存
- 重转换类：必须缓存到磁盘
- 文件变更后自动失效，依据 `mtime + size`

### 8.5 安全边界

必须保证：

1. 只允许预览工作区内或白名单内文件
2. HTML 预览必须沙箱化
3. 不执行用户文件中的任意脚本
4. 不向前端暴露任意本地绝对路径
5. 大文件有加载上限与降级策略

---

## 9. 前端交互与组件设计

### 9.1 工作区形态

采用可收起的三段式工作区：

```text
Sidebar
  -> 文件树
Main Area
  -> 聊天 / 结果区
Right Preview Panel
  -> 文件预览
```

默认仍是两栏；触发预览时，右侧面板展开。

### 9.2 建议新增状态

```ts
type PreviewMode = "structured" | "rendered" | "raw" | "metadata";

interface PreviewState {
  isOpen: boolean;
  target: PreviewTarget | null;
  descriptor: PreviewDescriptor | null;
  selectedMode: PreviewMode | null;
  isLoading: boolean;
  error: string | null;
  focusedView: boolean;
}
```

### 9.3 建议组件拆分

#### `FilePreviewController`

职责：

- 管理预览状态
- 调用 `describe` / `resolve`
- 统一协调文件树与聊天入口

#### `FilePreviewPanel`

职责：

- 右侧面板容器
- 顶部栏
- 模式切换
- 加载 / 错误 / 空态

#### `PreviewModeTabs`

职责：

- 渲染可用模式切换

#### `PreviewRenderer`

职责：

- 根据 `content.kind` 选择具体渲染器

可拆为：

- `TextPreview`
- `MarkdownPreview`
- `HtmlPreview`
- `TablePreview`
- `MediaPreview`
- `PagedPreview`
- `FallbackPreview`

#### `FocusedPreviewOverlay`

职责：

- 预览放大模式
- 复用同一个 descriptor，不重复渲染逻辑

### 9.4 文件树交互

- 单击：选中
- 双击：打开预览
- Hover 或按钮：显式预览
- 已打开预览文件高亮

### 9.5 聊天结果交互

这些位置增加打开预览入口：

- `@文件`
- `file_operations.path`
- diff card 文件路径
- artifact 里的原文件按钮

统一调用：

```ts
openPreview(target)
```

### 9.6 模式切换规则

- 初次打开使用 `default_mode`
- 切换模式时调用 `resolve_file_preview(path, mode)`
- 已加载过的 mode 尽量复用缓存
- 重模式切换不应整面板白屏

### 9.7 错误与降级状态

前端需要明确区分：

- `empty`
- `loading`
- `unsupported`
- `degraded`
- `error`

不能只显示一句“预览失败”，而要同时说明：

- 当前失败的是哪个 mode
- 是否有别的 mode 可用
- 是否可下载或系统打开

---

## 10. 首版实施边界

### 10.1 首版必须完成

1. 文件树主入口预览
2. 聊天结果文件引用跳转预览
3. 右侧预览面板
4. 放大预览覆盖层
5. 文本 / 代码 / Markdown / JSON / YAML / TOML
6. 图片 / 音视频 / PDF
7. CSV 表格化
8. HTML 渲染与源码双模式
9. `docx / xlsx / pptx` 的结构化预览
10. 统一 descriptor 与模式切换

### 10.2 首版可降级但要有清晰行为

1. Office 原始外观预览
2. 超大文件局部预览
3. 二进制与压缩包降级
4. 复杂多 sheet / 多页文档的高级导航

### 10.3 首版明确不做

1. 编辑能力
2. 富批注
3. 多预览标签页
4. 非工作区任意路径访问
5. 设计工具文件的高保真渲染

---

## 11. 推荐实施顺序

### Phase A：预览骨架

- 建立 `PreviewTarget / PreviewDescriptor`
- 增加右侧预览面板状态
- 文件树入口接入
- 空态 / loading / error / unsupported

### Phase B：前端直渲染类型

- text / code
- markdown
- image / audio / video
- pdf
- csv
- html

### Phase C：聊天结果入口接入

- `@文件`
- file operations
- diff / artifact 文件引用

### Phase D：后端轻转换

- `docx -> html`
- `xlsx -> sheets`
- `pptx -> outline`

### Phase E：放大预览与重转换缓存

- focused overlay
- 延迟触发的 `rendered` 模式
- 转换缓存

---

## 12. 验收标准

该功能完成后，应满足：

1. 文件树中的文件可以通过统一入口进入预览
2. 聊天结果中的文件引用可以打开同一预览器
3. 文本、Markdown、代码、图片、音视频、PDF、CSV、HTML 具有可用且稳定的预览体验
4. `docx / xlsx / pptx` 至少具备结构化预览能力
5. 复杂文件可以从右侧面板放大到独立预览视图
6. 不支持的类型有清晰降级，而不是静默失败
7. 前后端之间通过统一 descriptor 协议工作，而不是散落的扩展名特判
8. 预览系统不破坏当前聊天结果区与工作区主流程

---

## 13. 结论

这个功能不应被实现为“再加一个读文件弹窗”，而应被实现为：

- 统一的文件预览系统
- 两个入口，共用一个控制器
- 前端直渲染 + 后端转换的混合架构
- 右侧工作流面板 + 放大预览视图
- 结构化预览与原始外观预览并存

从产品上看，这个能力会让 DeepSeekX 从“能读文本文件的聊天式工作区”，升级到“能浏览和理解多类型工作区文件的任务型桌面 Agent 工具”。
