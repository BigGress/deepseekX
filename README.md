# DeepSeekX

DeepSeekX 是一个基于 Tauri 的桌面 AI 工作台，面向代码、文档、检索和本地工具协作。它同时提供普通 Chat 与 Agent 两条运行路径，让你既可以把它当作聊天助手，也可以把它当作一个能读取工作区、调用工具、执行验证和产生产物的桌面智能体。

和普通聊天工具相比，DeepSeekX 更强调本地工作区协作、工具执行、结果可追踪和桌面端工作流整合，而不是只返回一段文本答案。

## 核心能力

- 普通 Chat 与 Agent 双模式，适合从快速问答到多步任务执行的不同场景
- 本地工作区读取、文件编辑、结构化 patch、diff 预览和文件操作
- 检索与网页搜索，支持工作区上下文、知识库和联网搜索组合使用
- MCP / skills 扩展能力，可把外部工具和专用工作流接入桌面应用
- 调试模式、LLM 请求日志和执行步骤视图，便于排查 planner、tool 和 UI 问题
- 会话级 agent 状态机，用于维护阶段状态和客观执行事实
- 聊天结果区、执行过程区和文件产物区分层展示，减少过程噪音
- 文件预览体系正在持续增强，支持在工作区中查看不同类型文件的内容与产物

## 典型使用场景

- 让 agent 阅读当前项目并完成一个小到中等规模的代码修改任务
- 做研究型问答，保留最终结论，同时在需要时查看执行过程和验证证据
- 对改动运行 build、test、lint 或其他验证命令，并把结果回写到对话上下文
- 从聊天结果里的文件引用、diff 或附件继续查看工作区文件
- 通过 MCP / skills 把浏览器自动化、桌面操作或其他外部能力接进同一工作台

## 技术栈

- Frontend: React 18 + TypeScript + Vite
- Desktop shell: Tauri 2
- Runtime: Rust agent runtime
- Markdown rendering: `react-markdown` + `remark-gfm`
- Persistence and diagnostics: local session files, logs, debug surfaces

## 快速开始

前提：
- Node.js
- Rust toolchain
- Tauri 2 构建环境

常用命令：

```bash
npm install
npm run dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

本地开发时：
- 前端 dev server 默认运行在 `http://localhost:1420`
- Tauri 构建产物位于 `src-tauri/target/release/bundle/macos/DeepSeekX.app`

## 仓库结构

- `src/`：前端界面与交互逻辑
- `src-tauri/src/lib.rs`：Tauri commands 与主桥接层
- `src-tauri/src/agent/`：agent runtime、planner、orchestrator、permissions、executor
- `src-tauri/icons/`：桌面图标与打包资产
- `docs/`：开发、agent 运行时和产品设计文档

## 开发文档导航

- [AGENTS.md](/Users/Gress/code/ai/deepseekX/AGENTS.md)
- [CONTRIBUTING.md](/Users/Gress/code/ai/deepseekX/docs/development/CONTRIBUTING.md)
- [ARCHITECTURE.md](/Users/Gress/code/ai/deepseekX/docs/development/ARCHITECTURE.md)
- [RUNTIME.md](/Users/Gress/code/ai/deepseekX/docs/agent/RUNTIME.md)
- [TOOLS.md](/Users/Gress/code/ai/deepseekX/docs/agent/TOOLS.md)
- [DEBUGGING.md](/Users/Gress/code/ai/deepseekX/docs/agent/DEBUGGING.md)

## 当前状态

DeepSeekX 当前处于活跃开发中。仓库里已经接通了桌面聊天、agent 运行时、工具调用、调试链路和一部分工作区能力，但仍有一些能力在持续演进中。

如果某项能力在 README、设计文档和实际代码之间存在差异，应以当前代码实现和仓库内最新稳定文档为准。
