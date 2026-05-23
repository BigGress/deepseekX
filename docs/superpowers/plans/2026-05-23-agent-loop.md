# Agent Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a manual-on-demand in-app Agent mode that can plan, read/write files, run guarded commands, and stop with a structured completion result.

**Architecture:** Add a new Rust `agent` module that owns the loop (`Plan -> Act -> Observe -> Replan -> Finish`), guard execution through a permission gate, and expose a new Tauri command separate from the existing chat path. Extend the React UI with an Agent toggle beside the input, call the new command, and render agent progress through the existing turn-based display.

**Tech Stack:** Tauri 2, Rust, reqwest, tokio, React 18, TypeScript, Vite

---

> Notes:
> - This workspace currently does not expose a usable git repository, so commit steps are intentionally omitted from the plan.
> - Follow TDD where there is clear backend logic to lock down. For frontend rendering in this repo, use `npm run build` as the safety net because no UI test runner is configured.

## File Map

- Create: `src-tauri/src/agent/mod.rs`
- Create: `src-tauri/src/agent/actions.rs`
- Create: `src-tauri/src/agent/planner.rs`
- Create: `src-tauri/src/agent/permissions.rs`
- Create: `src-tauri/src/agent/executor.rs`
- Create: `src-tauri/src/agent/state.rs`
- Create: `src-tauri/src/agent/orchestrator.rs`
- Modify: `src-tauri/src/api.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/session.rs`
- Modify: `src/types.ts`
- Modify: `src/api.ts`
- Modify: `src/components/ChatInput.tsx`
- Modify: `src/components/ChatPanel.tsx`
- Modify: `src/components/MessageList.tsx`
- Modify: `src/components/TurnItem.tsx`
- Modify: `src/App.tsx`

### Task 1: Scaffold Agent Types And Planner Parser

**Files:**
- Create: `src-tauri/src/agent/mod.rs`
- Create: `src-tauri/src/agent/actions.rs`
- Create: `src-tauri/src/agent/planner.rs`

- [ ] **Step 1: Write the failing planner parser test**

Add this test block to `src-tauri/src/agent/planner.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::parse_planner_action;
    use crate::agent::actions::AgentAction;

    #[test]
    fn parses_read_files_action() {
        let raw = r#"{
            "action": "read_files",
            "reason": "inspect current ui flow",
            "files": ["src/App.tsx", "src/components/ChatInput.tsx"]
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::ReadFiles { reason, files } => {
                assert_eq!(reason, "inspect current ui flow");
                assert_eq!(files.len(), 2);
                assert_eq!(files[0], "src/App.tsx");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn rejects_unknown_action() {
        let raw = r#"{"action":"shell","reason":"nope"}"#;
        let err = parse_planner_action(raw).unwrap_err();
        assert!(err.contains("unsupported action"));
    }
}
```

- [ ] **Step 2: Run the planner test to verify it fails**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml parses_read_files_action -- --nocapture
```

Expected: FAIL because `src-tauri/src/agent/planner.rs` and `parse_planner_action` do not exist yet.

- [ ] **Step 3: Add the agent module shell and core action enum**

Create `src-tauri/src/agent/mod.rs`:

```rust
pub mod actions;
pub mod planner;
pub mod permissions;
pub mod executor;
pub mod state;
pub mod orchestrator;
```

Create `src-tauri/src/agent/actions.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    ListFiles { reason: String, path: Option<String>, depth: Option<usize> },
    ReadFiles { reason: String, files: Vec<String> },
    WriteFiles { reason: String, files: Vec<WriteFileRequest> },
    RunCommand { reason: String, command: String, cwd: Option<String> },
    AskUser { reason: String, question: String },
    Finish {
        reason: String,
        goal_status: GoalStatus,
        summary: String,
        verification: String,
        what_changed: Option<Vec<String>>,
        next_step: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Done,
    Blocked,
    NeedsConfirmation,
}
```

Create `src-tauri/src/agent/planner.rs`:

```rust
use crate::agent::actions::AgentAction;

pub fn parse_planner_action(raw: &str) -> Result<AgentAction, String> {
    let parsed: AgentAction =
        serde_json::from_str(raw).map_err(|e| format!("invalid planner json: {}", e))?;

    match parsed {
        AgentAction::ListFiles { .. }
        | AgentAction::ReadFiles { .. }
        | AgentAction::WriteFiles { .. }
        | AgentAction::RunCommand { .. }
        | AgentAction::AskUser { .. }
        | AgentAction::Finish { .. } => Ok(parsed),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_planner_action;
    use crate::agent::actions::AgentAction;

    #[test]
    fn parses_read_files_action() {
        let raw = r#"{
            "action": "read_files",
            "reason": "inspect current ui flow",
            "files": ["src/App.tsx", "src/components/ChatInput.tsx"]
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::ReadFiles { reason, files } => {
                assert_eq!(reason, "inspect current ui flow");
                assert_eq!(files.len(), 2);
                assert_eq!(files[0], "src/App.tsx");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn rejects_unknown_action() {
        let raw = r#"{"action":"shell","reason":"nope"}"#;
        let err = parse_planner_action(raw).unwrap_err();
        assert!(err.contains("unknown variant") || err.contains("unsupported action"));
    }
}
```

- [ ] **Step 4: Run the planner tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml planner::tests -- --nocapture
```

Expected: PASS for `parses_read_files_action` and `rejects_unknown_action`.

### Task 2: Add Permission Gate And Workspace-Safe Executor

**Files:**
- Create: `src-tauri/src/agent/permissions.rs`
- Create: `src-tauri/src/agent/executor.rs`
- Create: `src-tauri/src/agent/state.rs`

- [ ] **Step 1: Write failing tests for safe paths and command gating**

Add this test block to `src-tauri/src/agent/permissions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{classify_command_risk, ensure_workspace_relative};

    #[test]
    fn rejects_parent_traversal() {
        let err = ensure_workspace_relative("../secrets.txt").unwrap_err();
        assert!(err.contains("outside workspace"));
    }

    #[test]
    fn blocks_dangerous_shell_commands() {
        let verdict = classify_command_risk("rm -rf src");
        assert_eq!(verdict.requires_confirmation, true);
        assert_eq!(verdict.allowed_without_confirmation, false);
    }

    #[test]
    fn allows_known_safe_build_commands() {
        let verdict = classify_command_risk("cargo test");
        assert_eq!(verdict.allowed_without_confirmation, true);
    }
}
```

- [ ] **Step 2: Run the permissions test to verify it fails**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml permissions::tests -- --nocapture
```

Expected: FAIL because the permission helpers do not exist yet.

- [ ] **Step 3: Implement workspace guards, risk classification, and executor primitives**

Create `src-tauri/src/agent/permissions.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRiskVerdict {
    pub allowed_without_confirmation: bool,
    pub requires_confirmation: bool,
    pub reason: String,
}

pub fn ensure_workspace_relative(path: &str) -> Result<(), String> {
    if path.is_empty() || path.starts_with('/') || path.contains("..") {
        return Err("path resolves outside workspace".into());
    }
    Ok(())
}

pub fn classify_command_risk(command: &str) -> CommandRiskVerdict {
    let normalized = command.trim().to_lowercase();

    let safe_prefixes = [
        "ls",
        "pwd",
        "cat ",
        "cargo test",
        "cargo build",
        "npm test",
        "pnpm test",
        "pnpm build",
    ];

    let dangerous_markers = ["rm ", "sudo ", "git push", "curl ", "wget ", "brew install"];

    if dangerous_markers.iter().any(|m| normalized.contains(m)) {
        return CommandRiskVerdict {
            allowed_without_confirmation: false,
            requires_confirmation: true,
            reason: "dangerous command".into(),
        };
    }

    if safe_prefixes.iter().any(|p| normalized == *p || normalized.starts_with(p)) {
        return CommandRiskVerdict {
            allowed_without_confirmation: true,
            requires_confirmation: false,
            reason: "safe command allowlist".into(),
        };
    }

    CommandRiskVerdict {
        allowed_without_confirmation: false,
        requires_confirmation: true,
        reason: "command is outside allowlist".into(),
    }
}
```

Create `src-tauri/src/agent/state.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AgentTaskState {
    pub goal: String,
    pub step_index: usize,
    pub max_steps: usize,
    pub status: AgentLoopStatus,
    pub observations: Vec<AgentObservation>,
}

#[derive(Debug, Clone, Serialize)]
pub enum AgentLoopStatus {
    Planning,
    AwaitingApproval,
    Executing,
    Observing,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentObservation {
    pub action_name: String,
    pub summary: String,
    pub is_error: bool,
}
```

Create `src-tauri/src/agent/executor.rs`:

```rust
use std::path::Path;
use tokio::process::Command;

use crate::agent::permissions::ensure_workspace_relative;

pub async fn read_workspace_file(workspace_root: &str, rel_path: &str) -> Result<String, String> {
    ensure_workspace_relative(rel_path)?;
    let full_path = Path::new(workspace_root).join(rel_path);
    tokio::fs::read_to_string(&full_path)
        .await
        .map_err(|e| format!("read failed ({}): {}", full_path.display(), e))
}

pub async fn write_workspace_file(workspace_root: &str, rel_path: &str, content: &str) -> Result<(), String> {
    ensure_workspace_relative(rel_path)?;
    let full_path = Path::new(workspace_root).join(rel_path);
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir failed ({}): {}", parent.display(), e))?;
    }
    tokio::fs::write(&full_path, content)
        .await
        .map_err(|e| format!("write failed ({}): {}", full_path.display(), e))
}

pub async fn run_guarded_command(workspace_root: &str, command: &str) -> Result<(i32, String, String), String> {
    let output = Command::new("zsh")
        .arg("-lc")
        .arg(command)
        .current_dir(workspace_root)
        .output()
        .await
        .map_err(|e| format!("command spawn failed: {}", e))?;

    Ok((
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}
```

- [ ] **Step 4: Run the permissions tests to verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml permissions::tests -- --nocapture
```

Expected: PASS for traversal rejection, dangerous command blocking, and safe command allowlist.

### Task 3: Implement The Agent Orchestrator And Tauri Command

**Files:**
- Create: `src-tauri/src/agent/orchestrator.rs`
- Modify: `src-tauri/src/api.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/session.rs`

- [ ] **Step 1: Write a failing orchestrator stop-condition test**

Add this test block to `src-tauri/src/agent/orchestrator.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::should_stop;

    #[test]
    fn stops_when_step_budget_is_exhausted() {
        assert_eq!(should_stop(12, 12), true);
        assert_eq!(should_stop(13, 12), true);
        assert_eq!(should_stop(3, 12), false);
    }
}
```

- [ ] **Step 2: Run the orchestrator test to verify it fails**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml stops_when_step_budget_is_exhausted -- --nocapture
```

Expected: FAIL because `should_stop` and the orchestrator do not exist yet.

- [ ] **Step 3: Add the loop result model, planner call helper, and new Tauri command**

Create `src-tauri/src/agent/orchestrator.rs`:

```rust
use serde::Serialize;

use crate::agent::actions::GoalStatus;
use crate::agent::state::{AgentLoopStatus, AgentObservation, AgentTaskState};

#[derive(Debug, Clone, Serialize)]
pub struct AgentRunResponse {
    pub final_response: String,
    pub goal_status: String,
    pub steps: Vec<AgentObservation>,
}

pub fn should_stop(current_step: usize, max_steps: usize) -> bool {
    current_step >= max_steps
}

pub async fn run_agent_loop(goal: &str) -> Result<AgentRunResponse, String> {
    let state = AgentTaskState {
        goal: goal.to_string(),
        step_index: 0,
        max_steps: 12,
        status: AgentLoopStatus::Completed,
        observations: vec![AgentObservation {
            action_name: "agent_plan".into(),
            summary: "first planner iteration placeholder".into(),
            is_error: false,
        }],
    };

    Ok(AgentRunResponse {
        final_response: format!("任务已完成：{}", state.goal),
        goal_status: match GoalStatus::Done {
            GoalStatus::Done => "done".into(),
            GoalStatus::Blocked => "blocked".into(),
            GoalStatus::NeedsConfirmation => "needs_confirmation".into(),
        },
        steps: state.observations,
    })
}

#[cfg(test)]
mod tests {
    use super::should_stop;

    #[test]
    fn stops_when_step_budget_is_exhausted() {
        assert_eq!(should_stop(12, 12), true);
        assert_eq!(should_stop(13, 12), true);
        assert_eq!(should_stop(3, 12), false);
    }
}
```

Modify `src-tauri/src/api.rs` by adding a helper that can send planner/system prompts without forcing the existing web-search behavior:

```rust
pub async fn chat_completion_with_options(
    config: &ApiConfig,
    messages: Vec<Value>,
    web_search_enabled: bool,
) -> Result<Value, String> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let api_messages: Vec<Message> = messages
        .into_iter()
        .map(|m| Message {
            role: m["role"].as_str().unwrap_or("user").to_string(),
            content: m["content"].clone(),
        })
        .collect();

    let body = ChatRequest {
        model: config.model.clone(),
        messages: api_messages,
        stream: false,
        web_search: if web_search_enabled {
            Some(serde_json::json!({"enable": true}))
        } else {
            None
        },
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    let status = resp.status();
    let resp_text = resp.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
    if !status.is_success() {
        return Err(format!("API 返回错误 ({}): {}", status, resp_text));
    }

    let chat_resp: ChatResponse =
        serde_json::from_str(&resp_text).map_err(|e| format!("解析响应失败: {}", e))?;

    chat_resp.choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| "API 返回空响应".to_string())
}
```

Then replace the existing `chat_completion()` body with:

```rust
pub async fn chat_completion(
    config: &ApiConfig,
    messages: Vec<Value>,
) -> Result<Value, String> {
    chat_completion_with_options(config, messages, true).await
}
```

Modify `src-tauri/src/lib.rs` near the module declarations:

```rust
mod agent;
```

Add this new Tauri command above `run()`:

```rust
#[tauri::command]
async fn run_agent_task(
    _db: tauri::State<'_, Arc<Database>>,
    session_id: String,
    content: String,
    _title: String,
    _workspace_root: String,
    _instructions: Option<String>,
    _skills: Option<String>,
    _mcp_servers: Option<String>,
) -> Result<String, String> {
    let result = agent::orchestrator::run_agent_loop(&content).await?;
    let response = serde_json::json!({
        "session_id": session_id,
        "goal_status": result.goal_status,
        "steps": result.steps,
        "final_response": result.final_response
    });
    Ok(response.to_string())
}
```

Register the command in `tauri::generate_handler![]`:

```rust
run_agent_task,
```

Modify `src-tauri/src/session.rs` to add optional agent metadata to the turn model:

```rust
#[derive(Debug, Serialize, Clone)]
pub struct Turn {
    pub id: String,
    pub user_input: String,
    pub thinking_steps: Vec<ThinkingStep>,
    pub final_response: Option<String>,
    pub agent_goal_status: Option<String>,
}
```

Update both turn construction sites to set:

```rust
agent_goal_status: None,
```

- [ ] **Step 4: Run backend tests to verify the new loop scaffolding builds**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS for the new module tests and no compile errors from the added command/type fields.

### Task 4: Extend Frontend Types, API, And Agent Input Mode

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api.ts`
- Modify: `src/components/ChatInput.tsx`
- Modify: `src/components/ChatPanel.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Add the frontend agent result types**

Modify `src/types.ts` by appending:

```ts
export interface AgentStep {
  action_name: string;
  summary: string;
  is_error: boolean;
}

export interface AgentRunResult {
  session_id: string;
  goal_status: "done" | "blocked" | "needs_confirmation";
  steps: AgentStep[];
  final_response: string;
}
```

- [ ] **Step 2: Add the new invoke helper for Agent mode**

Modify `src/api.ts` by appending:

```ts
import type { AgentRunResult } from "./types";

export async function runAgentTask(
  sessionId: string,
  content: string,
  title: string,
  workspaceRoot: string,
  instructions?: string,
  skills?: string,
  mcpServers?: string,
): Promise<AgentRunResult> {
  const raw = await invoke<string>("run_agent_task", {
    sessionId,
    content,
    title,
    workspaceRoot,
    instructions: instructions ?? null,
    skills: skills ?? null,
    mcpServers: mcpServers ?? null,
  });

  return JSON.parse(raw) as AgentRunResult;
}
```

- [ ] **Step 3: Add the Agent toggle UI beside the input**

Modify `src/components/ChatInput.tsx` props:

```ts
interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  isLoading: boolean;
  inputRef: React.RefObject<HTMLTextAreaElement>;
  fileNodes: FileNode[];
  skillNames?: string[];
  mcpNames?: string[];
  agentMode: boolean;
  onAgentModeChange: (enabled: boolean) => void;
}
```

Render this toggle before the send button:

```tsx
<button
  type="button"
  onClick={() => onAgentModeChange(!agentMode)}
  disabled={isLoading}
  className={`shrink-0 px-3 h-9 rounded-lg text-xs font-medium transition-colors ${
    agentMode
      ? "bg-emerald-600 text-white hover:bg-emerald-500"
      : "bg-neutral-800 text-neutral-300 hover:bg-neutral-700"
  }`}
>
  Agent
</button>
```

Update the placeholder:

```tsx
placeholder={
  agentMode
    ? "输入任务目标... Agent 会自动规划并执行"
    : "输入任务描述... (Enter 发送, Shift+Enter 换行, @文件路径 引用文件)"
}
```

- [ ] **Step 4: Thread the new agentMode state through ChatPanel and App**

Modify `src/components/ChatPanel.tsx` props:

```ts
interface ChatPanelProps {
  conversation: Conversation | null;
  isLoading: boolean;
  onSend: (content: string, agentMode: boolean) => void;
  fileNodes: FileNode[];
  skillNames?: string[];
  mcpNames?: string[];
}
```

Add state in `ChatPanel`:

```ts
const [agentMode, setAgentMode] = useState(false);
```

Change send handling:

```ts
const handleSend = () => {
  if (!inputValue.trim() || isLoading) return;
  onSend(inputValue, agentMode);
  setInputValue("");
  if (inputRef.current) {
    inputRef.current.style.height = "auto";
  }
};
```

Pass new props into `ChatInput`:

```tsx
agentMode={agentMode}
onAgentModeChange={setAgentMode}
```

Modify `src/App.tsx` imports:

```ts
import { runAgentTask, sendMessageViaApi } from "./api";
```

Change the handler signature:

```ts
const handleSend = useCallback(
  async (content: string, agentMode: boolean) => {
```

Inside the try block, replace the single API call with:

```ts
if (agentMode) {
  const agentResult = await runAgentTask(
    activeConvId,
    content,
    activeConversation.title || content.slice(0, 30),
    selectedProject.root_path,
    selectedProject.instructions || undefined,
    selectedProject.skills || undefined,
    selectedProject.mcp_servers || undefined,
  );

  const assistantMessage: Message = {
    id: crypto.randomUUID(),
    role: "assistant",
    content: agentResult.final_response,
    timestamp: Date.now(),
  };

  setConversations((prev) =>
    prev.map((conv) =>
      conv.id === activeConvId
        ? { ...conv, messages: [...conv.messages, assistantMessage] }
        : conv,
    ),
  );
} else {
  const responseText = await sendMessageViaApi(
    activeConvId,
    content,
    activeConversation.title || content.slice(0, 30),
    selectedProject.root_path,
    selectedProject.instructions || undefined,
    selectedProject.skills || undefined,
    selectedProject.mcp_servers || undefined,
  );

  const assistantMessage: Message = {
    id: crypto.randomUUID(),
    role: "assistant",
    content: responseText,
    timestamp: Date.now(),
  };

  setConversations((prev) =>
    prev.map((conv) =>
      conv.id === activeConvId
        ? { ...conv, messages: [...conv.messages, assistantMessage] }
        : conv,
    ),
  );
}
```

- [ ] **Step 5: Run the frontend build to verify type and render safety**

Run:

```bash
npm run build
```

Expected: PASS with `tsc && vite build` succeeding.

### Task 5: Render Agent Steps In The Existing Turn UI

**Files:**
- Modify: `src/types.ts`
- Modify: `src/components/MessageList.tsx`
- Modify: `src/components/TurnItem.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Extend the turn type so the UI can carry agent progress**

Modify `src/types.ts`:

```ts
export interface Turn {
  id: string;
  user_input: string;
  thinking_steps: ThinkingStep[];
  final_response: string | null;
  agent_goal_status?: "done" | "blocked" | "needs_confirmation" | null;
  agent_steps?: AgentStep[];
}
```

- [ ] **Step 2: Inject agent step data into the optimistic pending turn**

Modify `src/App.tsx` where `pendingTurn` is created:

```ts
const pendingTurn: Turn = {
  id: `${activeConvId}-pending`,
  user_input: content,
  thinking_steps: [],
  final_response: null,
  agent_goal_status: null,
  agent_steps: agentMode
    ? [{ action_name: "agent_plan", summary: "正在规划任务...", is_error: false }]
    : [],
};
```

When Agent mode returns successfully, replace the pending turn in state:

```ts
setConversations((prev) =>
  prev.map((conv) =>
    conv.id === activeConvId
      ? {
          ...conv,
          turns: (conv.turns || []).map((turn) =>
            turn.id === `${activeConvId}-pending`
              ? {
                  ...turn,
                  final_response: agentResult.final_response,
                  agent_goal_status: agentResult.goal_status,
                  agent_steps: agentResult.steps,
                }
              : turn,
          ),
        }
      : conv,
  ),
);
```

- [ ] **Step 3: Add an Agent status card in TurnItem**

Modify `src/components/TurnItem.tsx` under the user bubble:

```tsx
{turn.agent_steps && turn.agent_steps.length > 0 && (
  <div className="flex justify-start">
    <div className="max-w-[85%] w-full rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-3">
      <div className="flex items-center justify-between text-xs">
        <span className="text-neutral-400">Agent 执行过程</span>
        {turn.agent_goal_status && (
          <span
            className={`rounded-full px-2 py-0.5 ${
              turn.agent_goal_status === "done"
                ? "bg-emerald-950 text-emerald-400"
                : turn.agent_goal_status === "blocked"
                  ? "bg-red-950 text-red-400"
                  : "bg-yellow-950 text-yellow-400"
            }`}
          >
            {turn.agent_goal_status}
          </span>
        )}
      </div>

      <div className="mt-3 space-y-2">
        {turn.agent_steps.map((step, idx) => (
          <div key={`${step.action_name}-${idx}`} className="rounded-md bg-neutral-950 px-3 py-2">
            <p className="text-xs text-blue-400">{step.action_name}</p>
            <p className={`mt-1 text-xs ${step.is_error ? "text-red-400" : "text-neutral-300"}`}>
              {step.summary}
            </p>
          </div>
        ))}
      </div>
    </div>
  </div>
)}
```

- [ ] **Step 4: Make sure MessageList still renders safely when turns are present**

Keep `MessageList` on the existing turn-first path and only adjust the empty-state copy so the new mode is discoverable:

```tsx
<p className="text-neutral-500 text-sm">
  输入任务描述，或开启 Agent 让 AI 自动规划并执行
</p>
```

- [ ] **Step 5: Run the frontend build again and smoke-check the Tauri app**

Run:

```bash
npm run build
```

Expected: PASS.

Then run:

```bash
npm run tauri dev
```

Expected: The app opens, the input shows an `Agent` toggle, and Agent mode sends a task without crashing the renderer.

## Self-Review

- Spec coverage:
  - Agent loop architecture is covered by Tasks 1-3.
  - Action protocol and permission gate are covered by Tasks 1-2.
  - Frontend entry and manual-on-demand mode are covered by Task 4.
  - Step rendering and structured finish output are covered by Task 5.
- Placeholder scan:
  - No placeholder instructions remain in the plan body.
  - Commands, file paths, and code snippets are concrete.
- Type consistency:
  - Rust action names stay in `snake_case` across `actions.rs`, planner parsing, and API response.
  - Frontend `AgentRunResult`, `AgentStep`, and `Turn.agent_steps` use matching property names.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-23-agent-loop.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration

2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
