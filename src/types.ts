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
  retrieval_sources: string;
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

export interface LlmRequestLogEntry {
  timestamp: string;
  request_id: string;
  endpoint: string;
  model: string;
  web_search_enabled: boolean;
  request_body: unknown;
  status_code?: number | null;
  duration_ms: number;
  success: boolean;
  error?: string | null;
  response_text?: string | null;
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
  request_attachments?: RequestAttachment[] | null;
  thinking_steps: ThinkingStep[];
  final_response: string | null;
  agent_goal_status?: string | null;
  agent_steps?: AgentStep[] | null;
  duration_ms?: number | null;
  error_stage?: string | null;
  retry_count?: number | null;
  llm_debug_responses?: LlmDebugResponse[] | null;
}

export type RequestAttachmentKind = "skill" | "mcp" | "file";

export interface RequestAttachment {
  kind: RequestAttachmentKind;
  name: string;
}

export interface LlmDebugResponse {
  request_id: string;
  endpoint: string;
  model: string;
  web_search_enabled: boolean;
  duration_ms: number;
  response_text: string;
}

export type AgentFollowUpAction = "approve" | "reject" | "retry";

export type SlashCommandAttachmentKind = "skill" | "mcp";

export interface SlashCommandAttachment {
  id: string;
  kind: SlashCommandAttachmentKind;
  name: string;
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
  file_operations?: FileOperation[] | null;
}

export interface FileOperation {
  path: string;
  operation: string;
  target_path?: string | null;
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
  llm_debug_responses?: LlmDebugResponse[] | null;
}

export interface SlashCommand {
  prefix: string;
  name: string;
  description: string;
}

// ============ 文件预览系统类型 ============

export type PreviewMode = "structured" | "rendered" | "raw" | "metadata";

export type PreviewCategory =
  | "text" | "markdown" | "code" | "image" | "audio" | "video"
  | "pdf" | "html" | "csv" | "spreadsheet" | "document" | "presentation"
  | "archive" | "binary" | "unknown";

export interface PreviewCapabilities {
  readonly can_render_inline: boolean;
  readonly can_open_focused: boolean;
  readonly can_download: boolean;
  readonly can_show_text_extract: boolean;
  readonly can_show_original_appearance: boolean;
}

export type PreviewContent =
  | { kind: "text"; text: string; language?: string }
  | { kind: "markdown"; markdown: string }
  | { kind: "html"; html: string; sandboxed: boolean }
  | { kind: "table"; columns: string[]; rows: string[][] }
  | { kind: "media"; url: string; mediaType: "image" | "audio" | "video" | "pdf" }
  | { kind: "pages"; pages: Array<{ page: number; imageUrl: string }> }
  | { kind: "fallback"; message: string };

export interface PreviewMetadata {
  readonly detected_encoding?: string;
  readonly line_count?: number;
  readonly page_count?: number;
  readonly sheet_names?: string[];
  readonly width?: number;
  readonly height?: number;
  readonly duration_seconds?: number;
  readonly generated_by?: string;
}

export interface PreviewDescriptor {
  path: string;
  file_name: string;
  extension: string | null;
  mime_type: string | null;
  size_bytes: number | null;
  category: PreviewCategory;
  default_mode: PreviewMode;
  available_modes: PreviewMode[];
  capabilities: PreviewCapabilities;
  content: PreviewContent | null;
  metadata: PreviewMetadata | null;
  warnings: string[];
}

export interface PreviewTarget {
  path: string;
  source: "file_tree" | "chat_attachment" | "artifact" | "diff";
  title?: string;
}

export interface PreviewState {
  isOpen: boolean;
  target: PreviewTarget | null;
  descriptor: PreviewDescriptor | null;
  selectedMode: PreviewMode | null;
  isLoading: boolean;
  error: string | null;
  focusedView: boolean;
}
