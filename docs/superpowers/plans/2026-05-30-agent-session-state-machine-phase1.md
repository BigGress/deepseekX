# Agent Session State Machine Phase 1 Implementation Plan

> 状态：Implemented（代码与验证已完成；未执行 git checkpoint 步骤）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a session-scoped internal agent state machine that the runtime maintains automatically for `facts + phase`, restores from the current session file, and injects into planner context to improve decision stability without exposing it in the user-facing UI.

**Architecture:** Implement Phase 1 only from the approved spec. Add the state machine types in `state.rs`, wire them into `AgentTaskState`, update them from orchestrator action outcomes, serialize them into session files, restore them on future runs for the same conversation, and inject a compressed state summary into the planner prompt. Do not add LLM-authored `state_update` fields yet.

**Tech Stack:** Rust, Serde, Tauri backend, existing agent runtime in `src-tauri/src/agent`, session persistence in `src-tauri/src/session.rs`

---

## File Structure

- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/state.rs`
  - Add `AgentSessionStateMachine`, `AgentPhase`, `AgentFacts`, `AgentWorkingState`, `AgentStateTimestamps`
  - Add unit tests for serde and defaults
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/session.rs`
  - Persist optional `agent_session_state` in session file metadata layer
  - Add read/write compatibility helpers
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/planner.rs`
  - Extend `PlannerRequest` with `session_state_summary`
  - Inject compressed state summary into planner user prompt
  - Add prompt-level test coverage
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/orchestrator.rs`
  - Initialize session state machine
  - Restore prior state from session context
  - Update `facts + phase` from action outcomes
  - Emit updated state back in response for persistence
  - Add state transition and summary tests
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/mod.rs`
  - Re-export any new types if current module pattern requires it
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs`
  - Load session-scoped state before `run_agent_loop`
  - Persist updated state after successful agent run
  - Keep UI payload unchanged

---

### Task 1: Add state machine types and defaults

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/state.rs`
- Test: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/state.rs`

- [x] **Step 1: Write failing tests for the new phase enum and default state shape**

```rust
#[test]
fn session_state_defaults_to_idle_and_empty_facts() {
    let state = AgentSessionStateMachine::new("conversation-1".into());

    assert_eq!(state.phase, AgentPhase::Idle);
    assert_eq!(state.version, 1);
    assert!(state.facts.verified_sources.is_empty());
    assert!(state.facts.retrieved_refs.is_empty());
    assert!(state.facts.verification_evidence.is_empty());
    assert!(state.facts.applied_file_operations.is_empty());
    assert!(state.facts.knowledge_gaps.is_empty());
    assert!(state.working_state.open_questions.is_empty());
    assert!(state.working_state.next_plan.is_empty());
}

#[test]
fn serializes_agent_phase_as_snake_case() {
    let value = serde_json::to_string(&AgentPhase::AwaitingConfirmation).unwrap();
    assert_eq!(value, "\"awaiting_confirmation\"");
}
```

- [x] **Step 2: Run the focused Rust test target to verify the new tests fail**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml session_state_defaults_to_idle_and_empty_facts -- --nocapture
```

Expected:

- Compile error for missing `AgentSessionStateMachine` / `AgentPhase`

- [x] **Step 3: Add the new state machine structs and constructor**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    Idle,
    Retrieving,
    Reading,
    Verifying,
    Synthesizing,
    AwaitingConfirmation,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentFacts {
    pub verified_sources: Vec<String>,
    pub retrieved_refs: Vec<String>,
    pub verification_evidence: Vec<String>,
    pub source_usage: BTreeMap<String, usize>,
    pub applied_file_operations: Vec<FileOperationResult>,
    pub knowledge_gaps: Vec<String>,
    pub last_error: Option<String>,
    pub last_confirmation_request: Option<String>,
    pub last_completed_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentWorkingState {
    pub working_hypothesis: Option<String>,
    pub open_questions: Vec<String>,
    pub next_plan: Vec<String>,
    pub draft_summary: Option<String>,
    pub confidence_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateTimestamps {
    pub updated_at: String,
    pub facts_updated_at: Option<String>,
    pub working_state_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionStateMachine {
    pub conversation_id: String,
    pub version: u32,
    pub phase: AgentPhase,
    pub facts: AgentFacts,
    pub working_state: AgentWorkingState,
    pub timestamps: AgentStateTimestamps,
}

impl AgentSessionStateMachine {
    pub fn new(conversation_id: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            conversation_id,
            version: 1,
            phase: AgentPhase::Idle,
            facts: AgentFacts::default(),
            working_state: AgentWorkingState::default(),
            timestamps: AgentStateTimestamps {
                updated_at: now,
                facts_updated_at: None,
                working_state_updated_at: None,
            },
        }
    }
}
```

- [x] **Step 4: Run the focused tests and verify they pass**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml serializes_agent_phase_as_snake_case session_state_defaults_to_idle_and_empty_facts -- --nocapture
```

Expected:

- Both tests pass

- [x] **Step 5: Commit the state model checkpoint（按当前任务范围未执行 git checkpoint）**

```bash
git add src-tauri/src/agent/state.rs
git commit -m "feat: add agent session state machine types"
```

---

### Task 2: Persist session state in session files

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/session.rs`
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/state.rs`
- Test: `/Users/Gress/code/ai/deepseekX/src-tauri/src/session.rs`

- [x] **Step 1: Write failing compatibility tests for optional session state persistence**

```rust
#[test]
fn session_file_deserializes_without_agent_session_state() {
    let raw = r#"{
      "metadata": {"id":"conv-1","title":"Example"},
      "system_prompt": null,
      "messages": []
    }"#;

    let parsed: SessionFile = serde_json::from_str(raw).unwrap();
    assert!(parsed.agent_session_state.is_none());
}

#[test]
fn session_file_round_trips_agent_session_state() {
    let file = SessionFile {
        schema_version: Some(1),
        metadata: SessionMetadata {
            id: Some("conv-1".into()),
            title: Some("Example".into()),
            created_at: None,
            updated_at: None,
            message_count: None,
            total_tokens: None,
            model: None,
            workspace: None,
            mode: None,
        },
        system_prompt: None,
        messages: vec![],
        agent_session_state: Some(AgentSessionStateMachine::new("conv-1".into())),
    };

    let encoded = serde_json::to_string(&file).unwrap();
    let decoded: SessionFile = serde_json::from_str(&encoded).unwrap();
    assert!(decoded.agent_session_state.is_some());
}
```

- [x] **Step 2: Run the focused tests to verify the new field is missing**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml session_file_round_trips_agent_session_state -- --nocapture
```

Expected:

- Compile error for missing `agent_session_state` field

- [x] **Step 3: Add optional session persistence field and compatibility helpers**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionFile {
    pub schema_version: Option<i32>,
    pub metadata: SessionMetadata,
    pub system_prompt: Option<String>,
    pub messages: Vec<Value>,
    pub agent_session_state: Option<AgentSessionStateMachine>,
}
```

```rust
pub fn conversation_state_or_default(
    session_file: &SessionFile,
    conversation_id: &str,
) -> AgentSessionStateMachine {
    session_file
        .agent_session_state
        .clone()
        .unwrap_or_else(|| AgentSessionStateMachine::new(conversation_id.to_string()))
}
```

- [x] **Step 4: Re-run the session-focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml session_file_deserializes_without_agent_session_state session_file_round_trips_agent_session_state -- --nocapture
```

Expected:

- Both tests pass

- [x] **Step 5: Commit the session persistence checkpoint（按当前任务范围未执行 git checkpoint）**

```bash
git add src-tauri/src/session.rs src-tauri/src/agent/state.rs
git commit -m "feat: persist agent session state in session files"
```

---

### Task 3: Update facts and phase from action outcomes

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/orchestrator.rs`
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/state.rs`
- Test: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/orchestrator.rs`

- [x] **Step 1: Add failing tests for core phase and facts transitions**

```rust
#[test]
fn retrieve_context_updates_phase_and_retrieved_refs() {
    let mut state = AgentTaskState::for_test("conv-1", "research");
    let observation = AgentObservation {
        action_name: "retrieve_context".into(),
        reason: "collect".into(),
        input_summary: "query=Snowflake".into(),
        result_summary: "web_search | https://example.com/article | confidence=2.5".into(),
        summary: "命中 1 条上下文结果".into(),
        status: AgentStepStatus::Completed,
        is_error: false,
        requires_confirmation: false,
        preview_type: None,
        before_preview: None,
        after_preview: None,
        diff_preview: None,
        changed_ranges: None,
        file_operations: None,
    };

    apply_observation_to_session_state(&mut state.session_state, &observation);

    assert_eq!(state.session_state.phase, AgentPhase::Retrieving);
    assert_eq!(
        state.session_state.facts.retrieved_refs,
        vec!["https://example.com/article".to_string()]
    );
}

#[test]
fn blocked_observation_sets_confirmation_phase_and_reason() {
    let mut state = AgentTaskState::for_test("conv-1", "research");
    let observation = blocked_observation(
        &AgentAction::FetchUrl {
            reason: "fetch".into(),
            url: "https://example.com".into(),
            method: FetchMethod::Get,
            max_chars: Some(2000),
        },
        "需要用户确认".into(),
        "medium".into(),
    );

    apply_observation_to_session_state(&mut state.session_state, &observation);

    assert_eq!(state.session_state.phase, AgentPhase::AwaitingConfirmation);
    assert_eq!(
        state.session_state.facts.last_confirmation_request.as_deref(),
        Some("需要用户确认")
    );
}
```

- [x] **Step 2: Run the focused tests to verify the transition helper does not exist yet**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml retrieve_context_updates_phase_and_retrieved_refs blocked_observation_sets_confirmation_phase_and_reason -- --nocapture
```

Expected:

- Compile error for missing `apply_observation_to_session_state`

- [x] **Step 3: Add a dedicated session-state update helper and call it from the observation path**

```rust
fn apply_observation_to_session_state(
    session_state: &mut AgentSessionStateMachine,
    observation: &AgentObservation,
) {
    match observation.action_name.as_str() {
        "retrieve_context" => {
            session_state.phase = AgentPhase::Retrieving;
            for location in parse_collected_refs(&observation.result_summary) {
                if !session_state.facts.retrieved_refs.contains(&location) {
                    session_state.facts.retrieved_refs.push(location);
                }
            }
            update_source_usage(
                &mut session_state.facts.source_usage,
                &observation.result_summary,
            );
        }
        "read_files" => session_state.phase = AgentPhase::Reading,
        "verify_checks" => {
            session_state.phase = AgentPhase::Verifying;
            session_state
                .facts
                .verification_evidence
                .push(observation.summary.clone());
        }
        "summarize_findings" => {
            session_state.phase = AgentPhase::Synthesizing;
            session_state.working_state.draft_summary =
                Some(observation.result_summary.clone());
        }
        _ => {}
    }

    if observation.requires_confirmation {
        session_state.phase = AgentPhase::AwaitingConfirmation;
        session_state.facts.last_confirmation_request = Some(observation.reason.clone());
    } else if observation.is_error {
        session_state.phase = AgentPhase::Blocked;
        session_state.facts.last_error = Some(observation.summary.clone());
    } else {
        session_state.facts.last_completed_action = Some(observation.action_name.clone());
    }

    session_state.timestamps.updated_at = chrono::Utc::now().to_rfc3339();
    session_state.timestamps.facts_updated_at = Some(session_state.timestamps.updated_at.clone());
}
```

- [x] **Step 4: Add finish-phase finalization helper and test it**

```rust
fn apply_goal_status_to_session_state(
    session_state: &mut AgentSessionStateMachine,
    goal_status: &GoalStatus,
) {
    session_state.phase = match goal_status {
        GoalStatus::Done => AgentPhase::Completed,
        GoalStatus::Blocked => AgentPhase::Blocked,
        GoalStatus::NeedsConfirmation => AgentPhase::AwaitingConfirmation,
    };
    session_state.timestamps.updated_at = chrono::Utc::now().to_rfc3339();
}
```

- [x] **Step 5: Re-run the focused orchestrator tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml retrieve_context_updates_phase_and_retrieved_refs blocked_observation_sets_confirmation_phase_and_reason -- --nocapture
```

Expected:

- Transition tests pass

- [x] **Step 6: Commit the orchestrator state update checkpoint（按当前任务范围未执行 git checkpoint）**

```bash
git add src-tauri/src/agent/orchestrator.rs src-tauri/src/agent/state.rs
git commit -m "feat: update agent session state from action outcomes"
```

---

### Task 4: Inject session state summary into planner prompts

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/planner.rs`
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/orchestrator.rs`
- Test: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/planner.rs`

- [x] **Step 1: Add a failing planner-prompt test for session state summary**

```rust
#[test]
fn planner_prompt_includes_session_state_summary() {
    let request = PlannerRequest {
        goal: "研究 Snowflake",
        conversation_context: None,
        workspace_root: ".",
        project_instructions: None,
        workspace_listing: "",
        plan_summary: Some("先检索，再整理"),
        observations: &[],
        available_mcp_tools: None,
        session_state_summary: Some(
            "当前阶段: verifying\n已验证来源: infoq.cn\n当前计划: 生成最终结论"
        ),
        max_steps: 12,
        remaining_steps: 9,
    };

    let prompt = build_planner_user_prompt(&request);
    assert!(prompt.contains("当前状态机摘要"));
    assert!(prompt.contains("当前阶段: verifying"));
    assert!(prompt.contains("已验证来源: infoq.cn"));
}
```

- [x] **Step 2: Run the focused planner test to verify the field is missing**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml planner_prompt_includes_session_state_summary -- --nocapture
```

Expected:

- Compile error for missing `session_state_summary`

- [x] **Step 3: Extend `PlannerRequest` and planner user prompt**

```rust
pub struct PlannerRequest<'a> {
    pub goal: &'a str,
    pub conversation_context: Option<&'a str>,
    pub workspace_root: &'a str,
    pub project_instructions: Option<&'a str>,
    pub workspace_listing: &'a str,
    pub plan_summary: Option<&'a str>,
    pub observations: &'a [AgentObservation],
    pub available_mcp_tools: Option<&'a str>,
    pub session_state_summary: Option<&'a str>,
    pub max_steps: usize,
    pub remaining_steps: usize,
}
```

```rust
format!(
    "用户目标:\n{}\n\n当前状态机摘要:\n{}\n\n最近对话上下文:\n{}\n...",
    request.goal,
    request
        .session_state_summary
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("(暂无状态机摘要)"),
    ...
)
```

- [x] **Step 4: Add session-state summarizer in orchestrator and thread it into planner requests**

```rust
fn summarize_session_state_for_planner(state: &AgentSessionStateMachine) -> String {
    let verified = if state.facts.verified_sources.is_empty() {
        "(暂无)".to_string()
    } else {
        state.facts.verified_sources.join(", ")
    };
    let evidence = if state.facts.verification_evidence.is_empty() {
        "(暂无)".to_string()
    } else {
        state.facts.verification_evidence.join("；")
    };

    format!(
        "当前阶段: {:?}\n已验证来源: {}\n已完成验证: {}\n未解决问题: {}\n当前计划: {}",
        state.phase,
        verified,
        evidence,
        if state.working_state.open_questions.is_empty() {
            "(暂无)".into()
        } else {
            state.working_state.open_questions.join("；")
        },
        if state.working_state.next_plan.is_empty() {
            "(暂无)".into()
        } else {
            state.working_state.next_plan.join("；")
        }
    )
}
```

- [x] **Step 5: Re-run the planner-focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml planner_prompt_includes_session_state_summary -- --nocapture
```

Expected:

- Test passes and planner prompt contains the state summary block

- [x] **Step 6: Commit the planner integration checkpoint（按当前任务范围未执行 git checkpoint）**

```bash
git add src-tauri/src/agent/planner.rs src-tauri/src/agent/orchestrator.rs
git commit -m "feat: inject session state summary into planner context"
```

---

### Task 5: Restore and persist state through the agent entrypoint

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/lib.rs`
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/session.rs`
- Modify: `/Users/Gress/code/ai/deepseekX/src-tauri/src/agent/orchestrator.rs`
- Test: `/Users/Gress/code/ai/deepseekX/src-tauri/src/session.rs`

- [x] **Step 1: Add a failing test for saving updated state back into the session file**

```rust
#[test]
fn saves_updated_agent_session_state_into_session_file() {
    let mut session = SessionFile {
        schema_version: Some(1),
        metadata: SessionMetadata {
            id: Some("conv-1".into()),
            title: Some("Example".into()),
            created_at: None,
            updated_at: None,
            message_count: None,
            total_tokens: None,
            model: None,
            workspace: None,
            mode: None,
        },
        system_prompt: None,
        messages: vec![],
        agent_session_state: None,
    };

    let updated = AgentSessionStateMachine::new("conv-1".into());
    persist_agent_session_state(&mut session, updated.clone());

    assert_eq!(
        session.agent_session_state.unwrap().conversation_id,
        "conv-1".to_string()
    );
}
```

- [x] **Step 2: Run the focused test to verify the persistence helper is missing**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml saves_updated_agent_session_state_into_session_file -- --nocapture
```

Expected:

- Compile error for missing `persist_agent_session_state`

- [x] **Step 3: Add restore/persist helpers and use them around `run_agent_loop`**

```rust
pub fn persist_agent_session_state(
    session_file: &mut SessionFile,
    state: AgentSessionStateMachine,
) {
    session_file.agent_session_state = Some(state);
}
```

```rust
let restored_state = session::conversation_state_or_default(&session_file, &conversation_id);

let result = agent::orchestrator::run_agent_loop(
    &config,
    agent::orchestrator::AgentRunRequest {
        ...
        restored_session_state: Some(restored_state),
    },
).await?;

session::persist_agent_session_state(&mut session_file, result.session_state.clone());
```

- [x] **Step 4: Extend `AgentRunRequest` and `AgentRunResponse` for state pass-through**

```rust
pub struct AgentRunRequest<'a> {
    ...
    pub restored_session_state: Option<AgentSessionStateMachine>,
}

pub struct AgentRunResponse {
    pub final_response: String,
    pub goal_status: String,
    pub steps: Vec<AgentObservation>,
    pub llm_debug_responses: Vec<LlmDebugResponse>,
    pub session_state: AgentSessionStateMachine,
}
```

- [x] **Step 5: Re-run the focused persistence test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml saves_updated_agent_session_state_into_session_file -- --nocapture
```

Expected:

- Test passes

- [x] **Step 6: Commit the session restore/persist checkpoint（按当前任务范围未执行 git checkpoint）**

```bash
git add src-tauri/src/lib.rs src-tauri/src/session.rs src-tauri/src/agent/orchestrator.rs
git commit -m "feat: restore and persist agent session state by conversation"
```

---

### Task 6: Run full verification and document Phase 1 completion

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/docs/superpowers/specs/2026-05-30-agent-session-state-machine-design.md`
- Modify: `/Users/Gress/code/ai/deepseekX/docs/superpowers/plans/2026-05-30-agent-session-state-machine-phase1.md`

- [x] **Step 1: Run the focused agent/session test set**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml agent:: -- --nocapture
```

Expected:

- All agent module tests pass

- [x] **Step 2: Run the full backend and frontend verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

Expected:

- Rust test suite passes
- Frontend build passes

- [x] **Step 3: Update the spec status and plan checklist after successful implementation**

```md
> 状态：Implemented
```

```md
- [x] **Step N: ...**
```

- [x] **Step 4: Commit the verification + doc sync checkpoint（按当前任务范围未执行 git checkpoint）**

```bash
git add docs/superpowers/specs/2026-05-30-agent-session-state-machine-design.md docs/superpowers/plans/2026-05-30-agent-session-state-machine-phase1.md
git commit -m "docs: mark agent session state machine phase 1 implemented"
```

---

## Self-Review

- **Spec coverage:** This plan intentionally covers only Phase 1 from the spec: system-maintained `facts + phase`, planner state summary injection, and session restoration/persistence. It does not implement model-authored `state_update`; that belongs to a separate Phase 2 plan.
- **Placeholder scan:** No `TODO`/`TBD` placeholders remain. Every task includes exact files, code, and commands.
- **Type consistency:** The plan uses one consistent naming set throughout: `AgentSessionStateMachine`, `AgentPhase`, `AgentFacts`, `AgentWorkingState`, `AgentStateTimestamps`, `session_state_summary`, `persist_agent_session_state`, and `conversation_state_or_default`.
