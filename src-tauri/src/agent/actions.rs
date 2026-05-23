use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    RetrieveContext {
        reason: String,
        query: String,
        intent: RetrievalIntent,
        preferred_sources: Option<Vec<ContextSource>>,
        max_results: Option<usize>,
    },
    ReadFiles {
        reason: String,
        files: Vec<String>,
    },
    WriteFiles {
        reason: String,
        files: Vec<WriteFileRequest>,
    },
    RunCommand {
        reason: String,
        command: String,
        cwd: Option<String>,
    },
    SummarizeFindings {
        reason: String,
        focus: String,
        output_format: SummaryFormat,
    },
    AskUser {
        reason: String,
        question: String,
    },
    Finish {
        reason: String,
        goal_status: GoalStatus,
        summary: String,
        verification: String,
        what_changed: Option<Vec<String>>,
        next_step: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalIntent {
    UnderstandExistingSystem,
    LocateImplementation,
    FindDocumentation,
    GatherEvidence,
    PrepareReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    WorkspaceCode,
    WorkspaceDocs,
    UserKnowledgeBase,
    WebSearch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SummaryFormat {
    BulletReport,
    ChangeSummary,
    ResearchBrief,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Done,
    Blocked,
    NeedsConfirmation,
}
