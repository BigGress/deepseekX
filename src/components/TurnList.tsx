import type { Turn } from "../types";
import TurnItem from "./TurnItem";

interface TurnListProps {
  turns: Turn[];
}

export default function TurnList({ turns }: TurnListProps) {
  if (turns.length === 0) {
    return null;
  }

  return (
    <div className="space-y-8">
      {turns.map((turn) => (
        <TurnItem key={turn.id} turn={turn} />
      ))}
    </div>
  );
}
