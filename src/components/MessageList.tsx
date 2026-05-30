import { useEffect, useRef } from "react";
import type { AgentFollowUpAction, Message, Turn } from "../types";
import TurnList from "./TurnList";

interface MessageListProps {
  messages: Message[];
  turns?: Turn[];
  isLoading: boolean;
  onAgentFollowUp?: (turn: Turn, action: AgentFollowUpAction) => void;
}

export default function MessageList({
  messages,
  turns,
  isLoading,
  onAgentFollowUp,
}: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, turns]);

  const normalizedTurns = turns && turns.length > 0 ? turns : buildTurnsFromMessages(messages);
  const hasTurns = normalizedTurns.length > 0;

  // 空状态：既无 turns 也无 messages
  if (!hasTurns) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
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
            输入任务描述，或开启 Agent 让 AI 自动规划并执行
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-4 py-6">
      <div className="max-w-3xl mx-auto space-y-6">
        <TurnList
          turns={normalizedTurns}
          isLoading={isLoading}
          onAgentFollowUp={onAgentFollowUp}
        />
        <div ref={bottomRef} />
      </div>
    </div>
  );
}

function buildTurnsFromMessages(messages: Message[]): Turn[] {
  const turns: Turn[] = [];
  let pendingUser: Message | null = null;

  for (const message of messages) {
    if (message.role === "system") {
      continue;
    }

    if (message.role === "user") {
      pendingUser = message;
      turns.push({
        id: `${message.id}-turn`,
        user_input: message.content,
        request_attachments: null,
        thinking_steps: [],
        final_response: null,
        agent_steps: null,
        agent_goal_status: null,
        duration_ms: null,
        error_stage: null,
        retry_count: null,
      });
      continue;
    }

    if (message.role === "assistant") {
      const latestTurn = turns[turns.length - 1];
      if (latestTurn && pendingUser) {
        latestTurn.final_response = message.content;
        pendingUser = null;
      } else {
        turns.push({
          id: `${message.id}-assistant-turn`,
          user_input: "",
          request_attachments: null,
          thinking_steps: [],
          final_response: message.content,
          agent_steps: null,
          agent_goal_status: null,
          duration_ms: null,
          error_stage: null,
          retry_count: null,
        });
      }
    }
  }

  return turns;
}
