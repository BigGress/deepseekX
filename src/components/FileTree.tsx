import { useState } from "react";
import type { FileNode } from "../types";

interface FileTreeProps {
  nodes: FileNode[];
  onSelectFile?: (path: string) => void;
  onPreviewFile?: (path: string) => void;
  previewPath?: string;
}

export default function FileTree({ nodes, onSelectFile, onPreviewFile, previewPath }: FileTreeProps) {
  return (
    <div className="flex-1 overflow-y-auto py-2 text-xs">
      {nodes.length === 0 ? (
        <p className="text-neutral-600 px-3 py-4 text-center">暂无文件</p>
      ) : (
        nodes.map((node) => (
          <TreeNode
            key={node.path}
            node={node}
            depth={0}
            onSelectFile={onSelectFile}
            onPreviewFile={onPreviewFile}
            previewPath={previewPath}
          />
        ))
      )}
    </div>
  );
}

interface TreeNodeProps {
  node: FileNode;
  depth: number;
  onSelectFile?: (path: string) => void;
  onPreviewFile?: (path: string) => void;
  previewPath?: string;
}

function TreeNode({ node, depth, onSelectFile, onPreviewFile, previewPath }: TreeNodeProps) {
  const [expanded, setExpanded] = useState(false);

  if (node.is_directory) {
    const hasChildren = node.children && node.children.length > 0;
    return (
      <div>
        <div
          onClick={() => hasChildren && setExpanded(!expanded)}
          className={`flex items-center gap-1.5 px-3 py-1 cursor-pointer hover:bg-neutral-850 transition-colors select-none ${
            hasChildren ? "text-neutral-400 hover:text-neutral-200" : "text-neutral-600"
          }`}
          style={{ paddingLeft: `${12 + depth * 14}px` }}
        >
          <span className="w-3.5 h-3.5 flex items-center justify-center shrink-0">
            {hasChildren ? (
              <svg
                className={`w-3 h-3 transition-transform ${expanded ? "rotate-90" : ""}`}
                fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
              </svg>
            ) : (
              <div className="w-3" />
            )}
          </span>
          <svg className="w-3.5 h-3.5 shrink-0 text-neutral-500" viewBox="0 0 24 24" fill="currentColor">
            <path d="M20 6h-8l-2-2H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 12H4V8h16v10z" />
          </svg>
          <span className="truncate">{node.name}</span>
        </div>
        {expanded && hasChildren && (
          <div>
            {node.children!.map((child) => (
              <TreeNode
                key={child.path}
                node={child}
                depth={depth + 1}
                onSelectFile={onSelectFile}
                onPreviewFile={onPreviewFile}
                previewPath={previewPath}
              />
            ))}
          </div>
        )}
      </div>
    );
  }

  const isActive = node.path === previewPath;

  return (
    <div
      data-path={node.path}
      onClick={() => { onPreviewFile?.(node.path); onSelectFile?.(node.path); }}
      onDoubleClick={() => onPreviewFile?.(node.path)}
      className={`flex items-center gap-1.5 px-3 py-1 cursor-pointer hover:bg-neutral-850 transition-colors select-none ${
        isActive ? "text-blue-400" : "text-neutral-500 hover:text-neutral-300"
      }`}
      style={{ paddingLeft: `${12 + depth * 14}px` }}
      title={node.path}
    >
      <span className="w-3.5 h-3.5 flex items-center justify-center shrink-0">
        <div className="w-3" />
      </span>
      <svg className="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="currentColor">
        <path d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm-1 2l5 5h-5V4z" />
      </svg>
      <span className="truncate flex-1">{node.name}</span>
    </div>
  );
}
