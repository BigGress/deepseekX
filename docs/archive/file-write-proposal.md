# 技术提案：GUI 自动写入文件到本地磁盘

> 日期：2026-05-23  
> 状态：待 review  
> 版本：v1

---

## 0. 问题分析

### 0.1 当前行为

用户通过 DeepSeekX GUI 向 AI 发送"帮我初始化一个 golang 项目"，AI 回复：

> 我无法直接写入文件到你的磁盘，但我已经把完整的文件内容列在下面了。

### 0.2 根因

system prompt 中存在自相矛盾的两句话：

```
你对该目录有完整的读写权限。  ← AI 被告知"可以写"
你是一个纯文本模型，没有工具调用能力。 ← AI 被告知"不能写"
```

AI 正确判断自己只是文本模型，无法实际操作文件系统。即使知道目标目录，
也只能输出代码块，不会真正创建文件。

### 0.3 核心矛盾

DeepSeekX GUI 使用 API 模式（chat/completions），API 返回纯文本。
不像 TUI CLI 模式有工具调用能力。用户期望 GUI「能做事」，但当前只是「能说话」。

**解决方向**：不是让 AI 相信自己能写文件，而是让 GUI 在 AI 回复后真的去写文件。

---

## 1. 设计方案

### 1.1 核心思路

AI 回复中的代码块使用约定格式标注文件路径，GUI 端解析后自动写入磁盘。

```
用户: "帮我初始化一个 golang 项目"
  ↓
AI 回复（含约定格式）:
  我会创建以下文件：

  ```go file:main.go
  package main
  func main() { ... }
  ```

  ```text file:go.mod
  module myproject
  go 1.21
  ```
  ↓
GUI 解析 → 写入 workspace_root/main.go, workspace_root/go.mod
  ↓
用户看到回复 + "✅ 已写入 2 个文件"
```

### 1.2 文件标记格式

在代码块的语言标识符后添加 `file:` 前缀：

~~~
```go file:main.go
package main
...
```
~~~

格式规则：
- `file:` 后跟相对于 workspace_root 的路径
- 路径不能包含 `..`（安全防护）
- 语言标识符可选，纯文本用 `text` 或省略
- 标记出现在代码块第一行末尾

正则：`^```(?:\w+)?\s*file:(.+?)\s*$`

### 1.3 写入流程

```
send_message_via_api()
  │
  ├─ 4. 发送 API 请求 → 获取 assistant_content
  │
  ├─ 5. 解析 assistant_content 中的 file: 代码块
  │      │
  │      ├─ 提取 (相对路径, 代码内容) 对
  │      ├─ 安全检查：路径不含 ".."，不以 "/" 开头
  │      ├─ 拼接 workspace_root + 相对路径
  │      ├─ 创建父目录（如需要）
  │      └─ 写入文件
  │
  ├─ 6. 如果写入了文件，在回复末尾追加摘要
  │      "---\n✅ 已自动写入 2 个文件:\n- main.go\n- go.mod"
  │
  └─ 7. 返回文本（含文件摘要）给前端
```

### 1.4 安全约束

| 规则 | 说明 |
|------|------|
| 禁止 `..` | 路径中不得包含 `..`，防止目录穿越 |
| 禁止绝对路径 | `file:` 后的路径不接受 `/` 开头 |
| 仅文本内容 | 只写入代码块内容，不执行命令 |
| 工作目录内 | 所有路径相对于 workspace_root |

### 1.5 system prompt 更新

将当前矛盾的"能力说明"替换为统一表述：

```
## 文件写入能力

你可以通过代码块标记向工作目录写入文件。使用格式：
```lang file:相对路径
文件内容
```

GUI 会自动将标记了 file: 的代码块保存到工作目录下的对应路径。
你不需要说"我无法写入"，直接使用 file: 标记即可。
```

---

## 2. 实施清单

### Phase 1 — 后端解析 + 写入

- [ ] 1.1 `api.rs` 新增 `parse_file_blocks(text) -> Vec<(String, String)>`
- [ ] 1.2 `api.rs` 新增 `write_files_to_disk(root, files) -> Result<Vec<String>, String>`
- [ ] 1.3 `lib.rs` `send_message_via_api` 中调用解析和写入，追加摘要到返回文本
- [ ] 1.4 更新 system prompt

### Phase 2 — 验证

- [ ] 2.1 `cargo build` 零错误
- [ ] 2.2 单元测试解析逻辑（模拟 AI 回复）
- [ ] 2.3 完整流程测试：发送 golang 初始化请求 → 确认文件实际写入磁盘

---

## 3. 函数签名

```rust
/// 从 AI 回复文本中提取 file: 标记的代码块
/// 返回 Vec<(文件相对路径, 代码内容)>
fn parse_file_blocks(text: &str) -> Vec<(String, String)>;

/// 将文件写入工作目录，返回成功写入的文件名列表
fn write_files_to_disk(
    workspace_root: &str,
    files: &[(String, String)],
) -> Result<Vec<String>, String>;
```

---

## 4. 验收标准

- [ ] AI 回复中的 `file:path` 代码块自动写入磁盘
- [ ] 路径含 `..` 时拒绝写入
- [ ] 无 `file:` 标记的普通对话不受影响
- [ ] `cargo build` 零错误
- [ ] 写入成功后回复末尾显示文件清单
