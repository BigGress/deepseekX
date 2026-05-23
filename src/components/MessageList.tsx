import { useEffect, useRef } from "react";
import type { Message, Turn } from "../types";
import TurnList from "./TurnList";

interface MessageListProps {
  messages: Message[];
  turns?: Turn[];
  isLoading: boolean;
}

export default function MessageList({ messages, turns, isLoading }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, turns]);

  const hasTurns = turns && turns.length > 0;
  const hasMessages = messages.length > 0;

  // 空状态：既无 turns 也无 messages
  if (!hasTurns && !hasMessages) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <div className="mb-4">
            <svg
              className="w-16 h-16 mx-auto text-neutral-700"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
            </svg>
          </div>
          <h2 className="text-2xl font-semibold text-neutral-300 mb-2">
            DeepSeekX
          </h2>
          <p className="text-neutral-500 text-sm">
            输入任务描述，让 AI 帮你完成
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-6">
      <div className="max-w-3xl mx-auto space-y-6">
        {/* Turn 渲染路径（优先） */}
        {hasTurns ? (
          <TurnList turns={turns!} />
        ) : (
          /* 旧 Message 渲染路径 */
          messages.map((msg) => (
            <div
              key={msg.id}
              className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
            >
              <div
                className={`max-w-[85%] rounded-lg px-4 py-3 text-sm leading-relaxed ${
                  msg.role === "user"
                    ? "bg-blue-600 text-white"
                    : msg.role === "system"
                      ? "bg-neutral-800 text-yellow-300"
                      : "bg-neutral-850 text-neutral-200"
                }`}
              >
                {msg.role === "user" ? (
                  <p>{msg.content}</p>
                ) : (
                  <div className="prose prose-invert prose-sm max-w-none">
                    <MessageContent content={msg.content} />
                  </div>
                )}
              </div>
            </div>
          ))
        )}
        {/* Turn 模式下由 TurnItem 显示加载动画；Message 模式下在底部显示 */}
        {isLoading && !hasTurns && (
          <div className="flex justify-start">
            <div className="bg-neutral-850 rounded-lg px-4 py-3">
              <div className="flex items-center gap-1">
                <span className="w-2 h-2 bg-neutral-500 rounded-full animate-bounce" style={{ animationDelay: "0ms" }} />
                <span className="w-2 h-2 bg-neutral-500 rounded-full animate-bounce" style={{ animationDelay: "150ms" }} />
                <span className="w-2 h-2 bg-neutral-500 rounded-full animate-bounce" style={{ animationDelay: "300ms" }} />
              </div>
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}

function MessageContent({ content }: { content: string }) {
  // 简单的 markdown 代码块渲染
  const parts = content.split(/(```[\s\S]*?```)/g);
  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith("```")) {
          const code = part
            .replace(/```\w*\s*file:\S+\n?/, "")  // 去除 file: 标记
            .replace(/```\w*\n?/, "")             // 普通代码块
            .replace(/```$/, "");
          return (
            <pre
              key={i}
              className="bg-neutral-900 rounded-md p-3 my-2 overflow-x-auto text-xs text-neutral-300"
            >
              <code>{code}</code>
            </pre>
          );
        }
        return <span key={i} className="whitespace-pre-wrap">{part}</span>;
      })}
    </>
  );
}
