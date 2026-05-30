import { useMemo, useState } from "react";
import type { LlmRequestLogEntry } from "../types";

interface LlmDebugPanelProps {
  entries: LlmRequestLogEntry[];
}

export default function LlmDebugPanel({ entries }: LlmDebugPanelProps) {
  const [expandedIds, setExpandedIds] = useState<Record<string, boolean>>({});
  const [panelExpanded, setPanelExpanded] = useState(false);
  const reversedEntries = useMemo(() => [...entries].reverse(), [entries]);

  if (entries.length === 0) {
    return (
      <section className="shrink-0 border-t border-neutral-800 bg-neutral-950/90 px-4 py-3">
        <div className="mx-auto max-w-5xl">
          <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-500">Debug Mode</p>
          <p className="mt-2 text-sm text-neutral-400">当前还没有捕获到 LLM 接口返回。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="shrink-0 border-t border-neutral-800 bg-neutral-950/95 px-4 py-3">
      <div className="mx-auto max-w-5xl space-y-3">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-500">Debug Mode</p>
            <p className="mt-1 text-sm text-neutral-300">最近 {entries.length} 条 LLM 接口返回</p>
          </div>
          <button
            type="button"
            onClick={() => setPanelExpanded((value) => !value)}
            className="rounded-md border border-neutral-700 bg-neutral-900 px-3 py-1.5 text-xs text-neutral-300 transition-colors hover:bg-neutral-800"
          >
            {panelExpanded ? "收起" : "展开"}
          </button>
        </div>

        <div
          className={`overflow-y-auto transition-[max-height] duration-200 ${
            panelExpanded ? "max-h-80" : "max-h-28"
          }`}
        >
          <div className="space-y-2 pr-1">
          {reversedEntries.map((entry) => {
            const expanded = expandedIds[entry.request_id] ?? false;
            return (
              <article key={entry.request_id} className="rounded-xl border border-neutral-800 bg-neutral-900/80">
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
                      <span>{new Date(entry.timestamp).toLocaleString()}</span>
                      <span>{entry.model}</span>
                      <span>{entry.duration_ms}ms</span>
                      <span>{entry.web_search_enabled ? "web_search=on" : "web_search=off"}</span>
                      <span className={entry.success ? "text-emerald-300" : "text-rose-300"}>
                        {entry.success ? "success" : "error"}
                      </span>
                    </div>
                    <p className="mt-2 truncate text-sm text-neutral-200">{entry.request_id}</p>
                    {entry.error ? <p className="mt-1 text-xs text-rose-300">{entry.error}</p> : null}
                  </div>
                  <span className="text-xs text-neutral-500">{expanded ? "▲" : "▼"}</span>
                </button>

                {expanded ? (
                  <div className="border-t border-neutral-800 px-4 py-3">
                    <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-500">Endpoint</p>
                    <p className="mt-1 break-all text-xs text-neutral-300">{entry.endpoint}</p>
                    <p className="mt-3 text-[11px] uppercase tracking-[0.18em] text-neutral-500">Raw Response</p>
                    <pre className="mt-2 max-h-80 overflow-auto rounded-md bg-neutral-950 p-3 text-[11px] text-neutral-200">
                      <code>{entry.response_text ?? "(empty response)"}</code>
                    </pre>
                  </div>
                ) : null}
              </article>
            );
          })}
          </div>
        </div>
      </div>
    </section>
  );
}
