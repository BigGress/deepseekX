import { useMemo } from "react";
import type { PreviewContent } from "../../types";

interface HtmlPreviewProps {
  content: Extract<PreviewContent, { kind: "html" }>;
}

export default function HtmlPreview({ content }: HtmlPreviewProps) {
  const blobUrl = useMemo(() => {
    const blob = new Blob([content.html], { type: "text/html" });
    return URL.createObjectURL(blob);
  }, [content.html]);

  return (
    <div className="h-full">
      <iframe
        src={blobUrl}
        sandbox="allow-same-origin"
        className="w-full h-full border-0 bg-white"
        title="HTML 预览"
      />
    </div>
  );
}
