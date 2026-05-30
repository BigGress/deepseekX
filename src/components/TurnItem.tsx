import type { AgentFollowUpAction, Turn } from "../types";
import ArtifactPanel from "./turn/ArtifactPanel";
import ExecutionPanel from "./turn/ExecutionPanel";
import ResultCardGroup from "./turn/ResultCardGroup";
import StatusPanel from "./turn/StatusPanel";
import UserIntentCard from "./turn/UserIntentCard";
import { shouldExpandExecutionByDefault } from "./turn/turnPresentation";

interface TurnItemProps {
  turn: Turn;
  isLatest?: boolean;
  isLoading?: boolean;
  onAgentFollowUp?: (turn: Turn, action: AgentFollowUpAction) => void;
  onPreviewFile?: (path: string) => void;
}

export default function TurnItem({
  turn,
  isLatest = false,
  isLoading = false,
  onAgentFollowUp,
  onPreviewFile,
}: TurnItemProps) {
  return (
    <article className="space-y-3">
      <UserIntentCard userInput={turn.user_input} attachments={turn.request_attachments} onPreviewFile={onPreviewFile} />
      <ResultCardGroup turn={turn} />
      <StatusPanel
        turn={turn}
        isLatest={isLatest}
        isLoading={isLoading}
        onAgentFollowUp={onAgentFollowUp}
      />
      <ExecutionPanel
        thinkingSteps={turn.thinking_steps}
        agentSteps={turn.agent_steps}
        defaultExpanded={shouldExpandExecutionByDefault(turn)}
      />
      <ArtifactPanel agentSteps={turn.agent_steps} onPreviewFile={onPreviewFile} />
    </article>
  );
}
