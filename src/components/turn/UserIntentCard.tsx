import type { RequestAttachment } from "../../types";
import { attachmentLabel } from "./turnPresentation";

interface UserIntentCardProps {
  userInput: string;
  attachments?: RequestAttachment[] | null;
}

export default function UserIntentCard({ userInput, attachments }: UserIntentCardProps) {
  const visibleAttachments = attachments ?? [];

  return (
    <section className="space-y-2">
      {userInput ? (
        <div className="flex justify-end">
          <div className="max-w-[85%] rounded-2xl border border-blue-400/20 bg-blue-600 px-4 py-3 text-sm leading-relaxed text-white shadow-[0_10px_30px_rgba(37,99,235,0.15)]">
            <p className="whitespace-pre-wrap">{userInput}</p>
          </div>
        </div>
      ) : null}
      {visibleAttachments.length > 0 && (
        <div className="flex justify-end">
          <div className="max-w-[85%] rounded-xl border border-neutral-800 bg-neutral-900/80 px-3 py-2">
            <p className="text-[11px] uppercase tracking-[0.18em] text-neutral-500">User Intent</p>
            <div className="mt-2 flex flex-wrap gap-2">
              {visibleAttachments.map((attachment) => (
                <span
                  key={`${attachment.kind}-${attachment.name}`}
                  className="rounded-full border border-neutral-700 bg-neutral-950 px-2.5 py-1 text-[11px] text-neutral-300"
                >
                  {attachmentLabel(attachment)}
                </span>
              ))}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
