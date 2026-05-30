import { useMemo, useState } from "react";
import type { LlmDebugResponse } from "../../types";

interface TurnDebugPanelProps {
  entries?: LlmDebugResponse[] | null;
}

export default function TurnDebugPanel({ entries }: TurnDebugPanelProps) {
  const [expandedIds, setExpandedIds] = useState<Record<string, boolean>>({});
  const reversedEntries = useMemo(() => [...(entries ?? [])].reverse(), [entries]);

  if (reversedEntries.length === 0) {
    return null;
  }

  return (
    <div className="flex justify-start">
      <section className="max-w-[85%] w-full rounded-2xl border border-fuchsia-900/60 bg-fuchsia-950/20 px-4 py-3">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-[11px] uppercase tracking-[0.22em] text-fuchsia-300/80">LLM Debug</p>
            <p className="mt-1 text-sm text-neutral-100">本轮捕获到 {reversedEntries.length} 条模型原始返回</p>
          </div>
        </div>

        <div className="mt-4 space-y-2">
          {reversedEntries.map((entry) => {
            const expanded = expandedIds[entry.request_id] ?? false;
            return (
              <article key={entry.request_id} className="rounded-xl border border-fuchsia-900/40 bg-neutral-950/70">
                <button
                  type="button"
                  onClick={() =>
                    setExpandedIds((prev) => ({
                      ...prev,
                      [entry.request_id]: !expanded,
                    }))
                  }
                  className="flex w-full items-start justify-between gap-3 px-4 py-3 text-left"
                >
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2 text-xs text-neutral-400">
                      <span>{entry.model}</span>
                      <span>{entry.duration_ms}ms</span>
                      <span>{entry.web_search_enabled ? "web_search=on" : "web_search=off"}</span>
                    </div>
                    <p className="mt-2 truncate text-sm text-neutral-200">{entry.request_id}</p>
                    <p className="mt-1 truncate text-xs text-neutral-500">{entry.endpoint}</p>
                  </div>
                  <span className="text-xs text-neutral-500">{expanded ? "▲" : "▼"}</span>
                </button>

                {expanded ? (
                  <div className="border-t border-fuchsia-900/30 px-4 py-3">
                    <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-500">Raw Response</p>
                    <pre className="mt-2 max-h-80 overflow-auto rounded-md bg-neutral-950 p-3 text-[11px] text-neutral-200">
                      <code>{entry.response_text || "(empty response)"}</code>
                    </pre>
                  </div>
                ) : null}
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}
