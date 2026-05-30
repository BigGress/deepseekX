import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import TurnItem from "./TurnItem";
import type { AgentStep, Turn } from "../types";

function buildAgentStep(overrides: Partial<AgentStep> = {}): AgentStep {
  return {
    action_name: "summarize_findings",
    summary: "已整理研究简报，提炼为 4 个结论",
    result_summary:
      "聚焦主题: Snowflake\nweb_search | https://example.com | confidence=2.5\nworkspace_docs | docs/notes.md | confidence=1.3\n- https://foo.example/list\n来源: https://foo.example/source\n[延伸阅读](https://foo.example/markdown)\n关键结论第一条。\nhttps://foo.example/a\nnext_hint=refine\n关键结论第二条。",
    is_error: false,
    ...overrides,
  };
}

function buildTurn(overrides: Partial<Turn> = {}): Turn {
  return {
    id: "turn-execution-policy",
    user_input: "总结当前排查结果",
    thinking_steps: [],
    final_response: "已完成结果总结。",
    agent_goal_status: "done",
    agent_steps: [buildAgentStep()],
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
});

describe("TurnItem", () => {
  it("keeps execution detail collapsed by default for successful turns until expanded", () => {
    const turn = buildTurn({
      agent_goal_status: "done",
      agent_steps: [
        buildAgentStep(),
        buildAgentStep({
          action_name: "retrieve_context",
          summary: "命中 3 条上下文结果",
          result_summary: "web_search | https://retrieval.example | confidence=1.8",
        }),
      ],
    });

    render(<TurnItem turn={turn} />);

    expect(screen.getByText("Execution Detail")).toBeInTheDocument();
    expect(screen.getByText("查看执行过程")).toBeInTheDocument();
    expect(screen.queryByText(/关键结论第一条/)).not.toBeInTheDocument();
    expect(screen.queryByText(/已整理研究简报，提炼为 4 个结论/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Execution Detail/i }));

    expect(screen.getByText("执行过程详情")).toBeInTheDocument();
    expect(screen.getByText(/已整理研究简报，提炼为 4 个结论/)).toBeInTheDocument();
    expect(screen.queryByText(/关键结论第一条/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "查看整理正文" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "查看整理正文" }));

    expect(screen.getByText(/关键结论第一条。/)).toBeInTheDocument();
    expect(screen.getByText(/关键结论第二条。/)).toBeInTheDocument();
    expect(screen.queryByText(/web_search \| https:\/\/example.com/)).not.toBeInTheDocument();
    expect(screen.queryByText(/workspace_docs \| docs\/notes.md/)).not.toBeInTheDocument();
    expect(screen.queryByText(/https:\/\/foo.example\/list/)).not.toBeInTheDocument();
    expect(screen.queryByText(/来源: https:\/\/foo.example\/source/)).not.toBeInTheDocument();
    expect(screen.queryByText(/\[延伸阅读\]\(https:\/\/foo.example\/markdown\)/)).not.toBeInTheDocument();
    expect(screen.queryByText(/next_hint=refine/)).not.toBeInTheDocument();
  });

  it("auto-expands execution detail for blocked turns so the relevant step body is visible immediately", () => {
    const turn = buildTurn({
      id: "turn-blocked",
      agent_goal_status: "blocked",
      final_response: "执行被阻塞。",
      agent_steps: [
        buildAgentStep({
          action_name: "fetch_url",
          summary: "网页抓取失败",
          result_summary: "状态 blocked 时，应立即显示这段执行详情正文。",
          is_error: true,
        }),
      ],
    });

    render(<TurnItem turn={turn} />);

    expect(screen.getByText("Execution Detail")).toBeInTheDocument();
    expect(screen.getByText(/状态 blocked 时，应立即显示这段执行详情正文。/)).toBeInTheDocument();
  });

  it("auto-expands execution detail for failed turns", () => {
    const turn = buildTurn({
      id: "turn-failed",
      agent_goal_status: "failed",
      final_response: "执行失败。",
      agent_steps: [
        buildAgentStep({
          action_name: "fetch_url",
          summary: "网页抓取失败",
          result_summary: "状态 failed 时，应立即显示这段执行详情正文。",
          is_error: true,
        }),
      ],
    });

    render(<TurnItem turn={turn} />);

    expect(screen.getByText(/状态 failed 时，应立即显示这段执行详情正文。/)).toBeInTheDocument();
  });

  it("auto-expands execution detail for needs_confirmation turns", () => {
    const turn = buildTurn({
      id: "turn-needs-confirmation",
      agent_goal_status: "needs_confirmation",
      final_response: "需要用户确认后继续。",
      agent_steps: [
        buildAgentStep({
          action_name: "fetch_url",
          summary: "URL 抓取被权限门拦截",
          result_summary: "risk_level=medium; 抓取返回字符数过大",
          is_error: true,
          requires_confirmation: true,
        }),
      ],
    });

    render(<TurnItem turn={turn} />);

    expect(screen.getByText("Status Card")).toBeInTheDocument();
    expect(screen.getByText("PENDING")).toBeInTheDocument();
    expect(screen.getByText(/risk_level=medium; 抓取返回字符数过大/)).toBeInTheDocument();
  });

  it("auto-expands an existing turn when it transitions into an exceptional state", () => {
    const initialTurn = buildTurn({
      id: "turn-transition",
      agent_goal_status: "done",
    });

    const { rerender } = render(<TurnItem turn={initialTurn} />);

    expect(screen.queryByText(/状态 blocked 后重新渲染时，应立即展开详情正文。/)).not.toBeInTheDocument();

    rerender(
      <TurnItem
        turn={buildTurn({
          id: "turn-transition",
          agent_goal_status: "blocked",
          final_response: "执行被阻塞。",
          agent_steps: [
            buildAgentStep({
              action_name: "fetch_url",
              summary: "网页抓取失败",
              result_summary: "状态 blocked 后重新渲染时，应立即展开详情正文。",
              is_error: true,
            }),
          ],
        })}
      />,
    );

    expect(screen.getByText(/状态 blocked 后重新渲染时，应立即展开详情正文。/)).toBeInTheDocument();
  });
});
