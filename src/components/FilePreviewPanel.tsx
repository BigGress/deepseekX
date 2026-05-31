import type { PreviewMode, PreviewState } from "../types";
import PreviewModeTabs from "./preview/PreviewModeTabs";
import PreviewRenderer from "./preview/PreviewRenderer";
import FallbackPreview from "./preview/FallbackPreview";

interface FilePreviewPanelProps {
  state: PreviewState;
  workspaceRoot: string;
  onClose: () => void;
  onSwitchMode: (mode: PreviewMode, workspaceRoot: string) => Promise<void>;
  onOpenFocused: () => void;
}

export default function FilePreviewPanel({
  state, workspaceRoot, onClose, onSwitchMode, onOpenFocused,
}: FilePreviewPanelProps) {
  const { descriptor, selectedMode, isLoading, error, target } = state;

  const fileName = descriptor?.file_name ?? target?.title ?? target?.path.split("/").pop() ?? "文件预览";

  return (
    <div className="flex flex-col w-full bg-neutral-950 h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-neutral-800 shrink-0">
        <span className="flex-1 text-xs text-neutral-200 font-medium truncate" title={target?.path}>
          {fileName}
        </span>
        {descriptor && selectedMode && (
          <PreviewModeTabs
            modes={descriptor.available_modes}
            selected={selectedMode}
            onSelect={(mode) => onSwitchMode(mode, workspaceRoot)}
          />
        )}
        <button
          onClick={onOpenFocused}
          aria-label="放大预览"
          className="text-neutral-500 hover:text-neutral-200 transition-colors p-0.5"
          title="放大预览"
        >
          <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
          </svg>
        </button>
        <button
          onClick={onClose}
          aria-label="关闭"
          className="text-neutral-500 hover:text-neutral-200 transition-colors p-0.5"
        >
          <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Warnings */}
      {descriptor?.warnings && descriptor.warnings.length > 0 && (
        <div className="px-3 py-1.5 bg-yellow-900/20 border-b border-yellow-800/30">
          {descriptor.warnings.map((w, i) => (
            <p key={i} className="text-xs text-yellow-400">{w}</p>
          ))}
        </div>
      )}

      {/* Content area */}
      <div className="flex-1 overflow-hidden">
        {isLoading && !descriptor && (
          <div className="h-full flex items-center justify-center text-neutral-500 text-sm">
            正在加载预览…
          </div>
        )}
        {!isLoading && error && !descriptor && (
          <div className="h-full flex items-center justify-center p-4">
            <p className="text-sm text-red-400 text-center">{error}</p>
          </div>
        )}
        {descriptor?.content && (
          <PreviewRenderer content={descriptor.content} />
        )}
        {descriptor && !descriptor.content && !isLoading && (
          <FallbackPreview message="暂无可显示内容" />
        )}
      </div>

      {/* Footer metadata */}
      {descriptor && (
        <div className="shrink-0 px-3 py-1.5 border-t border-neutral-800 flex gap-3 text-xs text-neutral-600">
          {descriptor.size_bytes != null && (
            <span>{(descriptor.size_bytes / 1024).toFixed(1)} KB</span>
          )}
          {descriptor.mime_type && <span>{descriptor.mime_type}</span>}
          {descriptor.metadata?.line_count && <span>{descriptor.metadata.line_count} 行</span>}
        </div>
      )}
    </div>
  );
}
