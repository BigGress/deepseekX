import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { PreviewContent } from "../../types";

interface MarkdownPreviewProps {
  content: Extract<PreviewContent, { kind: "markdown" }>;
}

export default function MarkdownPreview({ content }: MarkdownPreviewProps) {
  return (
    <div className="h-full overflow-auto p-4">
      <div className="prose prose-invert prose-sm max-w-none
        prose-headings:text-neutral-200
        prose-p:text-neutral-300
        prose-code:text-blue-300 prose-code:bg-neutral-800
        prose-pre:bg-neutral-800 prose-pre:text-neutral-300
        prose-a:text-blue-400 prose-strong:text-neutral-200
        prose-blockquote:border-neutral-600 prose-blockquote:text-neutral-400">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>
          {content.markdown}
        </ReactMarkdown>
      </div>
    </div>
  );
}
