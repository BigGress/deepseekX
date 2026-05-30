import type { Turn } from "../../types";
import MessageMarkdown from "./MessageMarkdown";
import { classifyResultCard } from "./turnPresentation";

export default function ResultCardGroup({ turn }: { turn: Turn }) {
  if (!turn.final_response) {
    return null;
  }

  const kind = classifyResultCard(turn);
  const chrome =
    kind === "error"
      ? {
          label: "Status Card",
          className: "border-rose-900/70 bg-rose-950/30 text-rose-50",
          chip: "text-rose-300",
        }
      : kind === "research"
        ? {
            label: "Research Card",
            className: "border-sky-900/70 bg-sky-950/25 text-sky-50",
            chip: "text-sky-300",
          }
        : kind === "recommendation"
          ? {
              label: "Recommendation Card",
              className: "border-amber-900/70 bg-amber-950/25 text-amber-50",
              chip: "text-amber-300",
            }
          : {
              label: "Answer Card",
              className: "border-neutral-800 bg-neutral-900/90 text-neutral-100",
              chip: "text-emerald-300",
            };

  return (
    <div className="flex justify-start">
      <section className={`max-w-[85%] w-full rounded-2xl border px-4 py-3 shadow-sm ${chrome.className}`}>
        <div className="flex items-center justify-between gap-3">
          <p className="text-[11px] uppercase tracking-[0.22em] text-neutral-400">Primary Result</p>
          <span className={`text-[11px] font-medium uppercase tracking-wide ${chrome.chip}`}>
            {chrome.label}
          </span>
        </div>
        <div className="mt-3 prose prose-invert prose-sm max-w-none">
          <MessageMarkdown content={turn.final_response} />
        </div>
      </section>
    </div>
  );
}
