import type { AgentFollowUpAction, Turn } from "../../types";
import { extractApprovalPrompt, formatGoalStatus } from "./turnPresentation";

interface StatusPanelProps {
  turn: Turn;
  isLatest?: boolean;
  isLoading?: boolean;
  onAgentFollowUp?: (turn: Turn, action: AgentFollowUpAction) => void;
}

export default function StatusPanel({
  turn,
  isLatest = false,
  isLoading = false,
  onAgentFollowUp,
}: StatusPanelProps) {
  const status = formatGoalStatus(turn.agent_goal_status);
  const approvalPrompt = extractApprovalPrompt(turn);
  const canFollowUp =
    Boolean(onAgentFollowUp) &&
    turn.agent_goal_status === "needs_confirmation" &&
    Boolean(approvalPrompt) &&
    isLatest;
  const isRunning = !turn.final_response && isLatest && isLoading;

  if (!turn.agent_goal_status && !turn.error_stage && !canFollowUp && !isRunning) {
    return null;
  }

  const tone =
    status === "blocked" || status === "failed"
      ? "border-rose-900/70 bg-rose-950/25"
      : status === "needs_confirmation"
        ? "border-amber-900/70 bg-amber-950/25"
        : "border-emerald-900/70 bg-emerald-950/20";

  return (
    <div className="flex justify-start">
      <section className={`max-w-[85%] w-full rounded-2xl border px-4 py-3 ${tone}`}>
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-500">State & Actions</p>
            <p className="mt-1 text-sm font-medium text-neutral-100">
              {isRunning ? "running" : status}
            </p>
          </div>
          <StatusBadge status={isRunning ? "running" : status} />
        </div>

        <div className="mt-3 flex flex-wrap gap-2 text-[11px] text-neutral-400">
          {typeof turn.duration_ms === "number" ? <span>耗时 {formatDuration(turn.duration_ms)}</span> : null}
          {turn.error_stage ? <span>失败阶段 {turn.error_stage}</span> : null}
          {typeof turn.retry_count === "number" && turn.retry_count > 0 ? (
            <span>重试 {turn.retry_count} 次</span>
          ) : null}
        </div>

        {canFollowUp && approvalPrompt ? (
          <div className="mt-4 rounded-xl border border-amber-900/70 bg-amber-950/25 p-3">
            <p className="text-[11px] uppercase tracking-[0.18em] text-amber-300">ApprovalCard</p>
            <p className="mt-2 whitespace-pre-wrap text-sm text-neutral-100">{approvalPrompt}</p>
            <div className="mt-3 flex flex-wrap gap-2">
              <FollowUpButton
                disabled={isLoading}
                label="继续执行"
                onClick={() => onAgentFollowUp?.(turn, "approve")}
                tone="emerald"
              />
              <FollowUpButton
                disabled={isLoading}
                label="拒绝该动作"
                onClick={() => onAgentFollowUp?.(turn, "reject")}
                tone="rose"
              />
              <FollowUpButton
                disabled={isLoading}
                label="重新尝试"
                onClick={() => onAgentFollowUp?.(turn, "retry")}
                tone="neutral"
              />
            </div>
          </div>
        ) : null}

        {isRunning ? (
          <div className="mt-4 flex items-center gap-1">
            <span className="h-2 w-2 animate-bounce rounded-full bg-neutral-500" style={{ animationDelay: "0ms" }} />
            <span className="h-2 w-2 animate-bounce rounded-full bg-neutral-500" style={{ animationDelay: "150ms" }} />
            <span className="h-2 w-2 animate-bounce rounded-full bg-neutral-500" style={{ animationDelay: "300ms" }} />
          </div>
        ) : null}
      </section>
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const className =
    status === "blocked" || status === "failed"
      ? "border-rose-700/70 bg-rose-600/20 text-rose-200"
      : status === "needs_confirmation"
        ? "border-amber-700/70 bg-amber-600/20 text-amber-200"
        : status === "done"
          ? "border-emerald-700/70 bg-emerald-600/20 text-emerald-200"
          : "border-neutral-700 bg-neutral-900 text-neutral-200";

  return <span className={`rounded-full border px-2.5 py-1 text-[11px] uppercase tracking-wide ${className}`}>{status}</span>;
}

function FollowUpButton({
  disabled,
  label,
  onClick,
  tone,
}: {
  disabled: boolean;
  label: string;
  onClick: () => void;
  tone: "emerald" | "rose" | "neutral";
}) {
  const className =
    tone === "emerald"
      ? "border-emerald-700 bg-emerald-600/20 text-emerald-200 hover:bg-emerald-600/30"
      : tone === "rose"
        ? "border-rose-700 bg-rose-600/20 text-rose-200 hover:bg-rose-600/30"
        : "border-neutral-700 bg-neutral-900 text-neutral-200 hover:bg-neutral-800";

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`rounded-md border px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
    >
      {label}
    </button>
  );
}

function formatDuration(durationMs: number) {
  if (durationMs < 1000) {
    return `${durationMs}ms`;
  }

  const seconds = durationMs / 1000;
  return `${seconds.toFixed(seconds >= 10 ? 0 : 1)}s`;
}
