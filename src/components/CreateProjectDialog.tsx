import { useState, useRef, useEffect } from "react";

interface CreateProjectDialogProps {
  open: boolean;
  onClose: () => void;
  onCreate: (
    name: string,
    description: string,
    rootPath: string,
    instructions: string,
    model: string,
  ) => void;
}

export default function CreateProjectDialog({
  open,
  onClose,
  onCreate,
}: CreateProjectDialogProps) {
  const [name, setName] = useState("");
  const [rootPath, setRootPath] = useState("");
  const [description, setDescription] = useState("");
  const [instructions, setInstructions] = useState("");
  const [model, setModel] = useState("deepseek");
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setName("");
      setRootPath("");
      setDescription("");
      setInstructions("");
      setModel("deepseek");
      setTimeout(() => nameRef.current?.focus(), 50);
    }
  }, [open]);

  if (!open) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !rootPath.trim()) return;
    onCreate(name.trim(), description.trim(), rootPath.trim(), instructions.trim(), model);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div
        className="bg-neutral-925 border border-neutral-800 rounded-xl w-full max-w-lg mx-4 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-800">
          <h3 className="text-sm font-medium text-neutral-200">新建项目</h3>
          <button
            onClick={onClose}
            className="text-neutral-500 hover:text-neutral-300 transition-colors"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* 表单 */}
        <form onSubmit={handleSubmit} className="px-5 py-4 space-y-4">
          {/* 项目名称 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">项目名称</label>
            <input
              ref={nameRef}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：my-project"
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors"
            />
          </div>

          {/* 项目目录 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">项目目录</label>
            <div className="flex gap-2">
              <input
                value={rootPath}
                onChange={(e) => setRootPath(e.target.value)}
                placeholder="/path/to/project"
                className="flex-1 bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                           placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors"
              />
              <button
                type="button"
                onClick={async () => {
                  try {
                    const { open } = await import("@tauri-apps/plugin-dialog");
                    const dir = await open({ directory: true, title: "选择项目目录" });
                    if (dir) setRootPath(dir as string);
                  } catch {
                    // 回退：手动输入路径
                  }
                }}
                className="px-3 py-2 rounded-md bg-neutral-850 border border-neutral-700 text-neutral-400 
                           hover:text-neutral-200 hover:border-neutral-500 text-sm transition-colors shrink-0"
              >
                浏览
              </button>
            </div>
          </div>

          {/* 描述 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">项目描述（可选）</label>
            <input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="简要描述项目用途"
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors"
            />
          </div>

          {/* 模型选择 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">默认模型</label>
            <select
              value={model}
              onChange={(e) => setModel(e.target.value)}
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         outline-none focus:border-neutral-500 transition-colors appearance-none cursor-pointer"
            >
              <option value="deepseek">DeepSeek (默认)</option>
              <option value="deepseek-v4-pro">DeepSeek V4 Pro</option>
              <option value="deepseek-v4-flash">DeepSeek V4 Flash</option>
              <option value="deepseek-coder">DeepSeek Coder</option>
            </select>
          </div>

          {/* 自定义指令 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">自定义指令（可选）</label>
            <textarea
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
              placeholder="例如：你是一个 React 专家，代码风格遵循 Prettier 默认配置..."
              rows={3}
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors resize-none"
            />
          </div>

          {/* 按钮 */}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-md text-sm text-neutral-400 hover:text-neutral-200 transition-colors"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={!name.trim() || !rootPath.trim()}
              className="px-5 py-2 rounded-md bg-blue-600 hover:bg-blue-500 disabled:bg-neutral-700 disabled:cursor-not-allowed
                         text-white text-sm font-medium transition-colors"
            >
              创建项目
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
