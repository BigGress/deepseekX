import { useEffect, useState } from "react";
import type { AgentStep, ThinkingStep } from "../../types";
import {
  isSummaryLikeStep,
  sanitizeSummaryBody,
  shouldHideVerboseBodyByDefault,
  stepTone,
  summarizeExecutionStep,
} from "./turnPresentation";

interface ExecutionPanelProps {
  thinkingSteps: ThinkingStep[];
  agentSteps?: AgentStep[] | null;
  defaultExpanded?: boolean;
}

export default function ExecutionPanel({
  thinkingSteps,
  agentSteps,
  defaultExpanded = false,
}: ExecutionPanelProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const hasThinking = thinkingSteps.length > 0;
  const hasSteps = Boolean(agentSteps && agentSteps.length > 0);

  if (!hasThinking && !hasSteps) {
    return null;
  }

  useEffect(() => {
    if (defaultExpanded) {
      setExpanded(true);
    }
  }, [defaultExpanded]);

  const total = thinkingSteps.length + (agentSteps?.length ?? 0);

  return (
    <div className="flex justify-start">
      <section className="max-w-[85%] w-full rounded-2xl border border-neutral-800 bg-neutral-925/90 px-4 py-3">
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="flex w-full items-center justify-between text-left"
        >
          <div>
            <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-500">Execution Detail</p>
            <p className="mt-1 text-sm text-neutral-400">{expanded ? "执行过程详情" : "查看执行过程"}</p>
          </div>
          <span className="text-xs text-neutral-400">
            {total} 项 {expanded ? "▲" : "▼"}
          </span>
        </button>

        {expanded && (
          <div className="mt-4 space-y-3">
            {thinkingSteps.map((step, index) => (
              <ReasoningCard key={`thinking-${index}`} step={step} />
            ))}
            {agentSteps?.map((step, index) => (
              <ExecutionCard key={`${step.action_name}-${index}`} step={step} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function ReasoningCard({ step }: { step: ThinkingStep }) {
  return (
    <article className="rounded-xl border border-neutral-800 bg-neutral-950/80 px-3 py-3">
      <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-500">Reasoning Card</p>
      {step.thinking && <p className="mt-2 text-sm italic text-neutral-300">💭 {step.thinking}</p>}
      {step.tool_calls.length > 0 && (
        <div className="mt-3 space-y-1">
          {step.tool_calls.map((toolCall) => (
            <p key={toolCall.tool_call_id} className="text-xs text-sky-300">
              🔧 {toolCall.tool_name}
              {toolCall.tool_input ? <span className="text-neutral-400"> {toolCall.tool_input}</span> : null}
            </p>
          ))}
        </div>
      )}
      {step.tool_results.length > 0 && (
        <div className="mt-2 space-y-1">
          {step.tool_results.map((toolResult) => (
            <p
              key={toolResult.tool_call_id}
              className={`text-xs ${toolResult.is_error ? "text-rose-300" : "text-emerald-300"}`}
            >
              📋 {toolResult.summary || (toolResult.is_error ? "执行失败" : "执行成功")}
            </p>
          ))}
        </div>
      )}
    </article>
  );
}

function ExecutionCard({ step }: { step: AgentStep }) {
  const tone = stepTone(step);
  const isSummaryStep = isSummaryLikeStep(step);
  const hidesVerboseBody = shouldHideVerboseBodyByDefault(step);
  const summaryText = summarizeExecutionStep(step);
  const [showBody, setShowBody] = useState(false);
  const sanitizedBody = isSummaryStep && step.result_summary ? sanitizeSummaryBody(step.result_summary) : null;
  const detailBody = isSummaryStep ? sanitizedBody : step.result_summary;

  return (
    <article className={`rounded-xl border px-3 py-3 ${tone.className}`}>
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-400">Execution Card</p>
          <p className={`mt-1 text-sm font-medium ${tone.accent}`}>{step.action_name}</p>
        </div>
        <span className="text-[11px] uppercase tracking-wide opacity-90">{tone.badge}</span>
      </div>
      {step.reason && <p className="mt-2 text-xs text-neutral-200/90">{step.reason}</p>}
      {!hidesVerboseBody && step.input_summary && (
        <p className="mt-2 whitespace-pre-wrap text-xs text-neutral-300/90">输入: {step.input_summary}</p>
      )}
      {isSummaryStep || hidesVerboseBody ? (
        <p className="mt-1 whitespace-pre-wrap text-xs text-neutral-300/90">
          摘要: {summaryText}
        </p>
      ) : null}
      {!hidesVerboseBody && !isSummaryStep && (step.result_summary || step.summary) && (
        <p className="mt-1 whitespace-pre-wrap text-xs text-neutral-300/90">结果: {step.result_summary ?? step.summary}</p>
      )}
      {hidesVerboseBody && detailBody ? (
        <div className="mt-3">
          <button
            type="button"
            onClick={() => setShowBody((value) => !value)}
            className="text-xs text-neutral-400 transition hover:text-neutral-200"
          >
            {isSummaryStep ? (showBody ? "收起整理正文" : "查看整理正文") : showBody ? "收起详细结果" : "查看详细结果"}
          </button>
          {showBody ? (
            <pre className="mt-2 whitespace-pre-wrap text-xs text-neutral-300/90">{detailBody}</pre>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}
