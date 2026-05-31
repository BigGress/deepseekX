# 可拖拽面板尺寸调整

**日期:** 2026-05-31  
**状态:** 已批准

## 需求

用户希望在主工作区界面中，左侧对话区域（ChatPanel）和右侧文件预览区域（FilePreviewPanel）之间可以通过拖拽分隔线来调整各自的宽度，且调整后的比例在刷新或重启后能自动恢复。

## 方案选择

使用 `react-resizable-panels` 库（方案 A），而非自定义拖拽 hook（方案 B）。

**理由：** 该库内置持久化、键盘访问性、边界约束和窗口 resize 处理，减少手写边缘情况代码。

## 架构

### 当前布局（App.tsx:613）

```
<div className="flex-1 flex overflow-hidden">
  <ChatPanel ... />
  {previewState.isOpen && <FilePreviewPanel ... />}
</div>
```

### 目标布局

```
<PanelGroup direction="horizontal" id="main-layout" storage={localStorageAdapter}>
  <Panel id="chat" defaultSize={60} minSize={25}>
    <ChatPanel ... />
  </Panel>

  {previewState.isOpen && (
    <>
      <PanelResizeHandle className="...">
        <ResizeHandle />
      </PanelResizeHandle>
      <Panel id="preview" defaultSize={40} minSize={20}>
        <FilePreviewPanel ... />
      </Panel>
    </>
  )}
</PanelGroup>
```

## 组件设计

### `src/components/ResizeHandle.tsx`（新增）

封装拖拽手柄的视觉样式：
- 宽度 4px 的竖线区域，中间有 2px 的可见线
- 默认颜色：`bg-neutral-700`
- hover / active 时：`bg-blue-500`
- 光标：`cursor-col-resize`

### `App.tsx` 修改

- 导入 `PanelGroup`、`Panel`、`PanelResizeHandle`
- 将 `flex-1 flex overflow-hidden` 的 div 替换为 `<PanelGroup>`
- ChatPanel 放入 `<Panel id="chat">`
- 预览面板条件块增加 `<PanelResizeHandle>` 和 `<Panel id="preview">`

## 持久化

传入符合 `react-resizable-panels` 接口的 localStorage adapter：

```ts
const localStorageAdapter = {
  getItem: (key: string) => localStorage.getItem(key),
  setItem: (key: string, value: string) => localStorage.setItem(key, value),
};
```

存储 key 为 `"main-layout"`（由 PanelGroup 的 `id` prop 决定）。

## 尺寸约束

| 面板 | 默认比例 | 最小比例 |
|------|----------|----------|
| ChatPanel | 60% | 25% |
| FilePreviewPanel | 40% | 20% |

## 改动文件

| 文件 | 类型 | 说明 |
|------|------|------|
| `package.json` | 修改 | 添加 `react-resizable-panels` 依赖 |
| `src/App.tsx` | 修改 | 替换容器，引入 PanelGroup/Panel |
| `src/components/ResizeHandle.tsx` | 新增 | 拖拽手柄样式组件 |

## 测试要点

- 拖拽手柄可以改变左右面板宽度
- 面板不能被拖拽超过最小宽度限制
- 关闭预览后 ChatPanel 自动撑满全宽
- 重新打开预览后宽度从 localStorage 恢复（若有记录）
- 刷新页面后比例恢复
