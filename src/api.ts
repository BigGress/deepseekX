import { invoke } from "@tauri-apps/api/core";
import type {
  AgentRunResponse,
  ProjectRow,
  ConversationRow,
  MessageRow,
  FileNode,
  SessionSummary,
  SessionMessages,
  TurnBasedSession,
  SkillInfo,
  LlmRequestLogEntry,
  PreviewDescriptor,
  PreviewMode,
} from "./types";

// ---------- Project ----------

export async function createProject(
  name: string,
  description: string,
  rootPath: string,
  instructions: string,
  model: string,
  pinnedFiles?: string,
  retrievalSources?: string,
): Promise<ProjectRow> {
  return invoke("create_project", {
    name,
    description,
    rootPath,
    instructions,
    model,
    pinnedFiles,
    retrievalSources,
  });
}

export async function listProjects(): Promise<ProjectRow[]> {
  return invoke("list_projects");
}

export async function getProject(id: string): Promise<ProjectRow> {
  return invoke("get_project", { id });
}

export async function updateProject(
  id: string,
  name: string,
  description: string,
  instructions: string,
  model: string,
  pinnedFiles?: string,
  skills?: string,
  mcpServers?: string,
  retrievalSources?: string,
): Promise<void> {
  return invoke("update_project", { id, name, description, instructions, model, pinnedFiles, skills, mcpServers, retrievalSources });
}

export async function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
}

// ---------- Conversation ----------

export async function createConversation(
  projectId: string,
  title: string,
): Promise<ConversationRow> {
  return invoke("create_conversation", { projectId, title });
}

export async function listConversations(projectId: string): Promise<ConversationRow[]> {
  return invoke("list_conversations", { projectId });
}

export async function deleteConversation(id: string): Promise<void> {
  return invoke("delete_conversation", { id });
}

// ---------- Messages ----------

export async function saveMessage(
  id: string,
  conversationId: string,
  role: string,
  content: string,
  timestamp: number,
): Promise<void> {
  return invoke("save_message", {
    id,
    conversationId,
    role,
    content,
    timestamp,
  });
}

export async function listMessages(conversationId: string): Promise<MessageRow[]> {
  return invoke("list_messages", { conversationId });
}

// ---------- File ----------

export async function listFiles(rootPath: string, depth?: number): Promise<FileNode[]> {
  return invoke("list_files", { rootPath, depth });
}

export async function readFileContent(path: string): Promise<string> {
  return invoke("read_file_content", { path });
}

// ---------- Settings ----------

export async function getSetting(key: string): Promise<string | null> {
  return invoke("get_setting", { key });
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke("set_setting", { key, value });
}

// ---------- Chat ----------

export async function sendMessage(conversationId: string, prompt: string): Promise<string> {
  return invoke("send_message", { conversationId, prompt });
}

// ---------- TUI Session ----------

export async function listTuiSessions(workspace?: string): Promise<SessionSummary[]> {
  return invoke("list_tui_sessions", { workspace: workspace ?? null });
}

export async function readTuiSession(sessionId: string): Promise<SessionMessages> {
  return invoke("read_tui_session", { sessionId });
}

export async function deleteTuiSession(sessionId: string): Promise<void> {
  return invoke("delete_tui_session", { sessionId });
}

export async function readTuiSessionTurns(sessionId: string): Promise<TurnBasedSession> {
  return invoke("read_tui_session_turns", { sessionId });
}

export async function sendMessageViaApi(
  sessionId: string,
  content: string,
  title: string,
  workspaceRoot: string,
  instructions?: string,
  skills?: string,
  mcpServers?: string,
  retrievalSources?: string,
): Promise<string> {
  return invoke("send_message_via_api", {
    sessionId,
    content,
    title,
    workspaceRoot,
    instructions: instructions ?? null,
    skills: skills ?? null,
    mcpServers: mcpServers ?? null,
    retrievalSources: retrievalSources ?? null,
  });
}

export async function runAgentTask(
  sessionId: string,
  content: string,
  title: string,
  workspaceRoot: string,
  instructions?: string,
  skills?: string,
  mcpServers?: string,
  retrievalSources?: string,
): Promise<AgentRunResponse> {
  return invoke("run_agent_task", {
    sessionId,
    content,
    title,
    workspaceRoot,
    instructions: instructions ?? null,
    skills: skills ?? null,
    mcpServers: mcpServers ?? null,
    retrievalSources: retrievalSources ?? null,
  });
}

// ---------- Skills & MCP ----------

export async function listAvailableSkills(): Promise<SkillInfo[]> {
  return invoke("list_available_skills");
}

export async function listAvailableMcpServers(): Promise<string[]> {
  return invoke("list_available_mcp_servers");
}

export async function readLlmLogs(limit = 20): Promise<LlmRequestLogEntry[]> {
  return invoke("read_llm_logs", { limit });
}

// ---------- Preview ----------

export async function describeFilePreview(
  path: string,
  workspaceRoot: string,
): Promise<PreviewDescriptor> {
  return invoke("describe_file_preview", { path, workspaceRoot });
}

export async function resolveFilePreview(
  path: string,
  workspaceRoot: string,
  mode: PreviewMode,
): Promise<PreviewDescriptor> {
  return invoke("resolve_file_preview", { path, workspaceRoot, mode });
}
