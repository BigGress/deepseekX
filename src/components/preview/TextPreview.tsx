import type { PreviewContent } from "../../types";

interface TextPreviewProps {
  content: Extract<PreviewContent, { kind: "text" }>;
}

export default function TextPreview({ content }: TextPreviewProps) {
  return (
    <div className="h-full overflow-auto bg-neutral-950">
      {content.language && (
        <div className="px-4 py-1 text-xs text-neutral-600 border-b border-neutral-800 font-mono">
          {content.language}
        </div>
      )}
      <pre className="p-4 text-xs font-mono text-neutral-300 leading-relaxed whitespace-pre-wrap break-all">
        {content.text}
      </pre>
    </div>
  );
}
