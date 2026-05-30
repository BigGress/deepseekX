import { useState, useRef, useEffect } from "react";
import type {
  AgentFollowUpAction,
  ComposeMode,
  Conversation,
  FileNode,
  LlmRequestLogEntry,
  SlashCommandAttachment,
  SlashCommandAttachmentKind,
  Turn,
} from "../types";
import MessageList from "./MessageList";
import ChatInput from "./ChatInput";
import LlmDebugPanel from "./LlmDebugPanel";

interface ChatPanelProps {
  conversation: Conversation | null;
  isLoading: boolean;
  onSend: (content: string, mode: ComposeMode) => void;
  onAgentFollowUp?: (turn: Turn, action: AgentFollowUpAction) => void;
  fileNodes: FileNode[];
  skillNames?: string[];
  mcpNames?: string[];
  debugLlmResponsesEnabled?: boolean;
  llmDebugEntries?: LlmRequestLogEntry[];
}

export default function ChatPanel({
  conversation,
  isLoading,
  onSend,
  onAgentFollowUp,
  fileNodes,
  skillNames,
  mcpNames,
  debugLlmResponsesEnabled = false,
  llmDebugEntries = [],
}: ChatPanelProps) {
  const [inputValue, setInputValue] = useState("");
  const [composeMode, setComposeMode] = useState<ComposeMode>("agent");
  const [commandAttachments, setCommandAttachments] = useState<SlashCommandAttachment[]>([]);
  const inputRef = useRef<HTMLTextAreaElement>(null!);

  // 切换对话时聚焦输入框
  useEffect(() => {
    inputRef.current?.focus();
    setInputValue("");
    setComposeMode("agent");
    setCommandAttachments([]);
  }, [conversation?.id]);

  const handleSend = (mode: ComposeMode) => {
    if (!inputValue.trim() || isLoading) return;
    onSend(composePrompt(inputValue, commandAttachments), mode);
    setInputValue("");
    setCommandAttachments([]);
    // 重置输入框高度
    if (inputRef.current) {
      inputRef.current.style.height = "auto";
    }
  };

  const handleAddCommandAttachment = (kind: SlashCommandAttachmentKind, name: string) => {
    setCommandAttachments((prev) => {
      if (prev.some((item) => item.kind === kind && item.name === name)) {
        return prev;
      }
      return [...prev, { id: `${kind}-${name}`, kind, name }];
    });
  };

  const handleRemoveCommandAttachment = (id: string) => {
    setCommandAttachments((prev) => prev.filter((item) => item.id !== id));
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend(composeMode);
    }
  };

  return (
    <main className="flex min-h-0 min-w-0 flex-1 flex-col">
      {/* 顶部标题栏 */}
      <header className="h-12 border-b border-neutral-800 flex items-center px-4 bg-neutral-925 shrink-0">
        <h1 className="text-sm font-medium text-neutral-300 truncate">
          {conversation?.title ?? "DeepSeekX"}
        </h1>
      </header>

      {/* 消息区域 */}
      <MessageList
        messages={conversation?.messages ?? []}
        turns={conversation?.turns}
        isLoading={isLoading}
        onAgentFollowUp={onAgentFollowUp}
      />

      {debugLlmResponsesEnabled ? <LlmDebugPanel entries={llmDebugEntries} /> : null}

      {/* 输入区域 */}
      <ChatInput
        value={inputValue}
        onChange={setInputValue}
        onSend={handleSend}
        onKeyDown={handleKeyDown}
        mode={composeMode}
        onModeChange={setComposeMode}
        isLoading={isLoading}
        inputRef={inputRef}
        fileNodes={fileNodes}
        skillNames={skillNames}
        mcpNames={mcpNames}
        commandAttachments={commandAttachments}
        onAddCommandAttachment={handleAddCommandAttachment}
        onRemoveCommandAttachment={handleRemoveCommandAttachment}
      />
    </main>
  );
}

function composePrompt(inputValue: string, attachments: SlashCommandAttachment[]) {
  if (attachments.length === 0) {
    return inputValue;
  }

  const skills = attachments
    .filter((item) => item.kind === "skill")
    .map((item) => item.name);
  const mcps = attachments
    .filter((item) => item.kind === "mcp")
    .map((item) => item.name);

  const lines: string[] = [];
  if (skills.length > 0) {
    lines.push(`本次请求显式附加技能: ${skills.join(", ")}`);
  }
  if (mcps.length > 0) {
    lines.push(`本次请求显式附加 MCP: ${mcps.join(", ")}`);
  }
  lines.push("", inputValue);
  return lines.join("\n");
}
