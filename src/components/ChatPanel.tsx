import { useState, useRef, useEffect } from "react";
import type { ComposeMode, Conversation, FileNode } from "../types";
import MessageList from "./MessageList";
import ChatInput from "./ChatInput";

interface ChatPanelProps {
  conversation: Conversation | null;
  isLoading: boolean;
  onSend: (content: string, mode: ComposeMode) => void;
  fileNodes: FileNode[];
  skillNames?: string[];
  mcpNames?: string[];
}

export default function ChatPanel({
  conversation,
  isLoading,
  onSend,
  fileNodes,
  skillNames,
  mcpNames,
}: ChatPanelProps) {
  const [inputValue, setInputValue] = useState("");
  const [composeMode, setComposeMode] = useState<ComposeMode>("chat");
  const inputRef = useRef<HTMLTextAreaElement>(null!);

  // 切换对话时聚焦输入框
  useEffect(() => {
    inputRef.current?.focus();
    setInputValue("");
  }, [conversation?.id]);

  const handleSend = (mode: ComposeMode) => {
    if (!inputValue.trim() || isLoading) return;
    onSend(inputValue, mode);
    setInputValue("");
    // 重置输入框高度
    if (inputRef.current) {
      inputRef.current.style.height = "auto";
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend(composeMode);
    }
  };

  return (
    <main className="flex-1 flex flex-col min-w-0">
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
      />

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
      />
    </main>
  );
}
