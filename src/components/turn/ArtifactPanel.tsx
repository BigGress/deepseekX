import { useMemo, useState } from "react";
import type { AgentStep, FileOperation } from "../../types";
import { hasArtifacts } from "./turnPresentation";

export default function ArtifactPanel({ agentSteps }: { agentSteps?: AgentStep[] | null }) {
  const [expanded, setExpanded] = useState(false);
  const artifactSteps = useMemo(() => (agentSteps ?? []).filter(hasArtifacts), [agentSteps]);

  if (artifactSteps.length === 0) {
    return null;
  }

  return (
    <div className="flex justify-start">
      <section className="max-w-[85%] w-full rounded-2xl border border-neutral-800 bg-neutral-925/90 px-4 py-3">
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="flex w-full items-center justify-between text-left"
        >
          <div>
            <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-500">Artifacts</p>
            <p className="mt-1 text-sm text-neutral-200">代码、Diff 和文件操作产物</p>
          </div>
          <span className="text-xs text-neutral-400">
            {artifactSteps.length} 项 {expanded ? "▲" : "▼"}
          </span>
        </button>

        {expanded && (
          <div className="mt-4 space-y-3">
            {artifactSteps.map((step, index) => (
              <CodeArtifactCard key={`${step.action_name}-${index}`} step={step} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function CodeArtifactCard({ step }: { step: AgentStep }) {
  return (
    <article className="rounded-xl border border-neutral-800 bg-neutral-950/80 px-3 py-3">
      <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-500">
        {step.diff_preview || step.file_operations?.length ? "Diff Card" : "CodeArtifactCard"}
      </p>
      <p className="mt-1 text-sm font-medium text-neutral-100">{step.action_name}</p>
      {step.before_preview && <PreviewBlock title="修改前" content={step.before_preview} />}
      {step.after_preview && (
        <PreviewBlock
          title={step.preview_type === "full" ? "文件预览" : "修改后"}
          content={step.after_preview}
        />
      )}
      {step.diff_preview && <PreviewBlock title="Diff 预览" content={step.diff_preview} />}
      {step.changed_ranges && step.changed_ranges.length > 0 && (
        <p className="mt-2 text-[11px] text-neutral-400">变更范围: {step.changed_ranges.join(", ")}</p>
      )}
      {step.file_operations && step.file_operations.length > 0 && (
        <div className="mt-3 space-y-1">
          <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-500">文件操作</p>
          {step.file_operations.map((operation, index) => (
            <FileOperationItem key={`${operation.path}-${operation.operation}-${index}`} operation={operation} />
          ))}
        </div>
      )}
    </article>
  );
}

function PreviewBlock({ title, content }: { title: string; content: string }) {
  return (
    <div className="mt-3">
      <p className="mb-1 text-[11px] uppercase tracking-[0.18em] text-neutral-500">{title}</p>
      <pre className="overflow-x-auto rounded-md bg-neutral-900 p-3 text-[11px] text-neutral-200">
        <code>{content}</code>
      </pre>
    </div>
  );
}

function FileOperationItem({ operation }: { operation: FileOperation }) {
  const target = operation.target_path ? ` -> ${operation.target_path}` : "";
  const ranges =
    operation.changed_ranges && operation.changed_ranges.length > 0
      ? ` (${operation.changed_ranges.join(", ")})`
      : "";

  return (
    <p className="text-[11px] text-neutral-300">
      {operation.operation}: {operation.path}
      {target}
      {ranges}
    </p>
  );
}
