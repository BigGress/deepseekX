# 技术提案：项目级 Skill 和 MCP 配置

> 日期：2026-05-23  
> 状态：待 review  
> 版本：v2 — 新增斜杠命令唤起

---

## 0. 问题分析

### 0.1 当前状态

- TUI 支持 `~/.deepseek/config.toml` 中的 `[projects."/path"]` 项目级配置
- Skills 存放在 `~/.deepseek/skills/` 和 `~/.agents/skills/`（共 21 个）
- MCP 服务器在 `~/.deepseek/config.toml` 的 `[mcp.servers]` 中定义
- GUI 项目表目前只有：`name, description, root_path, instructions, model, pinned_files`
- 缺少：每项目独立选择 skill 和 MCP 的能力

### 0.2 用户需求

每个项目应该能独立配置：
- **Skills**：启用哪些 skill（如 `pdf`, `spreadsheets`, `mcp-builder`）
- **MCP**：关联哪些 MCP 服务器（如 `filesystem`, `github`）

这些配置影响 AI 的能力范围——例如 pdf 项目启用 `pdf` skill，数据分析项目启用 `spreadsheets` skill。

---

## 1. 设计方案

### 1.1 数据模型

projects 表新增 2 个列：

```sql
ALTER TABLE projects ADD COLUMN skills TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN mcp_servers TEXT NOT NULL DEFAULT '';
```

- `skills`：JSON 数组字符串，如 `["pdf", "spreadsheets", "feishu"]`
- `mcp_servers`：JSON 数组字符串，如 `["filesystem", "github"]`

### 1.2 配置界面

在 `ProjectSettings` 对话框中新增两个面板：

```
+------------------------------------------+
|  项目设置                    [x]         |
+------------------------------------------+
|  项目名称: [my-project          ]        |
|  项目描述: [                    ]        |
|  默认模型: [deepseek-v4-pro    v]        |
|  自定义指令:                             |
|  [                                    ]  |
|                                          |
|  --- Skills ---                          |
|  [x] pdf          [ ] spreadsheets       |
|  [x] feishu       [ ] documents          |
|  [ ] mcp-builder   ...                   |
|                                          |
|  --- MCP 服务器 ---                      |
|  [x] filesystem   [ ] github             |
|  [x] postgres     [ ] slack              |
|                                          |
|            [取消]  [保存]                |
+------------------------------------------+
```

### 1.3 Skill 列表获取

后端新增 `list_available_skills` command，扫描以下目录：
- `~/.deepseek/skills/`
- `~/.agents/skills/`

返回 `SkillInfo[]`：`{ name, description, path }`，从各 skill 的 `SKILL.md` 头部提取描述。

### 1.4 MCP 服务器列表获取

后端新增 `list_available_mcp_servers` command，读取 `~/.deepseek/config.toml` 中 `[mcp.servers]` 定义的服务器名称列表。

### 1.5 System Prompt 注入

在 `send_message_via_api` 中，根据项目配置的 skills，读取对应 `SKILL.md` 内容并注入 system prompt：

```
## 项目技能

以下技能已启用：

### pdf
{SKILL.md 摘要内容}

### spreadsheets
{SKILL.md 摘要内容}

## MCP 服务器

以下 MCP 服务器可用：
- filesystem
- github
```

---

### 1.6 斜杠命令唤起（类似 Cursor）

输入框支持 `/` 触发命令面板，快速插入 skill 和 MCP 引用。

**触发方式**：
- 在输入框中输入 `/` 弹出命令面板
- 继续输入可过滤匹配的命令
- `↑` `↓` 选择，`Enter` / `Tab` 确认
- `Escape` 关闭面板

**命令格式**：

| 前缀 | 示例 | 说明 |
|------|------|------|
| `/skill` | `/skill pdf` | 引用已启用的 skill |
| `/mcp` | `/mcp filesystem` | 引用已配置的 MCP 服务器 |
| `/file` | `/file src/main.ts` | 引用项目文件（已有 `@` 路径补全，`/` 作为快捷方式） |

**自动补全面板**：

```
+------------------------------------------+
|  输入: /skill pd                          |
+------------------------------------------+
|  ┌──────────────────────────────────────┐ |
|  │ /skill pdf          读取/创建 PDF     │ |  ← 高亮匹配项
|  │ /skill presentations  创建 PPT        │ |
|  └──────────────────────────────────────┘ |
+------------------------------------------+
```

**实现要点**：
- 复用 ChatInput 现有的自动补全框架（已有 `@` 文件补全）
- 检测输入框光标前的 `/` 触发
- 选择后替换 `/` 及后续文本为完整命令（如 `/skill pdf`）

**数据来源（核心）**：
```
list_available_skills()  ← 扫描 ~/.deepseek/skills/ + ~/.agents/skills/
         ↓
   本地已安装的全部 skill
         ↓
   取交集：项目配置中启用的 ∩ 本地真实存在的
         ↓
   ChatInput 斜杠命令面板显示
```

- **第一步**：后端 `list_available_skills` 扫描本地磁盘，返回所有已安装 skill 的 `{ name, description, path }`
- **第二步**：项目配置 `skills` 列（JSON 数组）存储该项目启用的 skill 名称
- **第三步**：斜杠面板只显示**项目已启用 + 本地已验证存在**的 skill（取交集，防止项目配置引用已删除的 skill）
- MCP 同理：`list_available_mcp_servers` 读取 `config.toml` 返回可用列表，与项目 `mcp_servers` 取交集

---

## 2. 实施清单

### Phase 1 — 数据库 + 后端类型

- [ ] 1.1 `db.rs` projects 表新增 `skills` 和 `mcp_servers` 列（带兼容迁移）
- [ ] 1.2 `db.rs` `ProjectRow` 新增字段，更新所有 CRUD 方法
- [ ] 1.3 `session.rs` 新增 `list_available_skills()` 函数
- [ ] 1.4 `session.rs` / `api.rs` 新增 `list_available_mcp_servers()` 函数

### Phase 2 — Tauri Commands

- [ ] 2.1 `lib.rs` 新增 `list_available_skills` command
- [ ] 2.2 `lib.rs` 新增 `list_available_mcp_servers` command
- [ ] 2.3 更新 `create_project` / `update_project` 支持 skills 和 mcp_servers
- [ ] 2.4 `send_message_via_api` 中根据项目配置注入 skills/MCP 到 system prompt

### Phase 3 — 前端配置界面 + 斜杠命令

- [ ] 3.1 `types.ts` 新增 `SkillInfo`, `McpServerInfo`, `SlashCommand` 类型
- [ ] 3.2 `api.ts` 新增 `listAvailableSkills()`, `listAvailableMcpServers()`
- [ ] 3.3 `ProjectSettings.tsx` 新增 Skills 和 MCP 勾选面板
- [ ] 3.4 `ChatInput.tsx` 新增 `/` 斜杠命令触发 + 自动补全面板
- [ ] 3.5 `App.tsx` / `ChatPanel.tsx` 传递项目 skills/mcp 到 ChatInput
- [ ] 3.6 `App.tsx` 更新项目创建/保存逻辑传递新字段

### Phase 4 — 验证

- [ ] 4.1 `cargo build` + `tsc --noEmit` 零错误
- [ ] 4.2 创建项目时选择 skills，确认 system prompt 包含 skill 内容
- [ ] 4.3 旧项目（无 skills/mcp_servers 字段）兼容运行

---

## 3. 关键类型定义

```typescript
// types.ts 新增

export interface SkillInfo {
  name: string;
  description: string;
  path: string;
}

export interface McpServerInfo {
  name: string;
  command: string;
  description: string;
}

export interface SlashCommand {
  prefix: string;       // "/skill" | "/mcp" | "/file"
  name: string;         // skill 名称或 mcp 服务器名
  description: string;  // 简短描述
}

```rust
// db.rs ProjectRow 新增字段
pub struct ProjectRow {
    // ... 现有字段
    pub skills: String,        // JSON 数组字符串
    pub mcp_servers: String,   // JSON 数组字符串
}
```

---

## 4. 验收标准

- [ ] 项目设置中可勾选 skills 和 MCP 服务器
- [ ] skills 的 SKILL.md 内容被注入到 system prompt
- [ ] 不同项目可配置不同的 skills/MCP 组合
- [ ] 旧项目（无新字段）正常加载不报错
- [ ] `cargo build` + `tsc --noEmit` 零错误
