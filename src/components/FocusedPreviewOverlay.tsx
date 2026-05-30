import type { PreviewDescriptor, PreviewMode } from "../types";
import PreviewModeTabs from "./preview/PreviewModeTabs";
import PreviewRenderer from "./preview/PreviewRenderer";

interface FocusedPreviewOverlayProps {
  descriptor: PreviewDescriptor;
  selectedMode: PreviewMode;
  workspaceRoot: string;
  onClose: () => void;
  onSwitchMode: (mode: PreviewMode, workspaceRoot: string) => Promise<void>;
}

export default function FocusedPreviewOverlay({
  descriptor, selectedMode, workspaceRoot, onClose, onSwitchMode,
}: FocusedPreviewOverlayProps) {
  return (
    <div className="fixed inset-0 z-50 bg-neutral-950/95 flex flex-col">
      {/* Header */}
      <div className="flex items-center gap-3 px-4 py-2.5 border-b border-neutral-800 shrink-0">
        <span className="flex-1 text-sm font-medium text-neutral-200 truncate">
          {descriptor.file_name}
        </span>
        <PreviewModeTabs
          modes={descriptor.available_modes}
          selected={selectedMode}
          onSelect={(mode) => onSwitchMode(mode, workspaceRoot)}
        />
        <button
          onClick={onClose}
          aria-label="关闭"
          className="text-neutral-400 hover:text-white transition-colors ml-2"
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Warnings */}
      {descriptor.warnings.length > 0 && (
        <div className="px-4 py-2 bg-yellow-900/20 border-b border-yellow-800/30 shrink-0">
          {descriptor.warnings.map((w, i) => (
            <p key={i} className="text-xs text-yellow-400">{w}</p>
          ))}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {descriptor.content
          ? <PreviewRenderer content={descriptor.content} />
          : <div className="h-full flex items-center justify-center text-neutral-500 text-sm">暂无内容</div>
        }
      </div>
    </div>
  );
}
