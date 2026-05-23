use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
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
    pub observations: Vec<AgentObservation>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStepStatus {
    Pending,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPreviewType {
    Full,
    Snippet,
    Diff,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebCapabilityStatus {
    Unavailable,
    Available,
}

#[cfg(test)]
mod tests {
    use super::{AgentLoopStatus, AgentStepStatus};

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
}
