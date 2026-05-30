import { convertFileSrc } from "@tauri-apps/api/core";
import type { PreviewContent } from "../../types";
import FallbackPreview from "./FallbackPreview";

interface MediaPreviewProps {
  content: Extract<PreviewContent, { kind: "media" }>;
}

export default function MediaPreview({ content }: MediaPreviewProps) {
  const src = convertFileSrc(content.url);

  if (content.media_type === "image") {
    return (
      <div className="h-full flex items-center justify-center overflow-auto p-4 bg-neutral-950">
        <img src={src} alt="预览" className="max-w-full max-h-full object-contain" />
      </div>
    );
  }

  if (content.media_type === "video") {
    return (
      <div className="h-full flex items-center justify-center bg-black">
        <video src={src} controls className="max-w-full max-h-full" />
      </div>
    );
  }

  if (content.media_type === "audio") {
    return (
      <div className="h-full flex items-center justify-center p-8">
        <audio src={src} controls className="w-full" />
      </div>
    );
  }

  if (content.media_type === "pdf") {
    return (
      <div className="h-full">
        <embed src={src} type="application/pdf" className="w-full h-full" />
      </div>
    );
  }

  return <FallbackPreview message={`未知媒体类型: ${content.media_type}`} />;
}
