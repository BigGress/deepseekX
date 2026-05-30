import FileTree from "./FileTree";
import type { Conversation, FileNode } from "../types";

function buildConversationTooltip(conv: Conversation): string {
  return [`标题: ${conv.title}`, `Session ID: ${conv.id}`, `Session Path: ${conv.sessionPath ?? "未生成"}`].join("\n");
}

interface SidebarProps {
  conversations: Conversation[];
  activeId: string;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  fileNodes: FileNode[];
  onSelectFile?: (path: string) => void;
  onPreviewFile?: (path: string) => void;
  previewPath?: string;
  projectName: string;
  onBackToProjects: () => void;
  onOpenSettings: () => void;
}

export default function Sidebar({
  conversations,
  activeId,
  onSelect,
  onNew,
  onDelete,
  fileNodes,
  onSelectFile,
  onPreviewFile,
  previewPath,
  projectName,
  onBackToProjects,
  onOpenSettings,
}: SidebarProps) {
  return (
    <aside className="w-64 min-w-[240px] border-r border-neutral-800 flex flex-col bg-neutral-925 select-none">
      {/* 项目标题 + 返回 */}
      <div className="p-3 border-b border-neutral-800">
        <div className="flex items-center gap-2">
          <button
            onClick={onBackToProjects}
            className="text-neutral-500 hover:text-neutral-300 transition-colors shrink-0"
            title="返回项目列表"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-neutral-200 truncate">{projectName}</p>
          </div>
          <button
            onClick={onOpenSettings}
            className="text-neutral-500 hover:text-neutral-300 transition-colors shrink-0"
            title="项目设置"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </button>
        </div>
      </div>

      {/* 文件树 */}
      <div className="border-b border-neutral-800 max-h-[40%] flex flex-col">
        <div className="px-3 py-2 text-xs text-neutral-500 font-medium shrink-0 select-none">
          文件
        </div>
        <FileTree nodes={fileNodes} onSelectFile={onSelectFile} onPreviewFile={onPreviewFile} previewPath={previewPath} />
      </div>

      {/* 对话区域 */}
      <div className="flex-1 flex flex-col min-h-0">
        <div className="px-3 py-2 border-b border-neutral-800 flex items-center justify-between shrink-0">
          <span className="text-xs text-neutral-500 font-medium select-none">对话</span>
          <button
            onClick={onNew}
            className="text-neutral-500 hover:text-neutral-300 transition-colors"
            title="新建对话"
          >
            <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
            </svg>
          </button>
        </div>

        {/* 对话列表 */}
        <div className="flex-1 overflow-y-auto py-2">
          {conversations.map((conv) => (
            <div
              key={conv.id}
              onClick={() => onSelect(conv.id)}
              title={buildConversationTooltip(conv)}
              className={`group mx-2 my-0.5 px-3 py-2 rounded-md cursor-pointer transition-colors flex items-start justify-between gap-2 ${
                conv.id === activeId
                  ? "bg-neutral-800 text-neutral-100"
                  : "text-neutral-400 hover:bg-neutral-850 hover:text-neutral-200"
              }`}
            >
              <div className="min-w-0 flex-1">
                <div className="text-sm truncate">{conv.title}</div>
                <div className="text-[11px] text-neutral-500 truncate">
                  {conv.id}
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(conv.id);
                }}
                className="opacity-0 group-hover:opacity-100 text-neutral-500 hover:text-red-400 
                           transition-all p-0.5 rounded shrink-0"
                title="删除对话"
              >
                <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          ))}
        </div>
      </div>

      {/* 底部 */}
      <div className="p-3 border-t border-neutral-800 text-xs text-neutral-500">
        <p>DeepSeekX v0.1.0</p>
      </div>
    </aside>
  );
}
