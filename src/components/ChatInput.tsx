import { useState, useRef, useEffect } from "react";
import type {
  ComposeMode,
  FileNode,
  SlashCommandAttachment,
  SlashCommandAttachmentKind,
} from "../types";

type AutocompleteItem =
  | { kind: "file"; value: string; label: string; description?: string }
  | { kind: "skill" | "mcp"; value: string; label: string; description?: string };

interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSend: (mode: ComposeMode) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  mode: ComposeMode;
  onModeChange: (mode: ComposeMode) => void;
  isLoading: boolean;
  inputRef: React.RefObject<HTMLTextAreaElement>;
  fileNodes: FileNode[];
  skillNames?: string[];
  mcpNames?: string[];
  commandAttachments: SlashCommandAttachment[];
  onAddCommandAttachment: (kind: SlashCommandAttachmentKind, name: string) => void;
  onRemoveCommandAttachment: (id: string) => void;
}

export default function ChatInput({
  value,
  onChange,
  onSend,
  onKeyDown,
  mode,
  onModeChange,
  isLoading,
  inputRef,
  fileNodes,
  skillNames,
  mcpNames,
  commandAttachments,
  onAddCommandAttachment,
  onRemoveCommandAttachment,
}: ChatInputProps) {
  const [showAutocomplete, setShowAutocomplete] = useState(false);
  const [autocompleteItems, setAutocompleteItems] = useState<AutocompleteItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const autocompleteRef = useRef<HTMLDivElement>(null);

  // 检测 @ 输入并匹配文件
  useEffect(() => {
    const cursorPos = inputRef.current?.selectionStart ?? value.length;
    const textBeforeCursor = value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/@([^\s@]*)$/);

    if (match) {
      const query = match[1].toLowerCase();
      const allPaths = flattenFilePaths(fileNodes);
      const filtered = allPaths
        .filter((p) => p.toLowerCase().includes(query))
        .slice(0, 8)
        .map((path) => ({ kind: "file" as const, value: path, label: `@${path}`, description: path }));
      if (filtered.length > 0) {
        setAutocompleteItems(filtered);
        setSelectedIndex(0);
        setShowAutocomplete(true);
        return;
      }
    }
    setShowAutocomplete(false);
  }, [value]);

  // 斜杠命令检测
  useEffect(() => {
    const cursorPos = inputRef.current?.selectionStart ?? value.length;
    const textBeforeCursor = value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/^\/(?:(skill|mcp)\s+)?([^\s]*)$/);

    if (match && (skillNames?.length || mcpNames?.length)) {
      const explicitKind = match[1] as SlashCommandAttachmentKind | undefined;
      const query = match[2].toLowerCase();
      const items: AutocompleteItem[] = [];
      if (!explicitKind || explicitKind === "skill") {
        skillNames?.forEach((name) =>
          items.push({
            kind: "skill",
            value: name,
            label: `/skill ${name}`,
            description: "添加到本次请求的技能附件",
          }),
        );
      }
      if (!explicitKind || explicitKind === "mcp") {
        mcpNames?.forEach((name) =>
          items.push({
            kind: "mcp",
            value: name,
            label: `/mcp ${name}`,
            description: "添加到本次请求的 MCP 附件",
          }),
        );
      }
      const filtered = items
        .filter((item) => item.label.toLowerCase().includes(query))
        .slice(0, 8);
      if (filtered.length > 0) {
        setAutocompleteItems(filtered);
        setSelectedIndex(0);
        setShowAutocomplete(true);
        return;
      }
    }
    if (textBeforeCursor.startsWith("/")) {
      setShowAutocomplete(false);
    }
  }, [value, skillNames, mcpNames]);

  // 键盘导航自动补全
  const handleCustomKeyDown = (e: React.KeyboardEvent) => {
    if (showAutocomplete) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(prev + 1, autocompleteItems.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.max(prev - 1, 0));
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        applyAutocomplete(autocompleteItems[selectedIndex]);
        return;
      }
      if (e.key === "Escape") {
        setShowAutocomplete(false);
        return;
      }
    }
    onKeyDown(e);
  };

  const applyAutocomplete = (item: AutocompleteItem) => {
    if (item.kind === "file") {
      insertFileAutocomplete(item.value);
      return;
    }

    onAddCommandAttachment(item.kind, item.value);
    clearSlashCommand();
  };

  const insertFileAutocomplete = (filePath: string) => {
    const cursorPos = inputRef.current?.selectionStart ?? value.length;
    const textBeforeCursor = value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/@([^\s@]*)$/);
    if (match) {
      const beforeAt = textBeforeCursor.slice(0, textBeforeCursor.length - match[0].length);
      const newValue = beforeAt + "@" + filePath + value.slice(cursorPos);
      onChange(newValue);
      setShowAutocomplete(false);
      // 将光标移到插入内容之后
      const newPos = beforeAt.length + 1 + filePath.length;
      setTimeout(() => {
        if (inputRef.current) {
          inputRef.current.selectionStart = newPos;
          inputRef.current.selectionEnd = newPos;
        }
      }, 0);
    }
  };

  const clearSlashCommand = () => {
    const cursorPos = inputRef.current?.selectionStart ?? value.length;
    const textBeforeCursor = value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/^\/(?:(skill|mcp)\s+)?([^\s]*)$/);
    if (!match) {
      return;
    }

    const newValue = value.slice(cursorPos).replace(/^\s*/, "");
    onChange(newValue);
    setShowAutocomplete(false);
    setAutocompleteItems([]);
    setSelectedIndex(0);
    setTimeout(() => {
      if (inputRef.current) {
        inputRef.current.selectionStart = 0;
        inputRef.current.selectionEnd = 0;
      }
    }, 0);
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value);
    const el = e.target;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 160) + "px";
  };

  // 粗略 token 估算 (中文 ~1, 英文 ~0.3, 按字符数/1.8 估算)
  const estimatedTokens = Math.ceil(value.length / 1.8);
  const tokenWarning = estimatedTokens > 8000;

  return (
    <footer className="border-t border-neutral-800 bg-neutral-925 p-4 shrink-0">
      <div className="max-w-3xl mx-auto relative">
        <div className="relative flex items-end gap-2 bg-neutral-850 rounded-xl border border-neutral-700 focus-within:border-neutral-500 transition-colors p-2">
          {commandAttachments.length > 0 && (
            <div className="absolute left-2 right-2 top-2 flex flex-wrap gap-2">
              {commandAttachments.map((attachment) => (
                <button
                  key={attachment.id}
                  type="button"
                  onClick={() => onRemoveCommandAttachment(attachment.id)}
                  className={`inline-flex items-center gap-2 rounded-full border px-2.5 py-1 text-[11px] transition-colors ${
                    attachment.kind === "skill"
                      ? "border-sky-700 bg-sky-500/10 text-sky-200 hover:bg-sky-500/20"
                      : "border-amber-700 bg-amber-500/10 text-amber-200 hover:bg-amber-500/20"
                  }`}
                >
                  <span>{attachment.kind === "skill" ? "Skill" : "MCP"}</span>
                  <span className="font-medium">{attachment.name}</span>
                  <span className="text-neutral-400">×</span>
                </button>
              ))}
            </div>
          )}
          <textarea
            ref={inputRef}
            value={value}
            onChange={handleInput}
            onKeyDown={handleCustomKeyDown}
            placeholder={
              mode === "agent"
                ? "输入目标，Agent 会自动规划并逐步执行..."
                : "输入任务描述... (Enter 发送, Shift+Enter 换行, @文件路径 引用文件)"
            }
            rows={1}
            disabled={isLoading}
            className="flex-1 bg-transparent text-sm text-neutral-200 placeholder-neutral-500 
                       resize-none outline-none px-2 py-1 max-h-40 disabled:opacity-50"
            style={{ paddingTop: commandAttachments.length > 0 ? "2.75rem" : undefined }}
          />
          <button
            type="button"
            onClick={() => onModeChange(mode === "agent" ? "chat" : "agent")}
            disabled={isLoading}
            className={`shrink-0 h-9 px-3 rounded-lg border text-xs font-medium transition-colors ${
              mode === "agent"
                ? "border-emerald-500 bg-emerald-500/15 text-emerald-300"
                : "border-neutral-700 bg-neutral-900 text-neutral-400 hover:text-neutral-200"
            } disabled:opacity-50`}
          >
            Agent
          </button>
          <button
            onClick={() => onSend(mode)}
            disabled={!value.trim() || isLoading}
            className="shrink-0 w-9 h-9 rounded-lg bg-blue-600 hover:bg-blue-500 
                       disabled:bg-neutral-700 disabled:cursor-not-allowed
                       text-white transition-colors flex items-center justify-center"
          >
            {isLoading ? (
              <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" 
                        className="opacity-25" />
                <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="3" 
                      strokeLinecap="round" className="opacity-75" />
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 12h14M12 5l7 7-7 7" />
              </svg>
            )}
          </button>
        </div>

        {/* 文件自动补全 */}
        {showAutocomplete && (
          <div
            ref={autocompleteRef}
            className="absolute bottom-full left-0 right-0 mb-1 bg-neutral-850 border border-neutral-700 rounded-lg 
                       shadow-xl max-h-48 overflow-y-auto z-10"
          >
            {autocompleteItems.map((item, idx) => (
              <div
                key={`${item.kind}-${item.value}`}
                onClick={() => applyAutocomplete(item)}
                className={`px-3 py-1.5 text-xs cursor-pointer flex items-center gap-2 transition-colors ${
                  idx === selectedIndex
                    ? "bg-neutral-700 text-neutral-200"
                    : "text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
                }`}
              >
                <svg className="w-3 h-3 shrink-0" viewBox="0 0 24 24" fill="currentColor">
                  {item.kind === "file" ? (
                    <path d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm-1 2l5 5h-5V4z" />
                  ) : (
                    <path d="M12 2 3 7l9 5 9-5-9-5Zm0 7.2L5.3 6 12 2.8 18.7 6 12 9.2Zm-7 4.2L12 18l7-4.6v4L12 22l-7-4.4v-4Z" />
                  )}
                </svg>
                <div className="min-w-0">
                  <p className="truncate">{item.label}</p>
                  {item.description && (
                    <p className="truncate text-[10px] text-neutral-500">{item.description}</p>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}

        {/* 上下文指示器 */}
        <div className="flex items-center justify-between mt-2">
          <p className="text-xs text-neutral-600">
            {mode === "agent"
              ? "Agent 模式会进行多步规划、执行和观察"
              : "DeepSeekX 调用 DeepSeek TUI 来执行任务"}
          </p>
          <div className="flex items-center gap-2">
            {value.length > 0 && (
              <span
                className={`text-xs ${
                  tokenWarning ? "text-yellow-400" : "text-neutral-600"
                }`}
              >
                ~{estimatedTokens} tokens
              </span>
            )}
            <span className="text-xs text-neutral-600">
              @文件路径 引用文件 · /skill · /mcp 添加本次请求附件
            </span>
          </div>
        </div>
      </div>
    </footer>
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
