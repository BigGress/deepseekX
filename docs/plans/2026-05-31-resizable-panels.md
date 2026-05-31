# 可拖拽面板尺寸调整 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 ChatPanel（左）和 FilePreviewPanel（右）之间加入可拖拽分隔线，并将面板比例持久化到 localStorage。

**Architecture:** 用 `react-resizable-panels` 的 `PanelGroup` 替换 `App.tsx:613` 的 `flex-1 flex overflow-hidden` 容器。ChatPanel 放入固定 `Panel`，FilePreviewPanel 随预览状态条件渲染；两者之间插入 `PanelResizeHandle` + 自定义视觉组件 `ResizeHandle`。`autoSaveId` 触发 localStorage 自动持久化。

**Tech Stack:** React 18, react-resizable-panels ^2, Tailwind CSS, Vitest + @testing-library/react

---

## 文件改动一览

| 文件 | 类型 | 说明 |
|------|------|------|
| `package.json` | 修改 | 添加 `react-resizable-panels` 依赖 |
| `src/components/ResizeHandle.tsx` | 新增 | 拖拽手柄视觉组件 |
| `src/components/ResizeHandle.test.tsx` | 新增 | ResizeHandle 单元测试 |
| `src/components/FilePreviewPanel.tsx` | 修改 | 去除固定宽度 class，改为填充父容器 |
| `src/App.tsx` | 修改 | 引入 PanelGroup/Panel，替换容器 |

---

## Task 1: 安装 react-resizable-panels

**Files:**
- Modify: `package.json`

- [ ] **Step 1: 安装依赖**

```bash
npm install react-resizable-panels
```

- [ ] **Step 2: 验证安装成功**

```bash
grep '"react-resizable-panels"' package.json
```

Expected output 包含类似 `"react-resizable-panels": "^2.x.x"` 的一行。

- [ ] **Step 3: 确认构建不报错**

```bash
npm run build 2>&1 | tail -5
```

Expected: 无 TS 报错，输出 `built in` 字样。

- [ ] **Step 4: Commit**

```bash
git add package.json package-lock.json
git commit -m "chore: add react-resizable-panels dependency"
```

---

## Task 2: 创建 ResizeHandle 组件（TDD）

**Files:**
- Create: `src/components/ResizeHandle.test.tsx`
- Create: `src/components/ResizeHandle.tsx`

- [ ] **Step 1: 写失败测试**

新建 `src/components/ResizeHandle.test.tsx`：

```tsx
import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import ResizeHandle from "./ResizeHandle";

afterEach(cleanup);

describe("ResizeHandle", () => {
  it("renders a div without crashing", () => {
    const { container } = render(<ResizeHandle />);
    expect(container.firstChild).not.toBeNull();
  });

  it("has cursor-col-resize class", () => {
    const { container } = render(<ResizeHandle />);
    expect(container.firstChild).toHaveClass("cursor-col-resize");
  });

  it("renders the inner visual bar", () => {
    const { container } = render(<ResizeHandle />);
    const inner = container.querySelector(".bg-neutral-700");
    expect(inner).not.toBeNull();
  });
});
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
npx vitest run src/components/ResizeHandle.test.tsx
```

Expected: FAIL — `Cannot find module './ResizeHandle'`

- [ ] **Step 3: 实现组件**

新建 `src/components/ResizeHandle.tsx`：

```tsx
export default function ResizeHandle() {
  return (
    <div className="w-1 h-full cursor-col-resize flex items-center justify-center">
      <div className="w-px h-full bg-neutral-700 hover:bg-blue-500 active:bg-blue-400 transition-colors duration-150" />
    </div>
  );
}
```

- [ ] **Step 4: 运行测试，确认通过**

```bash
npx vitest run src/components/ResizeHandle.test.tsx
```

Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/ResizeHandle.tsx src/components/ResizeHandle.test.tsx
git commit -m "feat: add ResizeHandle visual component"
```

---

## Task 3: 去除 FilePreviewPanel 固定宽度

**Files:**
- Modify: `src/components/FilePreviewPanel.tsx:22`

当前 `FilePreviewPanel` 根 div（第 22 行）带有固定宽度类 `w-80 min-w-[280px] max-w-[480px] shrink-0`，以及左边框 `border-l border-neutral-800`。引入 `PanelGroup` 后，宽度由外层 `Panel` 控制；视觉分隔线由 `ResizeHandle` 提供，不再需要 `border-l`。

- [ ] **Step 1: 修改根 div**

在 `src/components/FilePreviewPanel.tsx` 第 22 行，将：

```tsx
<div className="flex flex-col w-80 min-w-[280px] max-w-[480px] border-l border-neutral-800 bg-neutral-950 shrink-0 h-full overflow-hidden">
```

改为：

```tsx
<div className="flex flex-col w-full bg-neutral-950 h-full overflow-hidden">
```

- [ ] **Step 2: 运行现有测试，确认没有回归**

```bash
npx vitest run src/components/FilePreviewPanel.test.tsx
```

Expected: 所有测试 PASS（测试不依赖布局 class）

- [ ] **Step 3: Commit**

```bash
git add src/components/FilePreviewPanel.tsx
git commit -m "refactor: remove fixed width from FilePreviewPanel for resizable layout"
```

---

## Task 4: 在 App.tsx 中接入 PanelGroup

**Files:**
- Modify: `src/App.tsx:1-10`（import 区）
- Modify: `src/App.tsx:613-635`（布局容器）

- [ ] **Step 1: 添加 import**

在 `src/App.tsx` 顶部现有 import 列表末尾（第 39 行之后）添加：

```tsx
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import ResizeHandle from "./components/ResizeHandle";
```

- [ ] **Step 2: 替换布局容器**

将 `src/App.tsx:613-635` 的：

```tsx
      <div className="flex-1 flex overflow-hidden">
        <ChatPanel
          conversation={activeConversation ?? null}
          isLoading={isLoading}
          onSend={handleSend}
          onAgentFollowUp={handleAgentFollowUp}
          fileNodes={fileNodes}
          skillNames={skillNamesFromProject}
          mcpNames={mcpNamesFromProject}
          debugLlmResponsesEnabled={debugLlmResponses}
          llmDebugEntries={llmDebugEntries}
          onPreviewFile={handlePreviewFile}
        />
        {previewState.isOpen && (
          <FilePreviewPanel
            state={previewState}
            workspaceRoot={selectedProject?.root_path ?? ""}
            onClose={closePreview}
            onSwitchMode={switchPreviewMode}
            onOpenFocused={openFocused}
          />
        )}
      </div>
```

替换为：

```tsx
      <Group
        orientation="horizontal"
        defaultLayout={defaultLayout}
        onLayoutChanged={onLayoutChanged}
        className="flex-1 overflow-hidden"
      >
        <Panel defaultSize={60} minSize={25}>
          <ChatPanel
            conversation={activeConversation ?? null}
            isLoading={isLoading}
            onSend={handleSend}
            onAgentFollowUp={handleAgentFollowUp}
            fileNodes={fileNodes}
            skillNames={skillNamesFromProject}
            mcpNames={mcpNamesFromProject}
            debugLlmResponsesEnabled={debugLlmResponses}
            llmDebugEntries={llmDebugEntries}
            onPreviewFile={handlePreviewFile}
          />
        </Panel>
        {previewState.isOpen && (
          <>
            <Separator id="chat-preview-separator">
              <ResizeHandle />
            </Separator>
            <Panel defaultSize={40} minSize={20}>
              <FilePreviewPanel
                state={previewState}
                workspaceRoot={selectedProject?.root_path ?? ""}
                onClose={closePreview}
                onSwitchMode={switchPreviewMode}
                onOpenFocused={openFocused}
              />
            </Panel>
          </>
        )}
      </Group>
```

- [ ] **Step 3: 运行全量测试，确认无回归**

```bash
npm test
```

Expected: 所有测试 PASS

- [ ] **Step 4: 启动开发服务器，手动验证**

```bash
npm run dev
```

验证清单：
- 打开文件预览后，ChatPanel 和 FilePreviewPanel 之间出现可拖拽手柄
- 拖拽手柄可以改变两侧宽度
- ChatPanel 不能被压到 25% 以下，FilePreviewPanel 不能被压到 20% 以下
- 关闭预览后 ChatPanel 撑满全宽
- 拖拽调整宽度后刷新页面，比例从 localStorage 恢复（检查 key `react-resizable-panels:main-layout`）

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "feat: add resizable split panel between chat and preview"
```
