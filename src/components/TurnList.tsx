import type { AgentFollowUpAction, Turn } from "../types";
import TurnItem from "./TurnItem";

interface TurnListProps {
  turns: Turn[];
  isLoading: boolean;
  onAgentFollowUp?: (turn: Turn, action: AgentFollowUpAction) => void;
}

export default function TurnList({ turns, isLoading, onAgentFollowUp }: TurnListProps) {
  if (turns.length === 0) {
    return null;
  }

  return (
    <div className="space-y-8">
      {turns.map((turn, index) => (
        <TurnItem
          key={turn.id}
          turn={turn}
          isLatest={index === turns.length - 1}
          isLoading={isLoading}
          onAgentFollowUp={onAgentFollowUp}
        />
      ))}
    </div>
  );
}
