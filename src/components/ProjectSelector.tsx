import type { ProjectRow } from "../types";

interface ProjectSelectorProps {
  projects: ProjectRow[];
  onSelect: (project: ProjectRow) => void;
  onCreateNew: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
}

export default function ProjectSelector({
  projects,
  onSelect,
  onCreateNew,
  onDelete,
  onOpenSettings,
}: ProjectSelectorProps) {
  // 空状态：引导创建
  if (projects.length === 0) {
    return (
      <div className="flex h-screen w-screen bg-neutral-950 items-center justify-center select-none">
        {/* 设置按钮 */}
        <button
          onClick={onOpenSettings}
          className="absolute top-4 right-4 text-neutral-600 hover:text-neutral-300 transition-colors"
          title="应用设置"
        >
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
        <div className="text-center max-w-md">
          <div className="mb-6">
            <svg
              className="w-20 h-20 mx-auto text-neutral-700"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M20 6h-8l-2-2H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 12H4V8h16v10z" />
            </svg>
          </div>
          <h2 className="text-2xl font-semibold text-neutral-200 mb-3">欢迎使用 DeepSeekX</h2>
          <p className="text-neutral-500 text-sm mb-8 leading-relaxed">
            将本地项目与 AI 助手关联，管理项目文件、自定义指令，让 AI 更好地理解你的代码库。
          </p>
          <button
            onClick={onCreateNew}
            className="inline-flex items-center gap-2 px-6 py-3 rounded-lg bg-blue-600 hover:bg-blue-500 
                       text-white text-sm font-medium transition-colors"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
            </svg>
            创建第一个项目
          </button>
        </div>
      </div>
    );
  }

  // 有项目：列表 + 新建按钮
  return (
    <div className="flex h-screen w-screen bg-neutral-950 items-center justify-center select-none">
      {/* 设置按钮 */}
      <button
        onClick={onOpenSettings}
        className="absolute top-4 right-4 text-neutral-600 hover:text-neutral-300 transition-colors"
        title="应用设置"
      >
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
      </button>
      <div className="w-full max-w-lg px-6">
        <div className="text-center mb-8">
          <h2 className="text-2xl font-semibold text-neutral-200 mb-2">选择项目</h2>
          <p className="text-neutral-500 text-sm">选择一个项目进入工作区</p>
        </div>

        <div className="space-y-2 mb-6">
          {projects.map((p) => (
            <div
              key={p.id}
              onClick={() => onSelect(p)}
              className="group flex items-center gap-3 px-4 py-3 rounded-lg bg-neutral-925 border border-neutral-800
                         hover:border-neutral-600 cursor-pointer transition-colors"
            >
              <svg className="w-5 h-5 text-neutral-500 shrink-0" viewBox="0 0 24 24" fill="currentColor">
                <path d="M20 6h-8l-2-2H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 12H4V8h16v10z" />
              </svg>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-neutral-200 truncate">{p.name}</p>
                <p className="text-xs text-neutral-500 truncate">{p.root_path}</p>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(p.id);
                }}
                className="opacity-0 group-hover:opacity-100 text-neutral-500 hover:text-red-400
                           transition-all p-1 rounded shrink-0"
                title="删除项目"
              >
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </button>
            </div>
          ))}
        </div>

        <button
          onClick={onCreateNew}
          className="w-full py-2.5 rounded-lg bg-neutral-850 border border-neutral-800 hover:border-neutral-600
                     text-neutral-400 hover:text-neutral-200 text-sm transition-colors flex items-center justify-center gap-2"
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
          </svg>
          新建项目
        </button>
      </div>
    </div>
  );
}
