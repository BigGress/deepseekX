# Primary Result Markdown Rendering Implementation Plan

**Status:** Archived

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hand-written `PRIMARY RESULT` Markdown parser with the repo's existing `react-markdown + remark-gfm` stack while preserving current styling and adding GFM support for tables and task lists.

**Architecture:** Keep the existing `MessageMarkdown` component as the single rendering entry point for `PRIMARY RESULT`, but swap its internals to `ReactMarkdown` with a controlled `components` map. Add focused renderer tests for existing Markdown behavior plus new GFM cases, then validate through the existing preview surface and production build.

**Tech Stack:** React 18, TypeScript, `react-markdown`, `remark-gfm`, Vitest, Testing Library, Vite

---

## File Structure

### Files to Modify

- Modify: [src/components/turn/MessageMarkdown.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.tsx)
  - Replace the custom parser with `ReactMarkdown`
  - Define the `components` map for headings, paragraphs, blockquotes, links, inline code, code blocks, lists, tables, and read-only task list checkboxes
- Modify: [src/components/turn/MessageMarkdown.test.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.test.tsx)
  - Keep current regression coverage
  - Add tests for GFM table rendering
  - Add tests for task list rendering
  - Add tests for strikethrough rendering

### Files to Inspect During Implementation

- Inspect: [src/components/turn/ResultCardGroup.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/ResultCardGroup.tsx)
  - Confirm `PRIMARY RESULT` still calls `MessageMarkdown` unchanged
- Inspect: [src/debug/TurnItemPreview.tsx](/Users/Gress/code/ai/deepseekX/src/debug/TurnItemPreview.tsx)
  - Use the existing preview content to visually confirm the new renderer

### Files Not in Scope

- Do not modify other rich text renderers in `ExecutionPanel`, `ArtifactPanel`, `Debug Mode`, or non-`PRIMARY RESULT` sections
- Do not add new npm dependencies

---

### Task 1: Lock In Current Entry Point And Baseline Tests

**Files:**
- Inspect: [src/components/turn/ResultCardGroup.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/ResultCardGroup.tsx)
- Modify: [src/components/turn/MessageMarkdown.test.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.test.tsx)

- [ ] **Step 1: Confirm `PRIMARY RESULT` still routes through `MessageMarkdown`**

Run:

```bash
rg -n "MessageMarkdown content=|<MessageMarkdown" /Users/Gress/code/ai/deepseekX/src/components/turn /Users/Gress/code/ai/deepseekX/src/components/TurnItem.tsx
```

Expected: `ResultCardGroup` is the only `PRIMARY RESULT` path using `MessageMarkdown`, so the renderer swap stays scoped.

- [ ] **Step 2: Add failing GFM coverage before implementation**

Update [src/components/turn/MessageMarkdown.test.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.test.tsx) with tests shaped like:

```tsx
it("renders gfm tables", () => {
  render(
    <MessageMarkdown
      content={`| 平台 | 优势 |
| --- | --- |
| Snowflake | 易用性 |
| Databricks | ML 生态 |`}
    />,
  );

  expect(screen.getByRole("table")).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "平台" })).toBeInTheDocument();
  expect(screen.getByRole("cell", { name: "Snowflake" })).toBeInTheDocument();
});

it("renders gfm task lists as read-only checkboxes", () => {
  render(
    <MessageMarkdown
      content={`- [x] 已完成验证
- [ ] 待补充数据`}
    />,
  );

  const checkboxes = screen.getAllByRole("checkbox");
  expect(checkboxes).toHaveLength(2);
  expect(checkboxes[0]).toBeChecked();
  expect(checkboxes[0]).toBeDisabled();
  expect(checkboxes[1]).not.toBeChecked();
  expect(checkboxes[1]).toBeDisabled();
});

it("renders strikethrough text", () => {
  render(<MessageMarkdown content={"~~过时结论~~"} />);
  expect(screen.getByText("过时结论").tagName).toBe("DEL");
});
```

- [ ] **Step 3: Run the targeted tests to verify they fail against the current custom parser**

Run:

```bash
npm test -- --run src/components/turn/MessageMarkdown.test.tsx
```

Expected: the new GFM table/task-list/strikethrough assertions fail because the hand-written parser does not support them completely.

- [ ] **Step 4: Commit the red test state**

```bash
git add /Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.test.tsx
git commit -m "test: add markdown renderer gfm coverage"
```

### Task 2: Replace The Custom Parser With `react-markdown + remark-gfm`

**Files:**
- Modify: [src/components/turn/MessageMarkdown.tsx](/Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.tsx)

- [ ] **Step 1: Replace the custom parser with a single `ReactMarkdown` renderer**

Rewrite the component around this shape:

```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export default function MessageMarkdown({ content }: { content: string }) {
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
      {content}
    </ReactMarkdown>
  );
}
```

- [ ] **Step 2: Recreate the current visual language through a `components` map**

Implement a `markdownComponents` object that covers the required nodes with explicit classes. The code should look like:

```tsx
const markdownComponents = {
  h1: ({ children }) => <h1 className="text-xl font-semibold text-white">{children}</h1>,
  h2: ({ children }) => <h2 className="mt-4 text-lg font-semibold text-white">{children}</h2>,
  p: ({ children }) => <p className="leading-7 text-neutral-200">{children}</p>,
  blockquote: ({ children }) => (
    <blockquote className="border-l-2 border-emerald-500/40 pl-4 text-neutral-300">
      {children}
    </blockquote>
  ),
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="text-sky-300 underline decoration-sky-500/60 underline-offset-2 hover:text-sky-200"
    >
      {children}
    </a>
  ),
};
```

Keep the current link behavior and reuse the existing dark code styling semantics.

- [ ] **Step 3: Implement separate styling for inline code versus fenced blocks**

Use the node shape from `react-markdown` to distinguish inline and block code:

```tsx
code: ({ inline, className, children }) =>
  inline ? (
    <code className="rounded bg-neutral-950/80 px-1 py-0.5 text-[0.9em] text-neutral-100">
      {children}
    </code>
  ) : (
    <code className={className}>{children}</code>
  ),
pre: ({ children }) => (
  <pre className="my-3 overflow-x-auto rounded-md bg-neutral-950/80 p-3 text-xs text-neutral-200">
    {children}
  </pre>
),
```

Requirement: do not regress the current block-code look.

- [ ] **Step 4: Add table and task list renderers**

Implement renderer entries equivalent to:

```tsx
table: ({ children }) => (
  <div className="my-4 overflow-x-auto">
    <table className="min-w-full border-collapse text-sm text-neutral-200">{children}</table>
  </div>
),
th: ({ children }) => (
  <th className="border-b border-neutral-700 px-3 py-2 text-left font-medium text-white">
    {children}
  </th>
),
td: ({ children }) => (
  <td className="border-b border-neutral-800 px-3 py-2 align-top">{children}</td>
),
input: ({ checked, disabled, type }) =>
  type === "checkbox" ? (
    <input type="checkbox" checked={checked} disabled readOnly className="mr-2 accent-emerald-400" />
  ) : (
    <input type={type} disabled={disabled} readOnly />
  ),
```

Requirement: task list checkboxes must render as disabled/read-only, not interactive form controls.

- [ ] **Step 5: Run targeted tests to verify the new renderer passes**

Run:

```bash
npm test -- --run src/components/turn/MessageMarkdown.test.tsx
```

Expected: all tests in `MessageMarkdown.test.tsx` pass.

- [ ] **Step 6: Commit the renderer replacement**

```bash
git add /Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.tsx /Users/Gress/code/ai/deepseekX/src/components/turn/MessageMarkdown.test.tsx
git commit -m "feat: switch primary result markdown to react-markdown"
```

### Task 3: Visual Verification Through Existing Preview Surface

**Files:**
- Inspect: [src/debug/TurnItemPreview.tsx](/Users/Gress/code/ai/deepseekX/src/debug/TurnItemPreview.tsx)

- [ ] **Step 1: Ensure the existing preview sample contains coverage for headings, lists, blockquotes, links, code, and at least one GFM element**

If the current preview fixture lacks table or task-list content, extend the preview sample in [src/debug/TurnItemPreview.tsx](/Users/Gress/code/ai/deepseekX/src/debug/TurnItemPreview.tsx) with a small Markdown snippet such as:

```ts
const previewMarkdown = `# 调研结果

> 这是摘要引用

| 平台 | 优势 |
| --- | --- |
| Snowflake | 数据共享 |

- [x] 已完成调研
- [ ] 待补充财报数据`;
```

- [ ] **Step 2: Start the app preview and inspect the renderer visually**

Run:

```bash
npm run dev
```

Then open:

```text
http://127.0.0.1:1420/?debug=turn-preview
```

Expected:
- headings keep hierarchy
- code blocks keep dark styling
- tables are readable and horizontally scroll when needed
- task list checkboxes are visible but disabled
- links retain the existing visual treatment

- [ ] **Step 3: Stop the dev server after verification**

Expected: no background Vite process remains running after the check.

- [ ] **Step 4: Commit any fixture-only preview updates**

```bash
git add /Users/Gress/code/ai/deepseekX/src/debug/TurnItemPreview.tsx
git commit -m "test: expand markdown preview fixture"
```

Only do this commit if `TurnItemPreview.tsx` changed.

### Task 4: Full Frontend Verification And Handoff

**Files:**
- No additional source changes expected

- [ ] **Step 1: Run the focused frontend test suite**

Run:

```bash
npm test -- --run src/components/turn/MessageMarkdown.test.tsx src/components/TurnItem.test.tsx
```

Expected: PASS for the renderer coverage and existing turn rendering regressions.

- [ ] **Step 2: Run the production build**

Run:

```bash
npm run build
```

Expected: Vite production build succeeds with no TypeScript errors.

- [ ] **Step 3: Summarize the user-visible outcome**

Capture these outcomes in the handoff note:

```text
- PRIMARY RESULT now renders through react-markdown + remark-gfm
- Existing prose/code/link styling remains intact
- GFM tables, task lists, and strikethrough are supported
- No raw HTML rendering was enabled
```

- [ ] **Step 4: Commit the verification-complete state**

```bash
git add /Users/Gress/code/ai/deepseekX
git commit -m "chore: verify primary result markdown renderer"
```

Only include files changed by this plan.

---

## Self-Review

### Spec Coverage

- Replace `PRIMARY RESULT` Markdown renderer with `react-markdown + remark-gfm`: covered by Task 2
- Preserve existing styling semantics: covered by Task 2 and Task 3
- Add GFM support for tables and task lists: covered by Task 1 and Task 2
- Keep scope limited to `PRIMARY RESULT`: enforced by File Structure and task boundaries
- Verify through tests and build: covered by Task 4

### Placeholder Scan

No `TODO`, `TBD`, or generic “add tests later” steps remain. Each task names exact files, commands, and expected outcomes.

### Type Consistency

The plan uses the existing `MessageMarkdown({ content }: { content: string })` interface throughout and keeps `ResultCardGroup` as the unchanged call site, so there is no alternate renderer API introduced later in the plan.
