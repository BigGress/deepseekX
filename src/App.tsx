import { useState, useCallback, useEffect, useMemo } from "react";
import Sidebar from "./components/Sidebar";
import ChatPanel from "./components/ChatPanel";
import ProjectSelector from "./components/ProjectSelector";
import CreateProjectDialog from "./components/CreateProjectDialog";
import ProjectSettings from "./components/ProjectSettings";
import SettingsDialog from "./components/SettingsDialog";
import type {
  AgentFollowUpAction,
  ComposeMode,
  Message,
  Conversation,
  ProjectRow,
  FileNode,
  ContentBlock,
  LlmRequestLogEntry,
  RequestAttachment,
  Turn,
  PreviewTarget,
} from "./types";
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
  readLlmLogs,
  runAgentTask,
  sendMessageViaApi,
} from "./api";
import { useFilePreview } from "./hooks/useFilePreview";
import FilePreviewPanel from "./components/FilePreviewPanel";
import FocusedPreviewOverlay from "./components/FocusedPreviewOverlay";

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

function buildAgentFollowUpPrompt(turn: Turn, action: AgentFollowUpAction): string {
  const steps = turn.agent_steps ?? [];
  const pendingStep = [...steps]
    .reverse()
    .find((step) => step.requires_confirmation || step.action_name === "ask_user");
  const approvalContext =
    pendingStep?.result_summary ||
    pendingStep?.summary ||
    pendingStep?.reason ||
    turn.final_response ||
    "上一步存在需要确认的动作。";

  switch (action) {
    case "approve":
      return [
        "用户已经批准上一个 Agent 任务里需要确认的动作。",
        "请继续执行，不要重复询问同一确认事项。",
        "",
        "原始目标：",
        turn.user_input,
        "",
        "需要确认的内容：",
        approvalContext,
      ].join("\n");
    case "reject":
      return [
        "用户拒绝执行上一个 Agent 任务里需要确认的动作。",
        "请停止该高风险动作，并基于原始目标给出更安全的替代方案或当前可交付结果。",
        "",
        "原始目标：",
        turn.user_input,
        "",
        "被拒绝的内容：",
        approvalContext,
      ].join("\n");
    case "retry":
      return [
        "请重新尝试上一个 Agent 任务。",
        "如果上一步因为权限判断或暂时性失败而中断，请结合已有上下文重新规划。",
        "",
        "原始目标：",
        turn.user_input,
      ].join("\n");
  }
}

function App() {
  // 项目状态
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [selectedProject, setSelectedProject] = useState<ProjectRow | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [appSettingsOpen, setAppSettingsOpen] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [knowledgeBasePaths, setKnowledgeBasePaths] = useState("");
  const [commandAllowlist, setCommandAllowlist] = useState("");
  const [debugLlmResponses, setDebugLlmResponses] = useState(false);
  const [llmDebugEntries, setLlmDebugEntries] = useState<LlmRequestLogEntry[]>([]);

  // 项目内状态
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeConvId, setActiveConvId] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [fileNodes, setFileNodes] = useState<FileNode[]>([]);

  const {
    state: previewState,
    openPreview,
    switchMode: switchPreviewMode,
    closePreview,
    openFocused,
    closeFocused,
  } = useFilePreview();

  const handlePreviewFile = useCallback((path: string) => {
    const target: PreviewTarget = { path, source: "file_tree" };
    openPreview(target, selectedProject?.root_path ?? "");
  }, [openPreview, selectedProject]);

  const refreshLlmDebugEntries = useCallback(async () => {
    try {
      const entries = await readLlmLogs(20);
      setLlmDebugEntries(entries);
    } catch {
      setLlmDebugEntries([]);
    }
  }, []);

  // 初始化：加载项目列表 + 应用设置
  useEffect(() => {
    listProjects()
      .then(setProjects)
      .catch(() => setProjects([]));
    getSetting("api_key").then((val) => {
      if (val) setApiKey(val);
    }).catch(() => {});
    getSetting("knowledge_base_paths").then((val) => {
      if (val) setKnowledgeBasePaths(val);
    }).catch(() => {});
    getSetting("command_allowlist").then((val) => {
      if (val) setCommandAllowlist(val);
    }).catch(() => {});
    getSetting("debug_llm_responses").then((val) => {
      const enabled = val === "true";
      setDebugLlmResponses(enabled);
      if (enabled) {
        void refreshLlmDebugEntries();
      }
    }).catch(() => {});
  }, [refreshLlmDebugEntries]);

  useEffect(() => {
    if (debugLlmResponses) {
      void refreshLlmDebugEntries();
    }
  }, [debugLlmResponses, refreshLlmDebugEntries]);

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
      retrievalSources: string,
    ) => {
      if (!selectedProject) return;
      try {
        await updateProject(selectedProject.id, name, description, instructions, model, pinnedFiles, skills, mcpServers, retrievalSources);
        // 更新本地状态
        setSelectedProject((prev) =>
          prev
            ? {
                ...prev,
                name,
                description,
                instructions,
                model,
                pinned_files: pinnedFiles,
                skills,
                mcp_servers: mcpServers,
                retrieval_sources: retrievalSources,
              }
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
    async (
      key: string,
      kbPaths: string,
      commandEntries: string,
      debugEnabled: boolean,
    ) => {
      try {
        await setSetting("api_key", key);
        await setSetting("knowledge_base_paths", kbPaths);
        await setSetting("command_allowlist", commandEntries);
        await setSetting("debug_llm_responses", debugEnabled ? "true" : "false");
        setApiKey(key);
        setKnowledgeBasePaths(kbPaths);
        setCommandAllowlist(commandEntries);
        setDebugLlmResponses(debugEnabled);
        if (debugEnabled) {
          await refreshLlmDebugEntries();
        } else {
          setLlmDebugEntries([]);
        }
      } catch (e) {
        console.error("保存 API key 失败:", e);
      }
    },
    [refreshLlmDebugEntries],
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
      const optimisticIntent = parseOptimisticUserIntent(content);
      const pendingTurn: Turn = {
        id: `${activeConvId}-pending`,
        user_input: optimisticIntent.user_input,
        request_attachments: optimisticIntent.request_attachments,
        thinking_steps: [],
        final_response: null, // null → TurnItem 显示加载动画
        agent_steps: null,
        agent_goal_status: null,
        duration_ms: null,
        error_stage: null,
        retry_count: null,
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
                selectedProject.retrieval_sources || undefined,
              )).final_response
            : await sendMessageViaApi(
                activeConvId,
                content,
                activeConversation.title || content.slice(0, 30),
                selectedProject.root_path,
                selectedProject.instructions || undefined,
                selectedProject.skills || undefined,
                selectedProject.mcp_servers || undefined,
                selectedProject.retrieval_sources || undefined,
              );

        if (debugLlmResponses) {
          void refreshLlmDebugEntries();
        }

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
        if (debugLlmResponses) {
          void refreshLlmDebugEntries();
        }
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

  const handleAgentFollowUp = useCallback(
    (turn: Turn, action: AgentFollowUpAction) => {
      const prompt = buildAgentFollowUpPrompt(turn, action);
      void handleSend(prompt, "agent");
    },
    [handleSend],
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
          knowledgeBasePaths={knowledgeBasePaths}
          commandAllowlist={commandAllowlist}
          debugLlmResponses={debugLlmResponses}
          onClose={() => setAppSettingsOpen(false)}
          onSave={handleSaveAppSettings}
        />
      </>
    );
  }

  // 已选择项目：显示工作区
  return (
    <div className="flex h-screen w-screen bg-neutral-950 overflow-hidden">
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
        onPreviewFile={handlePreviewFile}
        previewPath={previewState.target?.path}
      />
      <div className="flex-1 flex overflow-hidden">
        <ChatPanel
          conversation={activeConversation ?? null}
          isLoading={isLoading}
          onSend={handleSend}
          onAgentFollowUp={handleAgentFollowUp}
          fileNodes={fileNodes}
          skillNames={skillNamesFromProject}
          mcpNames={mcpNamesFromProject}
          debugLlmResponsesEnabled={debugLlmResponses}
          llmDebugEntries={llmDebugEntries}
          onPreviewFile={handlePreviewFile}
        />
        {previewState.isOpen && (
          <FilePreviewPanel
            state={previewState}
            workspaceRoot={selectedProject?.root_path ?? ""}
            onClose={closePreview}
            onSwitchMode={switchPreviewMode}
            onOpenFocused={openFocused}
          />
        )}
      </div>
      <ProjectSettings
        open={settingsOpen}
        project={selectedProject}
        fileNodes={fileNodes}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSaveSettings}
      />
      {previewState.focusedView && previewState.descriptor && (
        <FocusedPreviewOverlay
          descriptor={previewState.descriptor}
          selectedMode={previewState.selectedMode ?? previewState.descriptor.default_mode}
          workspaceRoot={selectedProject?.root_path ?? ""}
          onClose={closeFocused}
          onSwitchMode={switchPreviewMode}
        />
      )}
    </div>
  );
}

export default App;

function parseOptimisticUserIntent(content: string): {
  user_input: string;
  request_attachments: RequestAttachment[] | null;
} {
  const attachments: RequestAttachment[] = [];
  const lines: string[] = [];

  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("本次请求显式附加技能:")) {
      for (const raw of trimmed.replace("本次请求显式附加技能:", "").split(",")) {
        const name = raw.trim();
        if (name) {
          attachments.push({ kind: "skill", name });
        }
      }
      continue;
    }
    if (trimmed.startsWith("本次请求显式附加 MCP:")) {
      for (const raw of trimmed.replace("本次请求显式附加 MCP:", "").split(",")) {
        const name = raw.trim();
        if (name) {
          attachments.push({ kind: "mcp", name });
        }
      }
      continue;
    }
    lines.push(line);
  }

  return {
    user_input: lines.join("\n").trim(),
    request_attachments: attachments.length > 0 ? attachments : null,
  };
}
