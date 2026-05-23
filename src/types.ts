// ============ 后端行类型（与 Rust 对应）============

export interface ProjectRow {
  id: string;
  name: string;
  description: string;
  root_path: string;
  instructions: string;
  model: string;
  pinned_files: string;
  skills: string;
  mcp_servers: string;
  created_at: number;
  updated_at: number;
}

export interface ConversationRow {
  id: string;
  project_id: string;
  title: string;
  created_at: number;
}

export interface MessageRow {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  timestamp: number;
}

export interface FileNode {
  name: string;
  path: string;
  is_directory: boolean;
  children: FileNode[] | null;
  size: number | null;
}

// ============ 前端应用类型 ============

export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
}

export interface Conversation {
  id: string;
  projectId: string;
  title: string;
  messages: Message[];
  turns?: Turn[];
  sessionPath?: string;
  createdAt: number;
}

export type ComposeMode = "chat" | "agent";

export interface Project {
  id: string;
  name: string;
  description: string;
  rootPath: string;
  instructions: string;
  model: string;
  pinnedFiles: string;
  createdAt: number;
  updatedAt: number;
}

// ============ TUI Session 类型 ============

export interface SessionSummary {
  id: string;
  title: string;
  message_count: number;
  workspace: string;
  model: string;
  session_path: string;
  updated_at: string;
}

export interface SessionMessages {
  id: string;
  title: string;
  messages: SessionMessage[];
  system_prompt: string | null;
}

export interface SessionMessage {
  role: string;
  content: ContentBlock[] | string;
}

export interface ContentBlock {
  type: string;
  text?: string;
  thinking?: string;
  // tool_use / tool_result 暂时不处理
}

// ============ Turn 分组类型 ============

export interface TurnBasedSession {
  id: string;
  title: string;
  turns: Turn[];
  system_prompt: string | null;
}

export interface Turn {
  id: string;
  user_input: string;
  thinking_steps: ThinkingStep[];
  final_response: string | null;
  agent_goal_status?: string | null;
  agent_steps?: AgentStep[] | null;
}

export interface AgentStep {
  action_name: string;
  reason?: string | null;
  input_summary?: string | null;
  result_summary?: string | null;
  summary: string;
  status?: string | null;
  is_error: boolean;
  requires_confirmation?: boolean | null;
  preview_type?: string | null;
  before_preview?: string | null;
  after_preview?: string | null;
  diff_preview?: string | null;
  changed_ranges?: string[] | null;
}

export interface ThinkingStep {
  thinking: string;
  tool_calls: ToolCallSummary[];
  tool_results: ToolResultSummary[];
}

export interface ToolCallSummary {
  tool_call_id: string;
  tool_name: string;
  tool_input: string;
}

export interface ToolResultSummary {
  tool_call_id: string;
  is_error: boolean;
  summary: string;
}

export interface SkillInfo {
  name: string;
  description: string;
  path: string;
}

export interface AgentRunResponse {
  final_response: string;
  goal_status: string;
  steps: AgentStep[];
}

export interface SlashCommand {
  prefix: string;
  name: string;
  description: string;
}
