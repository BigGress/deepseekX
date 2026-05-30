import { useMemo, useRef, useState } from "react";
import ChatInput from "../components/ChatInput";
import type {
  ComposeMode,
  FileNode,
  SlashCommandAttachment,
  SlashCommandAttachmentKind,
} from "../types";

const MOCK_FILES: FileNode[] = [
  { name: "src", path: "src", is_directory: true, children: [], size: null },
  { name: "README.md", path: "README.md", is_directory: false, children: null, size: 1200 },
];

const MOCK_SKILLS = ["browser", "gmail", "agent-browser"];
const MOCK_MCPS = ["fetch", "search", "filesystem"];

export default function ChatInputPreview() {
  const [value, setValue] = useState("");
  const [mode, setMode] = useState<ComposeMode>("agent");
  const [submitted, setSubmitted] = useState("");
  const [attachments, setAttachments] = useState<SlashCommandAttachment[]>([]);
  const inputRef = useRef<HTMLTextAreaElement>(null!);

  const attachmentSummary = useMemo(
    () =>
      attachments.map((item) => `${item.kind}:${item.name}`).join(", ") || "(none)",
    [attachments],
  );

  const handleAddCommandAttachment = (kind: SlashCommandAttachmentKind, name: string) => {
    setAttachments((prev) => {
      if (prev.some((item) => item.kind === kind && item.name === name)) {
        return prev;
      }
      return [...prev, { id: `${kind}-${name}`, kind, name }];
    });
  };

  const handleSend = () => {
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
    lines.push("", value);
    setSubmitted(lines.filter((line, index) => line !== "" || index === lines.length - 1).join("\n"));
  };

  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 p-8">
      <div className="max-w-4xl mx-auto space-y-6">
        <header className="space-y-2">
          <h1 className="text-xl font-semibold">ChatInput Slash Preview</h1>
          <p className="text-sm text-neutral-400">
            输入 <code>/skill</code> 或 <code>/mcp</code>，选择候选项后确认会出现附件 chip。
          </p>
        </header>

        <div className="rounded-2xl border border-neutral-800 bg-neutral-925 p-4">
          <ChatInput
            value={value}
            onChange={setValue}
            onSend={handleSend}
            onKeyDown={() => {}}
            mode={mode}
            onModeChange={setMode}
            isLoading={false}
            inputRef={inputRef}
            fileNodes={MOCK_FILES}
            skillNames={MOCK_SKILLS}
            mcpNames={MOCK_MCPS}
            commandAttachments={attachments}
            onAddCommandAttachment={handleAddCommandAttachment}
            onRemoveCommandAttachment={(id) =>
              setAttachments((prev) => prev.filter((item) => item.id !== id))
            }
          />
        </div>

        <section className="grid gap-4 md:grid-cols-2">
          <div className="rounded-xl border border-neutral-800 bg-neutral-925 p-4">
            <h2 className="text-sm font-medium text-neutral-200">当前附件</h2>
            <pre className="mt-3 whitespace-pre-wrap text-xs text-neutral-400">{attachmentSummary}</pre>
          </div>
          <div className="rounded-xl border border-neutral-800 bg-neutral-925 p-4">
            <h2 className="text-sm font-medium text-neutral-200">发送预览</h2>
            <pre className="mt-3 whitespace-pre-wrap text-xs text-neutral-400">{submitted || "(not sent yet)"}</pre>
          </div>
        </section>
      </div>
    </div>
  );
}
