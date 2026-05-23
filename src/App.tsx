import { useState, useCallback, useEffect, useMemo } from "react";
import Sidebar from "./components/Sidebar";
import ChatPanel from "./components/ChatPanel";
import ProjectSelector from "./components/ProjectSelector";
import CreateProjectDialog from "./components/CreateProjectDialog";
import ProjectSettings from "./components/ProjectSettings";
import SettingsDialog from "./components/SettingsDialog";
import type { ComposeMode, Message, Conversation, ProjectRow, FileNode, ContentBlock, Turn } from "./types";
import {
  listProjects,
  createProject,
  updateProject,
  deleteProject,
  listFiles,
  getSetting,
  setSetting,
  listTuiSessions,
  readTuiSession,
  deleteTuiSession,
  readTuiSessionTurns,
  runAgentTask,
  sendMessageViaApi,
} from "./api";

// 从 ContentBlock 数组中提取纯文本
function extractText(content: ContentBlock[] | string): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return String(content);
  return content
    .filter((b) => b.type === "text" && b.text)
    .map((b) => b.text!)
    .join("\n");
}

function buildSessionPathHint(sessionId: string): string {
  return `~/.deepseek/sessions/${sessionId}.json`;
}

function App() {
  // 项目状态
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [selectedProject, setSelectedProject] = useState<ProjectRow | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [appSettingsOpen, setAppSettingsOpen] = useState(false);
  const [apiKey, setApiKey] = useState("");

  // 项目内状态
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeConvId, setActiveConvId] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [fileNodes, setFileNodes] = useState<FileNode[]>([]);

  // 初始化：加载项目列表 + API key
  useEffect(() => {
    listProjects()
      .then(setProjects)
      .catch(() => setProjects([]));
    getSetting("api_key").then((val) => {
      if (val) setApiKey(val);
    }).catch(() => {});
  }, []);

  const activeConversation = conversations.find((c) => c.id === activeConvId);

  // 从项目中解析 skills / MCP 名称列表
  const skillNamesFromProject: string[] = useMemo(() => {
    try { return JSON.parse(selectedProject?.skills || "[]"); } catch { return []; }
  }, [selectedProject?.skills]);
  const mcpNamesFromProject: string[] = useMemo(() => {
    try { return JSON.parse(selectedProject?.mcp_servers || "[]"); } catch { return []; }
  }, [selectedProject?.mcp_servers]);

  // 切换对话时：延迟从 TUI session 加载消息
  useEffect(() => {
    if (!activeConvId) return;
    const conv = conversations.find((c) => c.id === activeConvId);
    if (conv && conv.messages.length === 0 && !conv.turns) {
      // 优先使用 Turn 分组加载
      readTuiSessionTurns(activeConvId)
        .then((data) => {
          const turns: Turn[] = data.turns;
          // 从 turns 中提取 messages（兼容旧渲染路径）
          const messages: Message[] = turns.map((t) => ({
            id: `${t.id}-user`,
            role: "user" as const,
            content: t.user_input,
            timestamp: Date.now(),
          }));
          setConversations((prev) =>
            prev.map((c) =>
              c.id === activeConvId ? { ...c, messages, turns } : c,
            ),
          );
        })
        .catch(() => {
          // 回退到旧方式加载
          readTuiSession(activeConvId)
            .then((data) => {
              const messages: Message[] = data.messages.map((m, i) => ({
                id: `${activeConvId}-${i}`,
                role: (m.role as "user" | "assistant" | "system") || "assistant",
                content: extractText(m.content),
                timestamp: Date.now() - (data.messages.length - i) * 1000,
              }));
              setConversations((prev) =>
                prev.map((c) =>
                  c.id === activeConvId ? { ...c, messages } : c,
                ),
              );
            })
            .catch(() => {
              // 新对话尚未有 session 文件，忽略
            });
        });
    }
  }, [activeConvId]);

  // 进入项目
  const handleSelectProject = useCallback(async (project: ProjectRow) => {
    setSelectedProject(project);

    // 加载文件树
    listFiles(project.root_path, 2)
      .then(setFileNodes)
      .catch(() => setFileNodes([]));

    // 从 TUI sessions 加载已有对话（按 workspace 过滤）
    try {
      const sessions = await listTuiSessions(project.root_path);
      if (sessions.length > 0) {
        const convs: Conversation[] = sessions.map((s) => ({
          id: s.id,
          projectId: project.id,
          title: s.title,
          messages: [],
          sessionPath: s.session_path,
          createdAt: new Date(s.updated_at).getTime(),
        }));
        setConversations(convs);
        setActiveConvId(convs[0].id);
        return;
      }
    } catch {
      // 加载失败则创建默认对话
    }

    // 没有已有对话：创建默认对话（新 sessionId）
    const sessionId = crypto.randomUUID();
    const defaultConv: Conversation = {
      id: sessionId,
      projectId: project.id,
      title: "新对话",
      messages: [],
      sessionPath: buildSessionPathHint(sessionId),
      createdAt: Date.now(),
    };
    setConversations([defaultConv]);
    setActiveConvId(defaultConv.id);
  }, []);

  // 返回项目列表
  const handleBackToProjects = useCallback(() => {
    setSelectedProject(null);
    setConversations([]);
    setActiveConvId("");
    setFileNodes([]);
    // 刷新项目列表
    listProjects()
      .then(setProjects)
      .catch(() => {});
  }, []);

  // 创建项目
  const handleCreateProject = useCallback(
    async (
      name: string,
      description: string,
      rootPath: string,
      instructions: string,
      model: string,
    ) => {
      try {
        const project = await createProject(name, description, rootPath, instructions, model);
        setProjects((prev) => [project, ...prev]);
        handleSelectProject(project);
      } catch (e) {
        // TODO: 显示错误提示
        console.error("创建项目失败:", e);
      }
    },
    [handleSelectProject],
  );

  // 删除项目
  const handleDeleteProject = useCallback(async (id: string) => {
    try {
      await deleteProject(id);
      setProjects((prev) => prev.filter((p) => p.id !== id));
    } catch (e) {
      console.error("删除项目失败:", e);
    }
  }, []);

  // 保存项目设置
  const handleSaveSettings = useCallback(
    async (
      name: string,
      description: string,
      instructions: string,
      model: string,
      pinnedFiles: string,
      skills: string,
      mcpServers: string,
    ) => {
      if (!selectedProject) return;
      try {
        await updateProject(selectedProject.id, name, description, instructions, model, pinnedFiles, skills, mcpServers);
        // 更新本地状态
        setSelectedProject((prev) =>
          prev
            ? { ...prev, name, description, instructions, model, pinned_files: pinnedFiles, skills, mcp_servers: mcpServers }
            : null,
        );
        // 刷新项目列表
        listProjects().then(setProjects).catch(() => {});
      } catch (e) {
        console.error("保存设置失败:", e);
      }
    },
    [selectedProject],
  );

  // 保存应用设置
  const handleSaveAppSettings = useCallback(
    async (key: string) => {
      try {
        await setSetting("api_key", key);
        setApiKey(key);
      } catch (e) {
        console.error("保存 API key 失败:", e);
      }
    },
    [],
  );

  // 发送消息
  const handleSend = useCallback(
    async (content: string, mode: ComposeMode) => {
      if (!content.trim() || !activeConversation || !selectedProject) return;

      const userMessage: Message = {
        id: crypto.randomUUID(),
        role: "user",
        content,
        timestamp: Date.now(),
      };

      // 乐观更新：追加 pending Turn（保留历史 + 显示加载中）
      const pendingTurn: Turn = {
        id: `${activeConvId}-pending`,
        user_input: content,
        thinking_steps: [],
        final_response: null, // null → TurnItem 显示加载动画
      };

      setConversations((prev) =>
        prev.map((conv) =>
          conv.id === activeConvId
            ? {
                ...conv,
                messages: [...conv.messages, userMessage],
                turns: [...(conv.turns || []), pendingTurn],
                title:
                  conv.messages.length === 0
                    ? content.slice(0, 30) + (content.length > 30 ? "..." : "")
                    : conv.title,
              }
            : conv,
        ),
      );

      setIsLoading(true);

      try {
        const responseText =
          mode === "agent"
            ? (await runAgentTask(
                activeConvId,
                content,
                activeConversation.title || content.slice(0, 30),
                selectedProject.root_path,
                selectedProject.instructions || undefined,
                selectedProject.skills || undefined,
                selectedProject.mcp_servers || undefined,
              )).final_response
            : await sendMessageViaApi(
                activeConvId,
                content,
                activeConversation.title || content.slice(0, 30),
                selectedProject.root_path,
                selectedProject.instructions || undefined,
                selectedProject.skills || undefined,
                selectedProject.mcp_servers || undefined,
              );

        const assistantMessage: Message = {
          id: crypto.randomUUID(),
          role: "assistant",
          content: responseText,
          timestamp: Date.now(),
        };

        setConversations((prev) =>
          prev.map((conv) =>
            conv.id === activeConvId
              ? { ...conv, messages: [...conv.messages, assistantMessage] }
              : conv,
          ),
        );

        // 发送成功后重新加载 turns（后端已追加到 session 文件）
        readTuiSessionTurns(activeConvId)
          .then((data) => {
            setConversations((prev) =>
              prev.map((conv) =>
                conv.id === activeConvId
                  ? { ...conv, turns: data.turns }
                  : conv,
              ),
            );
          })
          .catch(() => {
            // 加载失败：清除 turns 回退到 messages 渲染
            setConversations((prev) =>
              prev.map((conv) =>
                conv.id === activeConvId
                  ? { ...conv, turns: undefined }
                  : conv,
              ),
            );
          });
      } catch (error) {
        const errorMessage: Message = {
          id: crypto.randomUUID(),
          role: "assistant",
          content: `错误: ${error}`,
          timestamp: Date.now(),
        };

        setConversations((prev) =>
          prev.map((conv) =>
            conv.id === activeConvId
              ? { ...conv, messages: [...conv.messages, errorMessage] }
              : conv,
          ),
        );
      } finally {
        setIsLoading(false);
      }
    },
    [activeConvId, activeConversation, selectedProject],
  );

  // 新建对话
  const handleNewConversation = useCallback(() => {
    if (!selectedProject) return;
    const sessionId = crypto.randomUUID();
    const newConv: Conversation = {
      id: sessionId,
      projectId: selectedProject.id,
      title: "新对话",
      messages: [],
      sessionPath: buildSessionPathHint(sessionId),
      createdAt: Date.now(),
    };
    setConversations((prev) => [newConv, ...prev]);
    setActiveConvId(newConv.id);
  }, [selectedProject]);

  // 删除对话，并同步删除磁盘上的 session 文件
  const handleDeleteConversation = useCallback(
    async (id: string) => {
      try {
        await deleteTuiSession(id);
        setConversations((prev) => {
          const filtered = prev.filter((c) => c.id !== id);
          if (activeConvId === id) {
            setActiveConvId(filtered[0]?.id ?? "");
          }
          return filtered;
        });
      } catch (error) {
        console.error("删除对话失败:", error);
      }
    },
    [activeConvId],
  );

  // 点击文件
  const handleSelectFile = useCallback((path: string) => {
    // TODO: Phase 3 - 将 @文件路径 插入输入框
    console.log("选中文件:", path);
  }, []);

  // ---------- 渲染 ----------

  // 未选择项目：显示项目选择器
  if (!selectedProject) {
    return (
      <>
        <ProjectSelector
          projects={projects}
          onSelect={handleSelectProject}
          onCreateNew={() => setDialogOpen(true)}
          onDelete={handleDeleteProject}
          onOpenSettings={() => setAppSettingsOpen(true)}
        />
        <CreateProjectDialog
          open={dialogOpen}
          onClose={() => setDialogOpen(false)}
          onCreate={handleCreateProject}
        />
        <SettingsDialog
          open={appSettingsOpen}
          apiKey={apiKey}
          onClose={() => setAppSettingsOpen(false)}
          onSave={handleSaveAppSettings}
        />
      </>
    );
  }

  // 已选择项目：显示工作区
  return (
    <div className="flex h-screen w-screen bg-neutral-950">
      <Sidebar
        projectName={selectedProject.name}
        onBackToProjects={handleBackToProjects}
        conversations={conversations}
        activeId={activeConvId}
        onSelect={setActiveConvId}
        onNew={handleNewConversation}
        onDelete={handleDeleteConversation}
        fileNodes={fileNodes}
        onSelectFile={handleSelectFile}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <ChatPanel
        conversation={activeConversation ?? null}
        isLoading={isLoading}
        onSend={handleSend}
        fileNodes={fileNodes}
        skillNames={skillNamesFromProject}
        mcpNames={mcpNamesFromProject}
      />
      <ProjectSettings
        open={settingsOpen}
        project={selectedProject}
        fileNodes={fileNodes}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSaveSettings}
      />
    </div>
  );
}

export default App;
