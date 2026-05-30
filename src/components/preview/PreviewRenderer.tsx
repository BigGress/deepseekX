import type { PreviewContent } from "../../types";
import TextPreview from "./TextPreview";
import MarkdownPreview from "./MarkdownPreview";
import HtmlPreview from "./HtmlPreview";
import TablePreview from "./TablePreview";
import MediaPreview from "./MediaPreview";
import FallbackPreview from "./FallbackPreview";

interface PreviewRendererProps {
  content: PreviewContent;
}

export default function PreviewRenderer({ content }: PreviewRendererProps) {
  switch (content.kind) {
    case "text":
      return <TextPreview content={content} />;
    case "markdown":
      return <MarkdownPreview content={content} />;
    case "html":
      return <HtmlPreview content={content} />;
    case "table":
      return <TablePreview content={content} />;
    case "media":
      return <MediaPreview content={content} />;
    case "pages":
      return <FallbackPreview message={`${content.pages.length} 页幻灯片（页图渲染即将支持）`} />;
    case "fallback":
      return <FallbackPreview message={content.message} />;
  }
}
