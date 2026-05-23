import { useState } from "react";
import type { AgentStep, Turn, ThinkingStep } from "../types";

interface TurnItemProps {
  turn: Turn;
}

export default function TurnItem({ turn }: TurnItemProps) {
  const [thinkingExpanded, setThinkingExpanded] = useState(false);
  const [agentExpanded, setAgentExpanded] = useState(true);

  const hasThinkingSteps = turn.thinking_steps.length > 0;
  const stepCount = turn.thinking_steps.length;
  const hasAgentSteps = Boolean(turn.agent_steps && turn.agent_steps.length > 0);

  return (
    <div className="space-y-3">
      {/* 用户消息气泡 */}
      {turn.user_input && (
        <div className="flex justify-end">
          <div className="max-w-[85%] rounded-lg px-4 py-3 text-sm leading-relaxed bg-blue-600 text-white">
            <p>{turn.user_input}</p>
          </div>
        </div>
      )}

      {/* 思考过程面板 */}
      {hasThinkingSteps && (
        <div className="flex justify-start">
          <div className="max-w-[85%] w-full">
            {/* 折叠标题 */}
            <button
              onClick={() => setThinkingExpanded(!thinkingExpanded)}
              className="flex items-center gap-2 text-xs text-neutral-500 cursor-pointer hover:text-neutral-400 transition-colors"
            >
              <span className="transform transition-transform duration-150" style={{ display: "inline-block", transform: thinkingExpanded ? "rotate(90deg)" : "rotate(0deg)" }}>
                ▶
              </span>
              <span>思考过程 ({stepCount} 轮)</span>
            </button>

            {/* 展开内容 */}
            {thinkingExpanded && (
              <div className="mt-2 bg-neutral-900 rounded-lg p-3 space-y-2">
                {turn.thinking_steps.map((step, stepIdx) => (
                  <ThinkingStepItem key={stepIdx} step={step} isLast={stepIdx === stepCount - 1} />
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {hasAgentSteps && (
        <div className="flex justify-start">
          <div className="max-w-[85%] w-full rounded-xl border border-emerald-900/60 bg-emerald-950/30 p-3">
            <button
              onClick={() => setAgentExpanded((value) => !value)}
              className="flex w-full items-center justify-between text-left"
            >
              <div>
                <p className="text-xs uppercase tracking-[0.2em] text-emerald-400">Agent Loop</p>
                <p className="mt-1 text-sm text-neutral-200">
                  状态: {formatGoalStatus(turn.agent_goal_status)}
                </p>
              </div>
              <span className="text-xs text-neutral-400">
                {turn.agent_steps?.length} steps {agentExpanded ? "▲" : "▼"}
              </span>
            </button>

            {agentExpanded && (
              <div className="mt-3 space-y-2">
                {turn.agent_steps?.map((step, index) => (
                  <AgentStepItem
                    key={`${step.action_name}-${index}`}
                    step={step}
                    isLast={index === (turn.agent_steps?.length || 1) - 1}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* 最终回复 */}
      {turn.final_response && (
        <div className="flex justify-start">
          <div className="max-w-[85%] rounded-lg px-4 py-3 text-sm leading-relaxed bg-neutral-850 text-neutral-200">
            <div className="prose prose-invert prose-sm max-w-none">
              <MessageContent content={turn.final_response} />
            </div>
          </div>
        </div>
      )}

      {/* 等待中的情况：有 thinking 但没有 final_response */}
      {!turn.final_response && (
        <div className="flex justify-start">
          <div className="bg-neutral-850 rounded-lg px-4 py-3">
            <div className="flex items-center gap-1">
              <span className="w-2 h-2 bg-neutral-500 rounded-full animate-bounce" style={{ animationDelay: "0ms" }} />
              <span className="w-2 h-2 bg-neutral-500 rounded-full animate-bounce" style={{ animationDelay: "150ms" }} />
              <span className="w-2 h-2 bg-neutral-500 rounded-full animate-bounce" style={{ animationDelay: "300ms" }} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function AgentStepItem({ step, isLast }: { step: AgentStep; isLast: boolean }) {
  const statusColor = step.is_error
    ? "text-red-300 border-red-900/70 bg-red-950/30"
    : step.requires_confirmation
      ? "text-amber-300 border-amber-900/70 bg-amber-950/30"
      : "text-emerald-300 border-emerald-900/70 bg-emerald-950/20";

  return (
    <div className={`rounded-lg border px-3 py-2 ${statusColor}`}>
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm font-medium">{step.action_name}</p>
        <span className="text-[11px] uppercase tracking-wide opacity-80">
          {step.status ?? (step.is_error ? "failed" : "completed")}
        </span>
      </div>
      {step.reason && <p className="mt-1 text-xs opacity-90">{step.reason}</p>}
      {step.input_summary && (
        <p className="mt-2 whitespace-pre-wrap text-xs text-neutral-300/90">
          输入: {step.input_summary}
        </p>
      )}
      {(step.result_summary || step.summary) && (
        <p className="mt-1 whitespace-pre-wrap text-xs text-neutral-300/90">
          结果: {step.result_summary ?? step.summary}
        </p>
      )}
      {step.before_preview && (
        <PreviewBlock title="修改前" content={step.before_preview} />
      )}
      {step.after_preview && (
        <PreviewBlock title={step.preview_type === "full" ? "文件预览" : "预览"} content={step.after_preview} />
      )}
      {step.diff_preview && (
        <PreviewBlock title="Diff 预览" content={step.diff_preview} />
      )}
      {step.changed_ranges && step.changed_ranges.length > 0 && (
        <p className="mt-2 whitespace-pre-wrap text-[11px] text-neutral-400">
          变更范围: {step.changed_ranges.join(", ")}
        </p>
      )}
      {!isLast && <div className="mt-2 border-b border-white/10" />}
    </div>
  );
}

function PreviewBlock({ title, content }: { title: string; content: string }) {
  return (
    <div className="mt-2">
      <p className="mb-1 text-[11px] uppercase tracking-wide text-neutral-400">{title}</p>
      <pre className="overflow-x-auto rounded-md bg-neutral-950/70 p-2 text-[11px] text-neutral-200">
        <code>{content}</code>
      </pre>
    </div>
  );
}

// ---------- ThinkingStep 子项 ----------

function ThinkingStepItem({ step, isLast }: { step: ThinkingStep; isLast: boolean }) {
  return (
    <div>
      {/* 思考内容 */}
      {step.thinking && (
        <p className="italic text-neutral-400 text-xs mb-1">💭 {step.thinking}</p>
      )}

      {/* 工具调用 */}
      {step.tool_calls.map((tc) => (
        <div key={tc.tool_call_id} className="mb-1">
          <p className="text-blue-400 text-xs">
            🔧 {tc.tool_name} {tc.tool_input && <span className="text-neutral-500">{tc.tool_input}</span>}
          </p>
        </div>
      ))}

      {/* 工具结果 */}
      {step.tool_results.map((tr) => (
        <div key={tr.tool_call_id} className="mb-1 ml-4">
          <p className={`text-xs ${tr.is_error ? "text-red-400" : "text-green-400"}`}>
            📋 {tr.summary || (tr.is_error ? "执行失败" : "执行成功")}
          </p>
        </div>
      ))}

      {/* 分隔线（非最后一项） */}
      {!isLast && <hr className="border-neutral-800 my-2" />}
    </div>
  );
}

// ---------- Markdown 简单渲染 ----------

function MessageContent({ content }: { content: string }) {
  const parts = content.split(/(```[\s\S]*?```)/g);
  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith("```")) {
          const code = part
            .replace(/```\w*\s*file:\S+\n?/, "")  // 去除 file: 标记
            .replace(/```\w*\n?/, "")             // 普通代码块
            .replace(/```$/, "");
          return (
            <pre
              key={i}
              className="bg-neutral-900 rounded-md p-3 my-2 overflow-x-auto text-xs text-neutral-300"
            >
              <code>{code}</code>
            </pre>
          );
        }
        return <span key={i} className="whitespace-pre-wrap">{part}</span>;
      })}
    </>
  );
}

function formatGoalStatus(status?: string | null) {
  switch (status) {
    case "done":
      return "done";
    case "blocked":
      return "blocked";
    case "needs_confirmation":
      return "needs_confirmation";
    default:
      return "running";
  }
}
