import { useState } from "react";
import ProjectSettings from "../components/ProjectSettings";
import type { FileNode, ProjectRow } from "../types";

const MOCK_PROJECT: ProjectRow = {
  id: "preview-project",
  name: "deepseekx",
  description: "Project settings preview",
  root_path: "/Users/Gress/code/ai/deepseekX",
  instructions: "优先做工程化改动，并给出可验证结果。",
  model: "deepseek",
  pinned_files: "src/App.tsx,src/components/ProjectSettings.tsx",
  skills: JSON.stringify(["browser", "computer-use", "gmail"]),
  mcp_servers: JSON.stringify(["browser", "computer-use", "fetch"]),
  retrieval_sources: JSON.stringify([
    "workspace_code",
    "workspace_docs",
    "user_knowledge_base",
    "web_search",
  ]),
  created_at: Date.now(),
  updated_at: Date.now(),
};

const MOCK_FILES: FileNode[] = [
  { name: "src/App.tsx", path: "src/App.tsx", is_directory: false, children: null, size: 1200 },
  {
    name: "src/components/ProjectSettings.tsx",
    path: "src/components/ProjectSettings.tsx",
    is_directory: false,
    children: null,
    size: 1800,
  },
  { name: "docs/README.md", path: "docs/README.md", is_directory: false, children: null, size: 800 },
];

export default function ProjectSettingsPreview() {
  const [lastSaved, setLastSaved] = useState<string>("(not saved yet)");

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 p-8">
      <div className="max-w-5xl mx-auto space-y-4">
        <header className="space-y-2">
          <h1 className="text-xl font-semibold">ProjectSettings Preview</h1>
          <p className="text-sm text-neutral-400">
            这个调试页用于验证项目级 retrieval source 和浏览器/桌面自动化能力入口是否可见且可提交。
          </p>
        </header>

        <div className="rounded-xl border border-neutral-800 bg-neutral-925 p-4 text-xs text-neutral-400">
          <div className="font-medium text-neutral-200">最近一次保存 payload</div>
          <pre className="mt-3 whitespace-pre-wrap">{lastSaved}</pre>
        </div>
      </div>

      <ProjectSettings
        open
        project={MOCK_PROJECT}
        fileNodes={MOCK_FILES}
        onClose={() => {}}
        onSave={(name, description, instructions, model, pinnedFiles, skills, mcpServers, retrievalSources) => {
          setLastSaved(
            JSON.stringify(
              {
                name,
                description,
                instructions,
                model,
                pinnedFiles,
                skills,
                mcpServers,
                retrievalSources,
              },
              null,
              2,
            ),
          );
        }}
      />
    </div>
  );
}
