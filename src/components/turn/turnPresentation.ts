import type { AgentStep, RequestAttachment, Turn } from "../../types";

export function formatGoalStatus(status?: string | null) {
  switch (status) {
    case "done":
      return "done";
    case "blocked":
      return "blocked";
    case "failed":
      return "failed";
    case "needs_confirmation":
      return "needs_confirmation";
    default:
      return "running";
  }
}

export function extractApprovalPrompt(turn: Turn) {
  const steps = turn.agent_steps ?? [];
  const candidate = [...steps]
    .reverse()
    .find((step) => step.requires_confirmation || step.action_name === "ask_user");

  if (!candidate) {
    return turn.final_response ?? null;
  }

  return candidate.result_summary || candidate.summary || candidate.reason || turn.final_response || null;
}

export function hasArtifacts(step: AgentStep) {
  return Boolean(
    step.before_preview ||
      step.after_preview ||
      step.diff_preview ||
      (step.changed_ranges && step.changed_ranges.length > 0) ||
      (step.file_operations && step.file_operations.length > 0),
  );
}

export function shouldExpandExecutionByDefault(turn: Turn) {
  const status = formatGoalStatus(turn.agent_goal_status);
  return status === "blocked" || status === "failed" || status === "needs_confirmation";
}

export function isSummaryLikeStep(step: AgentStep) {
  return step.action_name === "summarize_findings";
}

export function summarizeExecutionStep(step: AgentStep) {
  const candidates = [step.summary, step.result_summary, step.reason, step.action_name];

  for (const candidate of candidates) {
    if (!candidate) {
      continue;
    }

    const firstMeaningfulLine = candidate
      .split("\n")
      .map((line) => line.trim())
      .find(Boolean);

    if (firstMeaningfulLine) {
      return firstMeaningfulLine;
    }
  }

  return step.action_name;
}

export function shouldHideVerboseBodyByDefault(step: AgentStep) {
  return step.action_name === "summarize_findings" || step.action_name === "retrieve_context";
}

export function sanitizeSummaryBody(text: string) {
  const filtered = text
    .split("\n")
    .filter((line) => {
      const trimmed = line.trim();

      if (!trimmed) {
        return true;
      }

      const normalized = trimmed.replace(/^[-*]\s+/, "");

      if (
        normalized.startsWith("web_search | ") ||
        normalized.startsWith("workspace_code | ") ||
        normalized.startsWith("workspace_docs | ") ||
        normalized.startsWith("user_knowledge_base | ") ||
        normalized.startsWith("mcp | ") ||
        normalized.startsWith("fetch_url | ") ||
        normalized.startsWith("next_hint=") ||
        normalized.startsWith("user_knowledge_base: unavailable") ||
        normalized.startsWith("来源: http://") ||
        normalized.startsWith("来源: https://")
      ) {
        return false;
      }

      if (/^https?:\/\/\S+$/.test(normalized)) {
        return false;
      }

      if (/^\[[^\]]+\]\(https?:\/\/\S+\)$/.test(normalized)) {
        return false;
      }

      return true;
    })
    .join("\n")
    .trim();

  return filtered || "该步骤仅生成了整理摘要，未保留额外正文。";
}

export function classifyResultCard(turn: Turn): "answer" | "research" | "recommendation" | "error" {
  const response = (turn.final_response ?? "").toLowerCase();
  const steps = turn.agent_steps ?? [];

  if (
    turn.agent_goal_status === "blocked" ||
    turn.agent_goal_status === "failed" ||
    turn.agent_goal_status === "needs_confirmation" ||
    turn.error_stage ||
    response.includes("失败") ||
    response.includes("错误")
  ) {
    return "error";
  }

  if (
    steps.some((step) =>
      ["retrieve_context", "fetch_url", "call_mcp_tool"].includes(step.action_name),
    ) ||
    response.includes("来源") ||
    response.includes("价格") ||
    response.includes("数据")
  ) {
    return "research";
  }

  if (
    response.includes("建议") ||
    response.includes("下一步") ||
    response.includes("推荐") ||
    response.includes("策略")
  ) {
    return "recommendation";
  }

  return "answer";
}

export function attachmentLabel(attachment: RequestAttachment) {
  switch (attachment.kind) {
    case "skill":
      return `/skill ${attachment.name}`;
    case "mcp":
      return `/mcp ${attachment.name}`;
    case "file":
      return `@${attachment.name}`;
    default:
      return attachment.name;
  }
}

export function stepTone(step: AgentStep) {
  if (step.requires_confirmation) {
    return {
      badge: "PENDING",
      className: "border-amber-900/70 bg-amber-950/30 text-amber-100",
      accent: "text-amber-300",
    };
  }

  if (step.is_error) {
    return {
      badge: "FAILED",
      className: "border-rose-900/70 bg-rose-950/30 text-rose-200",
      accent: "text-rose-300",
    };
  }

  if (["retrieve_context", "fetch_url", "call_mcp_tool"].includes(step.action_name)) {
    return {
      badge: (step.status ?? "completed").toUpperCase(),
      className: "border-sky-900/70 bg-sky-950/25 text-sky-100",
      accent: "text-sky-300",
    };
  }

  if (["apply_patch", "write_files", "create_files", "rename_files", "delete_files"].includes(step.action_name)) {
    return {
      badge: (step.status ?? "completed").toUpperCase(),
      className: "border-violet-900/70 bg-violet-950/25 text-violet-100",
      accent: "text-violet-300",
    };
  }

  if (["run_command", "verify_checks"].includes(step.action_name)) {
    return {
      badge: (step.status ?? "completed").toUpperCase(),
      className: "border-cyan-900/70 bg-cyan-950/25 text-cyan-100",
      accent: "text-cyan-300",
    };
  }

  return {
    badge: (step.status ?? "completed").toUpperCase(),
    className: "border-emerald-900/70 bg-emerald-950/25 text-emerald-100",
    accent: "text-emerald-300",
  };
}
