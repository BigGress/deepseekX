# Tool Process Visibility Implementation Plan

**Status:** Archived

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tool and execution-process content secondary in the chat window by keeping successful execution detail collapsed by default and rendering `summarize_findings` as a one-line summary with an expandable full body that excludes raw retrieval hit dumps.

**Architecture:** Keep the existing `Turn`-based result layout and implement the behavior entirely in the presentation layer. Add explicit display-policy helpers in `turnPresentation.ts`, then update `ExecutionPanel` to support default-expand rules and per-card secondary expansion for `summarize_findings`, while keeping `ResultCardGroup` as the sole owner of the primary final answer.

**Tech Stack:** React, TypeScript, Vitest, existing `Turn` presentation components in `src/components/turn`

---

## File Structure

- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/turnPresentation.ts`
  - Add display-policy helpers for execution default expansion and summary-like steps.
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/ExecutionPanel.tsx`
  - Apply the new default expansion policy.
  - Add secondary expansion for `summarize_findings`.
  - Render short summary first and full body only on demand.
- Modify: `/Users/Gress/code/ai/deepseekX/src/debug/TurnItemPreview.tsx`
  - Add a success-case mock that demonstrates the new collapsed behavior and `summarize_findings` card.
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/TurnItem.test.tsx`
  - Add UI tests for collapsed success behavior and visible error-state expansion if current test harness allows.
- Optional if needed during implementation: `/Users/Gress/code/ai/deepseekX/src/types.ts`
  - Only if the implementation requires a small helper type or stricter prop typing. Avoid broad model changes.

---

### Task 1: Lock the display policy in tests

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/TurnItem.test.tsx`
- Reference: `/Users/Gress/code/ai/deepseekX/src/components/TurnItem.tsx`
- Reference: `/Users/Gress/code/ai/deepseekX/src/components/turn/ExecutionPanel.tsx`

- [ ] **Step 1: Add a failing test for successful turns keeping execution detail collapsed**

```tsx
it("keeps execution detail collapsed for successful research turns", () => {
  const turn = {
    id: "turn-success",
    user_input: "调研 Snowflake",
    final_response: "Snowflake 是一家云原生数据平台公司。",
    agent_goal_status: "done",
    thinking_steps: [],
    agent_steps: [
      {
        action_name: "retrieve_context",
        reason: "收集上下文",
        input_summary: "query=Snowflake",
        result_summary: "web_search | https://example.com | confidence=2.5",
        summary: "命中 3 条上下文结果",
        status: "completed",
        is_error: false,
        requires_confirmation: false,
        tool_results: [],
      },
      {
        action_name: "summarize_findings",
        reason: "整理研究结果",
        result_summary: "聚焦主题: Snowflake\n输出格式: research_brief\n\n完整正文……",
        summary: "已整理研究简报，提炼为 4 个结论",
        status: "completed",
        is_error: false,
        requires_confirmation: false,
        tool_results: [],
      },
    ],
  };

  render(<TurnItem turn={turn as any} />);

  expect(screen.getByText("Execution Detail")).toBeInTheDocument();
  expect(screen.queryByText(/完整正文/)).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Add a failing test for blocked turns auto-expanding execution detail**

```tsx
it("auto-expands execution detail for blocked turns", () => {
  const turn = {
    id: "turn-blocked",
    user_input: "抓取网页",
    final_response: "fetch_url 需要用户确认后才能继续。",
    agent_goal_status: "needs_confirmation",
    thinking_steps: [],
    agent_steps: [
      {
        action_name: "fetch_url",
        reason: "抓取站点",
        input_summary: "url=https://example.com",
        result_summary: "risk_level=medium; 抓取返回字符数过大",
        summary: "URL 抓取被权限门拦截",
        status: "blocked",
        is_error: true,
        requires_confirmation: true,
        tool_results: [],
      },
    ],
  };

  render(<TurnItem turn={turn as any} />);

  expect(screen.getByText(/risk_level=medium/)).toBeInTheDocument();
});
```

- [ ] **Step 3: Run the focused frontend test file and confirm at least one new assertion fails**

Run:

```bash
npm run build
```

Expected:

- TypeScript and current UI build still pass or fail only after the next implementation step if the test environment is separate.
- If the repository exposes a Vitest target, also run it and expect the newly added assertions to fail before implementation.

- [ ] **Step 4: Commit the failing-test checkpoint only if the repo already uses TDD commits for UI work**

```bash
git add src/components/TurnItem.test.tsx
git commit -m "test: cover tool process visibility rules"
```

If the project does not currently use failing intermediate commits, skip this commit and keep the test change staged with the implementation task.

---

### Task 2: Add display-policy helpers for successful vs exceptional turns

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/turnPresentation.ts`
- Test through: `/Users/Gress/code/ai/deepseekX/src/components/TurnItem.test.tsx`

- [ ] **Step 1: Add helpers that define when execution should auto-expand**

```ts
export function shouldExpandExecutionByDefault(turn: Turn) {
  const status = formatGoalStatus(turn.agent_goal_status);
  return status === "blocked" || status === "failed" || status === "needs_confirmation";
}

export function isSummaryLikeStep(step: AgentStep) {
  return step.action_name === "summarize_findings";
}

export function summarizeExecutionStep(step: AgentStep) {
  return step.summary || step.result_summary || step.reason || step.action_name;
}
```

- [ ] **Step 2: Add a helper that identifies retrieval-heavy step bodies**

```ts
export function shouldHideVerboseBodyByDefault(step: AgentStep) {
  return (
    step.action_name === "summarize_findings" ||
    step.action_name === "retrieve_context"
  );
}
```

- [ ] **Step 3: Re-run the frontend checks**

Run:

```bash
npm run build
```

Expected:

- Build passes.
- New helper exports compile cleanly with existing imports.

- [ ] **Step 4: Commit the display-policy helper layer**

```bash
git add src/components/turn/turnPresentation.ts
git commit -m "feat: add execution display policy helpers"
```

---

### Task 3: Make successful execution detail collapsed by default

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/ExecutionPanel.tsx`
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/turnPresentation.ts`

- [ ] **Step 1: Initialize the panel expansion state from the new helper**

```tsx
interface ExecutionPanelProps {
  thinkingSteps: ThinkingStep[];
  agentSteps?: AgentStep[] | null;
  defaultExpanded: boolean;
}

export default function ExecutionPanel({
  thinkingSteps,
  agentSteps,
  defaultExpanded,
}: ExecutionPanelProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
```

- [ ] **Step 2: Update the title copy to de-emphasize process details**

```tsx
<div>
  <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-500">Execution Detail</p>
  <p className="mt-1 text-sm text-neutral-200">
    {expanded ? "执行过程详情" : "查看执行过程"}
  </p>
</div>
```

- [ ] **Step 3: Thread the new prop from `TurnItem`**

```tsx
<ExecutionPanel
  thinkingSteps={turn.thinking_steps}
  agentSteps={turn.agent_steps}
  defaultExpanded={shouldExpandExecutionByDefault(turn)}
/>
```

- [ ] **Step 4: Run the build and confirm successful turns no longer render the execution body by default**

Run:

```bash
npm run build
```

Expected:

- Build passes.
- Successful turns show only the collapsed execution header until the user expands it.

- [ ] **Step 5: Commit the default-collapse behavior**

```bash
git add src/components/TurnItem.tsx src/components/turn/ExecutionPanel.tsx src/components/turn/turnPresentation.ts
git commit -m "feat: collapse successful execution detail by default"
```

---

### Task 4: Add two-level expansion for `summarize_findings`

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/ExecutionPanel.tsx`
- Reference: `/Users/Gress/code/ai/deepseekX/src/components/turn/turnPresentation.ts`

- [ ] **Step 1: Add per-card local expansion state for summary-like steps**

```tsx
function ExecutionCard({ step }: { step: AgentStep }) {
  const tone = stepTone(step);
  const isSummaryStep = isSummaryLikeStep(step);
  const [showBody, setShowBody] = useState(false);
```

- [ ] **Step 2: Render one-line summary by default for `summarize_findings`**

```tsx
<p className="mt-1 whitespace-pre-wrap text-xs text-neutral-300/90">
  摘要: {summarizeExecutionStep(step)}
</p>
```

- [ ] **Step 3: Add a secondary toggle that reveals the full body only on demand**

```tsx
{isSummaryStep && step.result_summary ? (
  <div className="mt-3">
    <button
      type="button"
      onClick={() => setShowBody((value) => !value)}
      className="text-xs text-neutral-400 hover:text-neutral-200"
    >
      {showBody ? "收起整理正文" : "查看整理正文"}
    </button>
    {showBody ? (
      <pre className="mt-2 whitespace-pre-wrap text-xs text-neutral-300/90">
        {step.result_summary}
      </pre>
    ) : null}
  </div>
) : null}
```

- [ ] **Step 4: Keep non-summary execution cards unchanged except for the new policy**

```tsx
{!isSummaryStep && (step.result_summary || step.summary) ? (
  <p className="mt-1 whitespace-pre-wrap text-xs text-neutral-300/90">
    结果: {step.result_summary ?? step.summary}
  </p>
) : null}
```

- [ ] **Step 5: Run frontend verification**

Run:

```bash
npm run build
```

Expected:

- Build passes.
- `summarize_findings` shows only the short summary until the card-level toggle is opened.

- [ ] **Step 6: Commit the `summarize_findings` secondary expansion**

```bash
git add src/components/turn/ExecutionPanel.tsx src/components/turn/turnPresentation.ts
git commit -m "feat: add secondary expansion for summarize findings"
```

---

### Task 5: Stop `summarize_findings` from surfacing raw retrieval hit dumps in normal chat

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src/components/turn/ExecutionPanel.tsx`
- Optional if needed: `/Users/Gress/code/ai/deepseekX/src/components/turn/turnPresentation.ts`

- [ ] **Step 1: Add a presentation-layer filter for verbose retrieval dump patterns inside `summarize_findings`**

```ts
function sanitizeSummaryBody(text: string) {
  return text
    .split("\n")
    .filter((line) => {
      const trimmed = line.trim();
      return !(
        trimmed.startsWith("web_search | ") ||
        trimmed.startsWith("workspace_code | ") ||
        trimmed.startsWith("workspace_docs | ") ||
        trimmed.startsWith("next_hint=") ||
        trimmed.startsWith("user_knowledge_base: unavailable")
      );
    })
    .join("\n")
    .trim();
}
```

- [ ] **Step 2: Apply the sanitizer only to summary-step full bodies**

```tsx
const body = isSummaryStep && step.result_summary
  ? sanitizeSummaryBody(step.result_summary)
  : step.result_summary;
```

- [ ] **Step 3: Make the body fallback safe when sanitization removes everything**

```tsx
const displayBody =
  body && body.trim().length > 0 ? body : "该步骤仅生成了整理摘要，未保留额外正文。";
```

- [ ] **Step 4: Re-run build and manually review the preview page**

Run:

```bash
npm run build
```

Expected:

- Build passes.
- `summarize_findings` full body no longer repeats retrieval hit lines in normal chat presentation.

- [ ] **Step 5: Commit the presentation-layer sanitization**

```bash
git add src/components/turn/ExecutionPanel.tsx src/components/turn/turnPresentation.ts
git commit -m "feat: hide retrieval dumps from summarize findings body"
```

---

### Task 6: Refresh the debug preview and final verification

**Files:**
- Modify: `/Users/Gress/code/ai/deepseekX/src/debug/TurnItemPreview.tsx`
- Modify if needed: `/Users/Gress/code/ai/deepseekX/src/components/TurnItem.test.tsx`

- [ ] **Step 1: Update the preview mock to include a successful research turn with `summarize_findings`**

```tsx
const MOCK_RESEARCH_TURN = {
  id: "research-turn",
  user_input: "调研 Snowflake 的主营业务和盈利模式",
  final_response: "Snowflake 是一家云原生数据平台公司。",
  agent_goal_status: "done",
  thinking_steps: [],
  agent_steps: [
    {
      action_name: "retrieve_context",
      summary: "命中 10 条上下文结果",
      result_summary: "web_search | https://example.com | confidence=2.5",
      status: "completed",
      is_error: false,
      requires_confirmation: false,
      tool_results: [],
    },
    {
      action_name: "summarize_findings",
      summary: "已整理研究简报，提炼为 4 个结论",
      result_summary: "聚焦主题: Snowflake\n\n完整整理正文……",
      status: "completed",
      is_error: false,
      requires_confirmation: false,
      tool_results: [],
    },
  ],
};
```

- [ ] **Step 2: Render the preview turn so manual review clearly shows the new collapsed behavior**

```tsx
<TurnItem turn={MOCK_RESEARCH_TURN as any} />
```

- [ ] **Step 3: Run the full verification suite**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri build
```

Expected:

- Rust tests pass unchanged.
- Frontend build passes.
- Tauri bundle build passes.

- [ ] **Step 4: Manually inspect the updated desktop app or preview page**

Manual checks:

- successful research turn keeps `Execution Detail` collapsed by default
- blocked or confirmation-required turn auto-expands execution detail
- `summarize_findings` shows one-line summary first
- expanding that card shows cleaned full body without raw retrieval hit dump lines

- [ ] **Step 5: Commit the preview update and final integration**

```bash
git add src/debug/TurnItemPreview.tsx src/components/TurnItem.test.tsx src/components/TurnItem.tsx src/components/turn/ExecutionPanel.tsx src/components/turn/turnPresentation.ts
git commit -m "feat: de-emphasize tool process results in chat window"
```

---

## Self-Review

### Spec coverage

- Default collapse of process content: covered in Task 3.
- Auto-expand for blocked / failed / needs_confirmation: covered in Task 3 and Task 1.
- `summarize_findings` one-line summary + expandable body: covered in Task 4.
- Prevent retrieval raw hit dump from flowing into normal body display: covered in Task 5.
- Keep primary result as the sole main answer surface: preserved in Task 3 and Task 4 by not touching `ResultCardGroup`.

### Placeholder scan

- No `TBD`, `TODO`, or deferred “later” steps remain.
- Each task includes exact file paths and concrete commands.

### Type consistency

- The plan consistently uses `Turn`, `AgentStep`, `ExecutionPanel`, and `turnPresentation.ts`.
- The new helper names are consistent across tasks:
  - `shouldExpandExecutionByDefault`
  - `isSummaryLikeStep`
  - `summarizeExecutionStep`
  - `shouldHideVerboseBodyByDefault`

---

Plan complete and saved to `/Users/Gress/code/ai/deepseekX/docs/superpowers/plans/2026-05-30-tool-process-visibility.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration

2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
