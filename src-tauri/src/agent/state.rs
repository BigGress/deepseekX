use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskState {
    pub goal: String,
    pub workspace_root: String,
    pub step_index: usize,
    pub max_steps: usize,
    pub status: AgentLoopStatus,
    pub plan_summary: Option<String>,
    pub collected_context_refs: Vec<String>,
    pub knowledge_gaps: Vec<String>,
    pub report_draft: Option<String>,
    pub source_usage: BTreeMap<String, usize>,
    pub web_capability: WebCapabilityStatus,
    pub session_state: AgentSessionStateMachine,
    pub observations: Vec<AgentObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    Idle,
    Retrieving,
    Reading,
    Verifying,
    Synthesizing,
    AwaitingConfirmation,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AgentFacts {
    pub verified_sources: Vec<String>,
    pub retrieved_refs: Vec<String>,
    pub verification_evidence: Vec<String>,
    pub source_usage: BTreeMap<String, usize>,
    pub applied_file_operations: Vec<FileOperationResult>,
    pub knowledge_gaps: Vec<String>,
    pub last_error: Option<String>,
    pub last_confirmation_request: Option<String>,
    pub last_completed_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AgentWorkingState {
    pub working_hypothesis: Option<String>,
    pub open_questions: Vec<String>,
    pub next_plan: Vec<String>,
    pub draft_summary: Option<String>,
    pub confidence_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentStateTimestamps {
    pub updated_at: String,
    pub facts_updated_at: Option<String>,
    pub working_state_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSessionStateMachine {
    pub conversation_id: String,
    pub version: u32,
    pub phase: AgentPhase,
    pub facts: AgentFacts,
    pub working_state: AgentWorkingState,
    pub timestamps: AgentStateTimestamps,
}

impl AgentSessionStateMachine {
    pub fn new(conversation_id: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            conversation_id,
            version: 1,
            phase: AgentPhase::Idle,
            facts: AgentFacts::default(),
            working_state: AgentWorkingState::default(),
            timestamps: AgentStateTimestamps {
                updated_at: now,
                facts_updated_at: None,
                working_state_updated_at: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLoopStatus {
    Planning,
    AwaitingApproval,
    Executing,
    Observing,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentObservation {
    pub action_name: String,
    pub reason: String,
    pub input_summary: String,
    pub result_summary: String,
    pub summary: String,
    pub status: AgentStepStatus,
    pub is_error: bool,
    pub requires_confirmation: bool,
    pub preview_type: Option<AgentPreviewType>,
    pub before_preview: Option<String>,
    pub after_preview: Option<String>,
    pub diff_preview: Option<String>,
    pub changed_ranges: Option<Vec<String>>,
    pub file_operations: Option<Vec<FileOperationResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileOperationResult {
    pub path: String,
    pub operation: FileOperationKind,
    pub target_path: Option<String>,
    pub changed_ranges: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileOperationKind {
    Create,
    Modify,
    Overwrite,
    Delete,
    Rename,
}

impl fmt::Display for FileOperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            FileOperationKind::Create => "create",
            FileOperationKind::Modify => "modify",
            FileOperationKind::Overwrite => "overwrite",
            FileOperationKind::Delete => "delete",
            FileOperationKind::Rename => "rename",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStepStatus {
    Pending,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentPreviewType {
    Full,
    Snippet,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebCapabilityStatus {
    Unavailable,
    Available,
}

#[cfg(test)]
mod tests {
    use super::{AgentLoopStatus, AgentPhase, AgentSessionStateMachine, AgentStepStatus};

    #[test]
    fn serializes_status_as_snake_case() {
        let value = serde_json::to_string(&AgentLoopStatus::AwaitingApproval).unwrap();
        assert_eq!(value, "\"awaiting_approval\"");
    }

    #[test]
    fn serializes_step_status_as_snake_case() {
        let value = serde_json::to_string(&AgentStepStatus::Completed).unwrap();
        assert_eq!(value, "\"completed\"");
    }

    #[test]
    fn session_state_defaults_to_idle_and_empty_facts() {
        let state = AgentSessionStateMachine::new("conversation-1".into());

        assert_eq!(state.phase, AgentPhase::Idle);
        assert_eq!(state.version, 1);
        assert!(state.facts.verified_sources.is_empty());
        assert!(state.facts.retrieved_refs.is_empty());
        assert!(state.facts.verification_evidence.is_empty());
        assert!(state.facts.applied_file_operations.is_empty());
        assert!(state.facts.knowledge_gaps.is_empty());
        assert!(state.working_state.open_questions.is_empty());
        assert!(state.working_state.next_plan.is_empty());
    }

    #[test]
    fn serializes_agent_phase_as_snake_case() {
        let value = serde_json::to_string(&AgentPhase::AwaitingConfirmation).unwrap();
        assert_eq!(value, "\"awaiting_confirmation\"");
    }
}
