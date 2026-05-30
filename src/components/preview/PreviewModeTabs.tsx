import type { PreviewMode } from "../../types";

const MODE_LABELS: Record<PreviewMode, string> = {
  structured: "结构化",
  rendered: "预览",
  raw: "源码",
  metadata: "元数据",
};

interface PreviewModeTabsProps {
  modes: PreviewMode[];
  selected: PreviewMode;
  onSelect: (mode: PreviewMode) => void;
}

export default function PreviewModeTabs({ modes, selected, onSelect }: PreviewModeTabsProps) {
  return (
    <div className="flex gap-1">
      {modes.map((mode) => (
        <button
          key={mode}
          onClick={() => onSelect(mode)}
          className={`px-2 py-0.5 rounded text-xs transition-colors ${
            mode === selected
              ? "bg-neutral-700 text-white"
              : "text-neutral-500 hover:text-neutral-300"
          }`}
        >
          {MODE_LABELS[mode]}
        </button>
      ))}
    </div>
  );
}
