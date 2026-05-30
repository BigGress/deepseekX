import TurnItem from "../components/TurnItem";
import type { Turn } from "../types";

const MOCK_SUCCESS_SUMMARY_TURN: Turn = {
  id: "turn-preview-success-summary",
  user_input: "帮我总结 Snowflake 的核心投资观点。",
  thinking_steps: [],
  final_response: `# Snowflake 投资观点

- **短期**：更取决于收入增速能否重新抬升
- **中期**：高估值会持续放大波动
- **策略**：适合小仓位跟踪，而不是追涨

> 如果你看重中长期数据平台渗透率，可以把它当作成长标的观察名单。

建议重点看 \`product revenue growth\` 和 \`net revenue retention\`。`,
  agent_goal_status: "done",
  duration_ms: 2140,
  retry_count: 0,
  agent_steps: [
    {
      action_name: "retrieve_context",
      reason: "汇总近期研究材料与市场观点",
      input_summary: "query=Snowflake earnings guidance valuation",
      result_summary:
        "web_search | https://example.com/snowflake-q1 | confidence=2.5\nweb_search | https://example.com/snowflake-valuation | confidence=2.1\nnext_hint=https://example.com/snowflake-valuation",
      summary: "命中 2 条研究上下文",
      status: "completed",
      is_error: false,
    },
    {
      action_name: "summarize_findings",
      reason: "整理研究结果，输出投资判断",
      input_summary: "focus=Snowflake 研究结论, output_format=ResearchBrief",
      result_summary:
        "聚焦主题: Snowflake 研究结论\n输出格式: research_brief\n\nweb_search | https://example.com/snowflake-q1 | confidence=2.5\n- https://example.com/snowflake-valuation\n来源: https://example.com/snowflake-notes\n关键结论一：收入增速放缓后，估值对业绩恢复更敏感。\nhttps://example.com/snowflake-q1\nnext_hint=https://example.com/snowflake-valuation\n关键结论二：产品扩张和 AI 叙事提供中长期上行，但短期波动较大。",
      summary: "已整理研究简报，提炼为 2 个关键结论",
      status: "completed",
      is_error: false,
    },
  ],
};

const MOCK_CHAT_TOOL_TURN: Turn = {
  id: "turn-preview-1",
  user_input: "帮我检查这个项目能不能构建，并顺手看一下 README 里有没有安装说明。",
  request_attachments: [
    { kind: "skill", name: "browser" },
    { kind: "mcp", name: "fetch" },
    { kind: "file", name: "README.md" },
  ],
  thinking_steps: [
    {
      thinking: "先确认文档，再做最小构建验证，避免直接给出未经验证的结论。",
      tool_calls: [
        {
          tool_call_id: "tool-1",
          tool_name: "read_files",
          tool_input: "{\"paths\":[\"README.md\"]}",
        },
      ],
      tool_results: [
        {
          tool_call_id: "tool-1",
          is_error: false,
          summary: "已读取 README.md",
        },
      ],
    },
  ],
  final_response:
    "我先读取了 README，再执行了构建验证。当前前端构建通过，README 里也有基本安装说明，但没有补充本地开发端口约定。",
  agent_goal_status: "needs_confirmation",
  duration_ms: 1820,
  retry_count: 0,
  agent_steps: [
    {
      action_name: "read_files",
      reason: "查看 README 是否包含安装与启动说明",
      input_summary: "README.md",
      result_summary: "成功读取 1 个文件",
      summary: "成功读取 1 个文件",
      status: "completed",
      is_error: false,
      preview_type: "full",
      after_preview: "## 安装\n\npnpm install\n\n## 启动\n\npnpm dev",
    },
    {
      action_name: "verify_checks",
      reason: "确认当前项目仍可构建",
      input_summary: "web build: npm run build @ .",
      result_summary:
        "## web build\ncommand=npm run build\ncwd=.\nstatus=pass\nexit_code=0\nstdout:\n...\nstderr:\n",
      summary: "验证通过：1/1",
      status: "completed",
      is_error: false,
      file_operations: [
        {
          path: "README.md",
          operation: "inspect",
        },
      ],
      after_preview: "- [pass] web build (npm run build)",
    },
    {
      action_name: "apply_patch",
      reason: "补充 README 中缺失的本地开发端口说明",
      input_summary: "README.md",
      result_summary: "已生成补丁预览，等待用户确认是否写入",
      summary: "补丁预览已准备",
      status: "blocked",
      is_error: false,
      requires_confirmation: true,
      before_preview: "## 启动\n\npnpm dev",
      after_preview: "## 启动\n\npnpm dev\n\n默认端口: 1420",
      diff_preview: "@@ -1,3 +1,5 @@\n ## 启动\n \n pnpm dev\n+\n+默认端口: 1420",
      changed_ranges: ["README.md:3-5"],
      file_operations: [
        {
          path: "README.md",
          operation: "modify",
          changed_ranges: ["3-5"],
        },
      ],
    },
  ],
};

export default function TurnItemPreview() {
  return (
    <div className="min-h-screen bg-neutral-950 px-6 py-10 text-neutral-100">
      <div className="mx-auto max-w-4xl space-y-6">
        <header className="space-y-2">
          <h1 className="text-xl font-semibold">TurnItem Chat Tool Preview</h1>
          <p className="text-sm text-neutral-400">
            这个调试页用于验证结果优先、过程后置的 Turn 展示策略。
          </p>
        </header>

        <section className="space-y-3 rounded-2xl border border-neutral-800 bg-neutral-925 p-4">
          <div>
            <h2 className="text-sm font-medium text-neutral-100">成功态研究结果</h2>
            <p className="mt-1 text-xs text-neutral-400">
              Execution Detail 默认折叠，`summarize_findings` 先显示一行摘要，完整正文需二级展开且已过滤 retrieval dump。
            </p>
          </div>
          <TurnItem turn={MOCK_SUCCESS_SUMMARY_TURN} />
        </section>

        <section className="space-y-3 rounded-2xl border border-neutral-800 bg-neutral-925 p-4">
          <div>
            <h2 className="text-sm font-medium text-neutral-100">异常态自动展开</h2>
            <p className="mt-1 text-xs text-neutral-400">
              `needs_confirmation` 会自动展开执行过程，方便用户直接看到阻塞原因和待确认动作。
            </p>
          </div>
          <TurnItem turn={MOCK_CHAT_TOOL_TURN} />
        </section>
      </div>
    </div>
  );
}
