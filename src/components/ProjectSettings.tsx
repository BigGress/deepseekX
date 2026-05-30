import { useState, useEffect } from "react";
import type { FileNode, ProjectRow, SkillInfo } from "../types";
import { listAvailableSkills, listAvailableMcpServers } from "../api";

const RETRIEVAL_SOURCES = [
  {
    name: "workspace_code",
    label: "工作区代码",
    description: "检索当前项目里的源码、配置和脚本文件",
  },
  {
    name: "workspace_docs",
    label: "工作区文档",
    description: "检索 docs、README 和本地说明文档",
  },
  {
    name: "user_knowledge_base",
    label: "用户知识库",
    description: "检索应用设置里配置的外部知识库路径",
  },
  {
    name: "web_search",
    label: "联网搜索",
    description: "允许查询实时网页信息和外部资料",
  },
] as const;

const BROWSER_SKILL_KEYWORDS = ["browser", "agent-browser", "chrome"];
const BROWSER_MCP_KEYWORDS = ["browser", "chrome", "playwright", "puppeteer"];
const DESKTOP_SKILL_KEYWORDS = ["computer-use"];
const DESKTOP_MCP_KEYWORDS = ["computer", "desktop", "macos", "ui"];

interface ProjectSettingsProps {
  open: boolean;
  project: ProjectRow;
  fileNodes: FileNode[];
  onClose: () => void;
  onSave: (
    name: string,
    description: string,
    instructions: string,
    model: string,
    pinnedFiles: string,
    skills: string,
    mcpServers: string,
    retrievalSources: string,
  ) => void;
}

export default function ProjectSettings({
  open,
  project,
  fileNodes,
  onClose,
  onSave,
}: ProjectSettingsProps) {
  const [name, setName] = useState(project.name);
  const [description, setDescription] = useState(project.description);
  const [instructions, setInstructions] = useState(project.instructions);
  const [model, setModel] = useState(project.model);
  const [pinnedFiles, setPinnedFiles] = useState<string[]>(
    project.pinned_files ? project.pinned_files.split(",").filter(Boolean) : [],
  );
  const [availableSkills, setAvailableSkills] = useState<SkillInfo[]>([]);
  const [availableMcp, setAvailableMcp] = useState<string[]>([]);
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(() => {
    try { return new Set(JSON.parse(project.skills || "[]")); } catch { return new Set(); }
  });
  const [selectedMcp, setSelectedMcp] = useState<Set<string>>(() => {
    try { return new Set(JSON.parse(project.mcp_servers || "[]")); } catch { return new Set(); }
  });
  const [selectedRetrievalSources, setSelectedRetrievalSources] = useState<Set<string>>(() => {
    try { return new Set(JSON.parse(project.retrieval_sources || "[]")); } catch { return new Set(); }
  });

  useEffect(() => {
    if (open) {
      setName(project.name);
      setDescription(project.description);
      setInstructions(project.instructions);
      setModel(project.model);
      setPinnedFiles(project.pinned_files ? project.pinned_files.split(",").filter(Boolean) : []);
      try { setSelectedSkills(new Set(JSON.parse(project.skills || "[]"))); } catch { setSelectedSkills(new Set()); }
      try { setSelectedMcp(new Set(JSON.parse(project.mcp_servers || "[]"))); } catch { setSelectedMcp(new Set()); }
      try { setSelectedRetrievalSources(new Set(JSON.parse(project.retrieval_sources || "[]"))); } catch { setSelectedRetrievalSources(new Set()); }
      listAvailableSkills().then(setAvailableSkills).catch(() => {});
      listAvailableMcpServers().then(setAvailableMcp).catch(() => {});
    }
  }, [open, project]);

  if (!open) return null;

  const togglePinned = (path: string) => {
    setPinnedFiles((prev) =>
      prev.includes(path) ? prev.filter((p) => p !== path) : [...prev, path],
    );
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave(
      name.trim() || project.name,
      description.trim(),
      instructions.trim(),
      model,
      pinnedFiles.join(","),
      JSON.stringify([...selectedSkills]),
      JSON.stringify([...selectedMcp]),
      JSON.stringify([...selectedRetrievalSources]),
    );
    onClose();
  };

  const allFiles = flattenFilePaths(fileNodes);
  const knownSkillNames = Array.from(new Set([
    ...availableSkills.map((sk) => sk.name),
    ...selectedSkills,
  ]));
  const knownMcpNames = Array.from(new Set([
    ...availableMcp,
    ...selectedMcp,
  ]));
  const browserSkills = knownSkillNames
    .filter((name) => containsAnyKeyword(name, BROWSER_SKILL_KEYWORDS));
  const browserMcp = knownMcpNames.filter((name) => containsAnyKeyword(name, BROWSER_MCP_KEYWORDS));
  const desktopSkills = knownSkillNames
    .filter((name) => containsAnyKeyword(name, DESKTOP_SKILL_KEYWORDS));
  const desktopMcp = knownMcpNames.filter((name) => containsAnyKeyword(name, DESKTOP_MCP_KEYWORDS));

  const enableCapability = (skillsToEnable: string[], mcpToEnable: string[]) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      skillsToEnable.forEach((name) => next.add(name));
      return next;
    });
    setSelectedMcp((prev) => {
      const next = new Set(prev);
      mcpToEnable.forEach((name) => next.add(name));
      return next;
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div
        className="bg-neutral-925 border border-neutral-800 rounded-xl w-full max-w-lg mx-4 shadow-2xl max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-800 shrink-0">
          <h3 className="text-sm font-medium text-neutral-200">项目设置</h3>
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
        <form onSubmit={handleSubmit} className="px-5 py-4 space-y-4 overflow-y-auto flex-1">
          {/* 名称 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">项目名称</label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors"
            />
          </div>

          {/* 描述 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">项目描述</label>
            <input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="简要描述项目用途"
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors"
            />
          </div>

          {/* 模型 */}
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
            <label className="block text-xs text-neutral-400 mb-1.5">自定义指令</label>
            <textarea
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
              placeholder="例如：你是一个 React 专家，代码风格遵循 Prettier 默认配置..."
              rows={4}
              className="w-full bg-neutral-850 border border-neutral-700 rounded-md px-3 py-2 text-sm text-neutral-200
                         placeholder-neutral-500 outline-none focus:border-neutral-500 transition-colors resize-none"
            />
          </div>

          {/* 锚定文档 */}
          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">
              锚定文档（选中的文件会在每次对话时自动注入上下文）
            </label>
            <div className="bg-neutral-850 border border-neutral-700 rounded-md max-h-40 overflow-y-auto">
              {allFiles.length === 0 ? (
                <p className="px-3 py-3 text-xs text-neutral-600">暂无文件</p>
              ) : (
                allFiles.map((filePath) => (
                  <label
                    key={filePath}
                    className="flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-neutral-800 text-xs text-neutral-400 hover:text-neutral-200 transition-colors"
                  >
                    <input
                      type="checkbox"
                      checked={pinnedFiles.includes(filePath)}
                      onChange={() => togglePinned(filePath)}
                      className="rounded bg-neutral-700 border-neutral-600 text-blue-600 focus:ring-0 cursor-pointer"
                    />
                    <span className="truncate">{filePath}</span>
                  </label>
                ))
              )}
            </div>
          </div>
          {/* Skills */}
          {availableSkills.length > 0 && (
            <div>
              <label className="block text-xs text-neutral-400 mb-1.5">
                Skills（选中后 AI 将获得对应技能）
              </label>
              <div className="bg-neutral-850 border border-neutral-700 rounded-md max-h-32 overflow-y-auto">
                {availableSkills.map((sk) => (
                  <label key={sk.name} className="flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-neutral-800 text-xs text-neutral-400 hover:text-neutral-200 transition-colors">
                    <input type="checkbox" checked={selectedSkills.has(sk.name)}
                      onChange={() => { const s = new Set(selectedSkills); if (s.has(sk.name)) s.delete(sk.name); else s.add(sk.name); setSelectedSkills(s); }}
                      className="rounded bg-neutral-700 border-neutral-600 text-blue-600 focus:ring-0 cursor-pointer" />
                    <span className="font-medium text-neutral-300">{sk.name}</span>
                    {sk.description && <span className="text-neutral-600">— {sk.description}</span>}
                  </label>
                ))}
              </div>
            </div>
          )}

          {(browserSkills.length > 0 || browserMcp.length > 0 || desktopSkills.length > 0 || desktopMcp.length > 0) && (
            <div>
              <label className="block text-xs text-neutral-400 mb-1.5">
                Automation Capabilities（一键启用浏览器/桌面自动化相关能力）
              </label>
              <div className="space-y-2">
                <CapabilityCard
                  title="浏览器自动化"
                  description="适合页面导航、抓取、UI 验证。建议同时启用相关 skill 和 MCP server。"
                  skills={browserSkills}
                  mcpServers={browserMcp}
                  enabledSkills={[...selectedSkills].filter((name) => browserSkills.includes(name))}
                  enabledMcp={[...selectedMcp].filter((name) => browserMcp.includes(name))}
                  onEnable={() => enableCapability(browserSkills, browserMcp)}
                />
                <CapabilityCard
                  title="桌面自动化"
                  description="适合原生应用交互、Computer Use、非工作区文件系统之外的桌面操作。"
                  skills={desktopSkills}
                  mcpServers={desktopMcp}
                  enabledSkills={[...selectedSkills].filter((name) => desktopSkills.includes(name))}
                  enabledMcp={[...selectedMcp].filter((name) => desktopMcp.includes(name))}
                  onEnable={() => enableCapability(desktopSkills, desktopMcp)}
                />
              </div>
            </div>
          )}

          <div>
            <label className="block text-xs text-neutral-400 mb-1.5">
              Retrieval Sources（控制这个项目允许使用哪些检索来源）
            </label>
            <div className="bg-neutral-850 border border-neutral-700 rounded-md overflow-hidden">
              {RETRIEVAL_SOURCES.map((source) => (
                <label
                  key={source.name}
                  className="flex items-start gap-2 px-3 py-2 cursor-pointer hover:bg-neutral-800 text-xs text-neutral-400 hover:text-neutral-200 transition-colors"
                >
                  <input
                    type="checkbox"
                    checked={selectedRetrievalSources.has(source.name)}
                    onChange={() => {
                      const next = new Set(selectedRetrievalSources);
                      if (next.has(source.name)) {
                        if (next.size === 1) return;
                        next.delete(source.name);
                      } else {
                        next.add(source.name);
                      }
                      setSelectedRetrievalSources(next);
                    }}
                    className="mt-0.5 rounded bg-neutral-700 border-neutral-600 text-blue-600 focus:ring-0 cursor-pointer"
                  />
                  <span className="min-w-0">
                    <span className="block font-medium text-neutral-300">{source.label}</span>
                    <span className="block text-neutral-600">{source.description}</span>
                  </span>
                </label>
              ))}
            </div>
            <p className="mt-1.5 text-[11px] text-neutral-600">
              至少保留一个来源。关闭 `联网搜索` 后，Agent 和普通 Chat 都会按本地模式运行。
            </p>
          </div>

          {/* MCP */}
          {availableMcp.length > 0 && (
            <div>
              <label className="block text-xs text-neutral-400 mb-1.5">MCP 服务器</label>
              <div className="bg-neutral-850 border border-neutral-700 rounded-md max-h-32 overflow-y-auto">
                {availableMcp.map((m) => (
                  <label key={m} className="flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-neutral-800 text-xs text-neutral-400 hover:text-neutral-200 transition-colors">
                    <input type="checkbox" checked={selectedMcp.has(m)}
                      onChange={() => { const s = new Set(selectedMcp); if (s.has(m)) s.delete(m); else s.add(m); setSelectedMcp(s); }}
                      className="rounded bg-neutral-700 border-neutral-600 text-blue-600 focus:ring-0 cursor-pointer" />
                    <span className="font-medium text-neutral-300">{m}</span>
                  </label>
                ))}
              </div>
            </div>
          )}
        </form>

        {/* 按钮 */}
        <div className="flex justify-end gap-2 px-5 py-4 border-t border-neutral-800 shrink-0">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-md text-sm text-neutral-400 hover:text-neutral-200 transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleSubmit}
            className="px-5 py-2 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-colors"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}

function containsAnyKeyword(value: string, keywords: string[]) {
  const normalized = value.toLowerCase();
  return keywords.some((keyword) => normalized.includes(keyword));
}

function CapabilityCard({
  title,
  description,
  skills,
  mcpServers,
  enabledSkills,
  enabledMcp,
  onEnable,
}: {
  title: string;
  description: string;
  skills: string[];
  mcpServers: string[];
  enabledSkills: string[];
  enabledMcp: string[];
  onEnable: () => void;
}) {
  const hasAnything = skills.length > 0 || mcpServers.length > 0;
  const fullyEnabled = enabledSkills.length === skills.length && enabledMcp.length === mcpServers.length && hasAnything;

  return (
    <div className="rounded-md border border-neutral-700 bg-neutral-850 px-3 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-sm font-medium text-neutral-200">{title}</p>
          <p className="mt-1 text-xs text-neutral-500">{description}</p>
        </div>
        <button
          type="button"
          onClick={onEnable}
          disabled={!hasAnything || fullyEnabled}
          className="shrink-0 rounded-md border border-neutral-700 bg-neutral-900 px-3 py-1.5 text-xs text-neutral-200 transition-colors hover:bg-neutral-800 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {fullyEnabled ? "已启用" : "一键启用"}
        </button>
      </div>
      <div className="mt-3 space-y-1 text-xs">
        <p className="text-neutral-400">
          Skills: {skills.length > 0 ? skills.join(", ") : "未检测到"}
        </p>
        <p className="text-neutral-400">
          MCP Servers: {mcpServers.length > 0 ? mcpServers.join(", ") : "未检测到"}
        </p>
        {(enabledSkills.length > 0 || enabledMcp.length > 0) && (
          <p className="text-emerald-300">
            当前已启用: {[...enabledSkills, ...enabledMcp].join(", ")}
          </p>
        )}
      </div>
    </div>
  );
}

function flattenFilePaths(nodes: FileNode[]): string[] {
  const paths: string[] = [];
  function walk(list: FileNode[]) {
    for (const n of list) {
      paths.push(n.path);
      if (n.children && n.children.length > 0) {
        walk(n.children);
      }
    }
  }
  walk(nodes);
  return paths;
}
