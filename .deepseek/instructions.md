# DeepSeekX 项目设计规范

> Herness 设计规范 v1.0  
> 本项目所有 agent 在执行前必须阅读并遵守本规范。

---

## 一、项目介绍

DeepSeekX 是一款跨平台桌面客户端，基于 Tauri v2 构建，为 DeepSeek CLI 提供图形化交互界面。用户可以在多对话环境中向 DeepSeek 模型发送任务描述，查看 AI 生成的响应。

- **项目代号**：Herness
- **产品名称**：DeepSeekX
- **版本**：0.1.0
- **语言**：简体中文（界面 / 文档 / 注释）
- **目标平台**：macOS、Windows、Linux

---

## 二、技术栈

| 层 | 技术 | 版本 |
|---|------|------|
| 桌面框架 | Tauri v2 | 2.x |
| 前端框架 | React | 18.3 |
| 类型系统 | TypeScript (strict) | 5.6 |
| CSS 框架 | TailwindCSS | 3.4 |
| 构建工具 | Vite | 5.4 |
| Rust 异步 | tokio | 1.x |
| 序列化 | serde + serde_json | 1.x |
| 后端通信 | Tauri invoke / tauri-plugin-shell | 2.x |

---

## 三、项目结构

```
deepseekX/
├── index.html              # 入口 HTML（lang=zh-CN）
├── package.json             # 前端依赖与脚本
├── tsconfig.json            # TypeScript 严格模式配置
├── vite.config.ts           # Vite 配置（端口 1420）
├── tailwind.config.js       # Tailwind 自定义色值
├── postcss.config.js        # PostCSS 配置
├── gen_icons.py             # 图标生成脚本
│
├── src/                     # React 前端源码
│   ├── main.tsx             # React 入口 (ReactDOM.createRoot)
│   ├── App.tsx              # 根组件 + 核心类型定义
│   ├── index.css            # Tailwind 指令 + 全局样式
│   ├── vite-env.d.ts        # Vite 类型声明
│   └── components/          # 组件目录
│       ├── Sidebar.tsx      # 左侧对话列表
│       ├── ChatPanel.tsx    # 主聊天面板
│       ├── MessageList.tsx  # 消息列表 + MessageContent
│       └── ChatInput.tsx    # 底部输入框
│
├── src-tauri/               # Rust 后端源码
│   ├── Cargo.toml           # Rust 依赖
│   ├── tauri.conf.json      # Tauri 应用配置
│   ├── build.rs
│   ├── src/
│   │   ├── main.rs          # 入口（Windows 隐藏控制台）
│   │   └── lib.rs           # Tauri command 定义
│   ├── capabilities/
│   │   └── default.json     # 权限声明
│   ├── gen/schemas/         # 自动生成的 schema
│   └── icons/               # 应用图标
│
├── dist/                    # 构建产物
└── node_modules/            # npm 依赖
```

---

## 四、命名规范

### 4.1 TypeScript / React

| 元素 | 风格 | 示例 |
|------|------|------|
| 组件文件 | PascalCase | `ChatInput.tsx` |
| 组件函数 | PascalCase | `function ChatInput()` |
| 接口 / 类型 | PascalCase | `Message`, `SidebarProps` |
| 变量 / 函数 | camelCase | `handleSend`, `activeId` |
| 常量 | UPPER_SNAKE 或 camelCase | `const MAX_LENGTH = 100` |
| React Hooks | `use` 前缀 | `useState`, `useCallback` |
| 事件处理 | `handle` 前缀 | `handleSend`, `handleKeyDown` |
| 布尔状态 | `is` / `has` / `should` 前缀 | `isLoading`, `hasError` |
| Ref | `xxxRef` 后缀 | `inputRef`, `bottomRef` |

### 4.2 Rust

| 元素 | 风格 | 示例 |
|------|------|------|
| 函数 | snake_case | `send_message` |
| 模块 | snake_case | `deepseekx_lib` |
| 类型 / Trait | PascalCase | `AppState` |
| 宏 | snake_case | `generate_handler!` |
| Tauri command | snake_case | `send_message` |

---

## 五、TypeScript 代码风格

### 5.1 类型定义

- 核心数据模型（`Message`、`Conversation`）定义在 `App.tsx` 顶部
- 组件 Props 以 `interface` 形式定义在各自组件文件内
- 所有组件 Props 必须有显式类型

```typescript
// ✅ 正确：组件文件内的 Props 接口
interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  isLoading: boolean;
  inputRef: React.RefObject<HTMLTextAreaElement>;
}

export default function ChatInput({ value, onChange, onSend, onKeyDown, isLoading, inputRef }: ChatInputProps) {
```

### 5.2 组件结构

- 使用函数组件 + React Hooks，禁止 class 组件
- 组件导出使用 `export default function`
- Hooks 调用放在组件函数顶部，按以下顺序：
  1. `useState`
  2. `useRef`
  3. `useEffect`
  4. `useCallback` / `useMemo`

```typescript
export default function ChatPanel({ conversation, isLoading, onSend }: ChatPanelProps) {
  const [inputValue, setInputValue] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null!);

  useEffect(() => {
    inputRef.current?.focus();
    setInputValue("");
  }, [conversation?.id]);

  const handleSend = () => { /* ... */ };
  const handleKeyDown = (e: React.KeyboardEvent) => { /* ... */ };

  return ( /* JSX */ );
}
```

### 5.3 回调与事件

- 父组件向子组件传递回调用 `on` 前缀命名：`onSend`、`onChange`、`onDelete`
- 事件处理器在子组件内部称为 `handle` 前缀：`handleSend`、`handleKeyDown`
- `useCallback` 用于传递给子组件的回调，避免不必要的重渲染

### 5.4 条件渲染

- 优先使用 `&&` 短路或三元表达式
- 早期 return 用于空状态

```typescript
// ✅ 早期 return 处理空列表
if (messages.length === 0) {
  return <EmptyState />;
}
```

### 5.5 TypeScript 严格模式

- `strict: true` 已开启
- `noUnusedLocals: true` — 无未使用的局部变量
- `noUnusedParameters: true` — 无未使用的参数
- `noFallthroughCasesInSwitch: true`

---

## 六、Rust 代码风格

### 6.1 Tauri Command 模式

每个 Tauri command 必须：
- 使用 `#[tauri::command]` 属性宏
- 返回 `Result<T, String>`，错误信息使用中文
- 通过 `.map_err()` 将底层错误转为用户友好的提示

```rust
#[tauri::command]
async fn send_message(conversation_id: String, message: String) -> Result<String, String> {
    let child = Command::new("deepseek")
        .arg("-p")
        .arg(&message)
        .env("DEEPSEEK_CONVERSATION_ID", &conversation_id)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 DeepSeek CLI: {}", e))?;
    // ...
}
```

### 6.2 错误处理

- 对外暴露的错误信息使用中文
- `.map_err()` 用于将底层错误转换为可读信息
- 不要在 `Err` 中暴露内部实现细节

### 6.3 异步

- Tauri command 使用 `async fn`
- 使用 `tokio::process::Command` 而非 `std::process::Command`
- 进程等待用 `.wait_with_output().await`

---

## 七、TailwindCSS 设计系统

### 7.1 主题

全应用使用暗色主题，禁止浅色方案。

| Token | 色值 | 用途 |
|-------|------|------|
| `neutral-950` | `#0a0a0a` | 页面主背景 |
| `neutral-925` | `#0f0f0f` | 面板背景（Sidebar / Header） |
| `neutral-850` | `#1a1a1a` | 消息气泡 / 输入区域背景 |
| `neutral-800` | `#262626` | 边框 / 活跃项背景 |
| `neutral-700` | `#404040` | 输入框边框 / 占位图标 |
| `neutral-500` | `#737373` | 次要文字 / 占位符 |
| `neutral-200` | `#e5e5e5` | 正文文字 |
| `neutral-100` | `#f5f5f5` | 高亮文字 |
| `blue-600` | Tailwind 默认 | 发送按钮 / 用户消息气泡 |
| `blue-500` | Tailwind 默认 | 按钮 hover 态 |
| `red-400` | Tailwind 默认 | 删除按钮 hover 态 |
| `yellow-300` | Tailwind 默认 | 系统消息文字 |

### 7.2 排版

- 正文字号 `text-sm`（14px）
- 标题字号 `text-sm font-medium`
- 副文本字号 `text-xs`
- 代码块字号 `text-xs`
- 行高：`leading-relaxed`

### 7.3 布局规则

- 全局容器：`h-screen w-screen bg-neutral-950`（禁止滚动条）
- 面板内滚动区域使用 `overflow-y-auto`
- 不可缩放区域使用 `shrink-0`
- 消息区域最大宽度：`max-w-3xl`（768px），`mx-auto` 居中
- 消息气泡最大宽度：`max-w-[85%]`

### 7.4 间距

- 组件内边距：`p-3` / `p-4`
- 元素间距：`gap-2`
- 圆角：`rounded-lg` / `rounded-xl` / `rounded-md`
- 项目间距：`my-0.5`

---

## 八、组件设计原则

### 8.1 单一职责

每个组件文件只导出一个默认组件：
- `Sidebar.tsx` — 仅负责对话列表侧边栏
- `ChatPanel.tsx` — 仅负责聊天面板布局
- `MessageList.tsx` — 仅负责消息渲染（内部含 `MessageContent` 辅助组件）
- `ChatInput.tsx` — 仅负责输入框与发送按钮

### 8.2 状态提升

- 对话状态（`conversations`、`activeId`、`isLoading`）全部在 `App.tsx` 管理
- 子组件通过 Props 接收状态和回调，保持无状态（展示组件）
- 输入框的本地状态（`inputValue`）保留在 `ChatPanel` 内，因为它只影响该组件

### 8.3 组件通信

```
App (状态管理)
├── Sidebar ← conversations, activeId, onSelect, onNew, onDelete
└── ChatPanel ← conversation, isLoading, onSend
    ├── MessageList ← messages, isLoading
    └── ChatInput ← value, onChange, onSend, onKeyDown, isLoading, inputRef
```

### 8.4 导入规范

- 第三方库放在文件顶部
- 本地导入按以下顺序：
  1. 类型导入（如有跨文件类型）：`import type { Message } from "../App"`
  2. 子组件导入
  3. Hooks 导入（如有自定义 hooks）

```typescript
import { useState, useRef, useEffect } from "react";
import type { Conversation } from "../App";
import MessageList from "./MessageList";
import ChatInput from "./ChatInput";
```

---

## 九、性能规范

- `useCallback` 包裹传递给子组件的回调函数
- `useRef` 存储 DOM 引用，避免 `document.getElementById`
- 列表渲染使用稳定的 `key`（`msg.id`、`conv.id`）
- 避免在 JSX 中内联函数或对象，除非已用 `useCallback` / `useMemo`
- TailwindCSS JIT 模式自动处理生产构建的 CSS 剔除

---

## 十、Git 提交规范

提交信息使用中文，格式：

```
<类型>: <简短描述>

类型可选值：
- feat    : 新功能
- fix     : 修复缺陷
- refactor: 代码重构
- style   : 样式调整
- docs    : 文档更新
- chore   : 构建/工具链变更

示例：
feat: 添加多对话管理侧边栏
fix: 修复消息发送后输入框高度不重置
refactor: 将 MessageContent 拆分为独立组件
```

---

## 十一、禁止事项

- ❌ 使用 CSS-in-JS 方案（如 styled-components）
- ❌ 使用 class 组件
- ❌ 在 Rust 错误信息中暴露原始系统错误
- ❌ 使用 `any` 类型（已在 strict 模式下尽可能避免）
- ❌ 使用行内样式（`style={{}}`），全部通过 Tailwind 工具类实现
- ❌ 在组件中直接操作 DOM（使用 React Refs）
- ❌ 使用浅色主题
- ❌ 在非组件文件中定义组件 Props 接口
