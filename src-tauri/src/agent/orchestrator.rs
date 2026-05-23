use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::agent::actions::{
    AgentAction, ContextSource, GoalStatus, RetrievalIntent, SummaryFormat, WriteFileRequest,
};
use crate::agent::executor::{read_workspace_file, run_guarded_command, write_workspace_file, GuardedCommandResult};
use crate::agent::permissions::{evaluate_action_permission, PermissionOutcome};
use crate::agent::planner::{plan_next_action, PlannerRequest};
use crate::agent::report::summarize_observations;
use crate::agent::retrieval::retrieve_context;
use crate::agent::state::{
    AgentLoopStatus, AgentObservation, AgentPreviewType, AgentStepStatus, AgentTaskState,
    WebCapabilityStatus,
};
use crate::api::ApiConfig;

const DEFAULT_MAX_STEPS: usize = 12;
const MAX_FAILURES: usize = 3;
const LOOP_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_FILE_SNIPPET_CHARS: usize = 6000;
const MAX_COMMAND_OUTPUT_CHARS: usize = 4000;

#[derive(Debug, Clone)]
pub struct AgentRunRequest<'a> {
    pub goal: &'a str,
    pub conversation_context: Option<&'a str>,
    pub workspace_root: &'a str,
    pub project_instructions: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentRunResponse {
    pub final_response: String,
    pub goal_status: String,
    pub steps: Vec<AgentObservation>,
}

fn goal_status_label(status: &GoalStatus) -> &'static str {
    match status {
        GoalStatus::Done => "done",
        GoalStatus::Blocked => "blocked",
        GoalStatus::NeedsConfirmation => "needs_confirmation",
    }
}

pub fn should_stop(current_step: usize, max_steps: usize) -> bool {
    current_step >= max_steps
}

pub async fn run_agent_loop(
    config: &ApiConfig,
    request: AgentRunRequest<'_>,
) -> Result<AgentRunResponse, String> {
    let goal = request.goal.trim();
    if goal.is_empty() {
        return Err("任务目标不能为空".to_string());
    }

    let started_at = Instant::now();
    let mut failure_count = 0usize;
    let mut invalid_action_count = 0usize;
    let workspace_listing = format_workspace_listing(request.workspace_root).await;
    let mut state = AgentTaskState {
        goal: goal.to_string(),
        workspace_root: request.workspace_root.to_string(),
        step_index: 0,
        max_steps: DEFAULT_MAX_STEPS,
        status: AgentLoopStatus::Planning,
        plan_summary: Some("先统一检索上下文，再决定精读、修改、验证或整理结果".into()),
        collected_context_refs: Vec::new(),
        knowledge_gaps: Vec::new(),
        report_draft: None,
        source_usage: BTreeMap::new(),
        web_capability: WebCapabilityStatus::Unavailable,
        observations: Vec::new(),
    };

    loop {
        if should_stop(state.step_index, state.max_steps) {
            state.status = AgentLoopStatus::Blocked;
            return Ok(build_terminal_response(
                GoalStatus::Blocked,
                "达到最大步数限制，任务已停止。".into(),
                "Agent 已达到 12 步预算。".into(),
                Some(vec!["请根据当前 observation 拆分更小任务后继续。".into()]),
                Some("重新发起更小范围的 Agent 任务".into()),
                &state,
            ));
        }

        if started_at.elapsed() >= LOOP_TIMEOUT {
            state.status = AgentLoopStatus::Blocked;
            return Ok(build_terminal_response(
                GoalStatus::Blocked,
                "达到总耗时上限，任务已停止。".into(),
                "Agent 总耗时超过 180 秒。".into(),
                None,
                Some("缩小任务范围后重试".into()),
                &state,
            ));
        }

        state.status = AgentLoopStatus::Planning;
        let planner_request = PlannerRequest {
            goal: &state.goal,
            conversation_context: request.conversation_context,
            workspace_root: &state.workspace_root,
            project_instructions: request.project_instructions,
            workspace_listing: &workspace_listing,
            plan_summary: state.plan_summary.as_deref(),
            observations: &state.observations,
            max_steps: state.max_steps,
            remaining_steps: state.max_steps.saturating_sub(state.step_index),
        };

        let action = match plan_next_action(config, &planner_request).await {
            Ok(action) => action,
            Err(error) => {
                invalid_action_count += 1;
                state.observations.push(base_observation(
                    "agent_plan",
                    "planner",
                    "生成下一步动作".into(),
                    truncate(&error, 500),
                    format!("planner 输出无效: {}", truncate(&error, 200)),
                    AgentStepStatus::Failed,
                    true,
                    false,
                ));

                if invalid_action_count >= 2 {
                    state.status = AgentLoopStatus::Failed;
                    return Ok(build_terminal_response(
                        GoalStatus::Blocked,
                        "Planner 连续输出非法动作，任务已中止。".into(),
                        "planner JSON 解析失败".into(),
                        None,
                        Some("调整目标描述或稍后重试".into()),
                        &state,
                    ));
                }

                continue;
            }
        };

        invalid_action_count = 0;
        state.step_index += 1;

        match evaluate_action_permission(Path::new(&state.workspace_root), &action)? {
            PermissionOutcome::Allowed => {}
            PermissionOutcome::RequiresConfirmation { reason, risk_level } => {
                state.status = AgentLoopStatus::AwaitingApproval;
                state.observations.push(blocked_observation(&action, &reason, risk_level));
                return Ok(build_terminal_response(
                    GoalStatus::NeedsConfirmation,
                    format!("{} 需要用户确认后才能继续。", action_name(&action)),
                    format!("权限门拦截：{}（风险等级：{}）", reason, risk_level),
                    None,
                    Some("用户确认后重新发起任务".into()),
                    &state,
                ));
            }
        }

        state.status = AgentLoopStatus::Executing;
        let outcome = execute_action(&state.workspace_root, &action, &state.observations).await;
        state.status = AgentLoopStatus::Observing;

        match outcome {
            ActionExecutionOutcome::Observation(mut observation) => {
                if observation.action_name == "retrieve_context" {
                    for location in parse_collected_refs(&observation.result_summary) {
                        if !state.collected_context_refs.contains(&location) {
                            state.collected_context_refs.push(location);
                        }
                    }
                    update_source_usage(&mut state.source_usage, &observation.result_summary);
                    if observation.result_summary.contains("web_search: unavailable") {
                        state.knowledge_gaps.push("web_search unavailable".into());
                    }
                }

                if observation.action_name == "summarize_findings" {
                    state.report_draft = Some(observation.result_summary.clone());
                }

                if observation.is_error {
                    failure_count += 1;
                } else {
                    failure_count = 0;
                }

                observation.summary = if let Some(report) = state.report_draft.as_ref() {
                    if observation.action_name == "summarize_findings" {
                        format!("已生成阶段性报告（{} chars）", report.len())
                    } else {
                        observation.summary.clone()
                    }
                } else {
                    observation.summary.clone()
                };

                state.plan_summary = Some(update_plan_summary(state.plan_summary.as_deref(), &action));
                state.observations.push(observation);

                if failure_count >= MAX_FAILURES {
                    state.status = AgentLoopStatus::Failed;
                    return Ok(build_terminal_response(
                        GoalStatus::Blocked,
                        "连续执行失败次数过多，任务已停止。".into(),
                        "最近多步 action 均未成功执行。".into(),
                        None,
                        Some("检查错误 observation 后重新尝试".into()),
                        &state,
                    ));
                }
            }
            ActionExecutionOutcome::Finish {
                goal_status,
                summary,
                verification,
                what_changed,
                next_step,
            } => {
                state.status = match goal_status {
                    GoalStatus::Done => AgentLoopStatus::Completed,
                    GoalStatus::Blocked => AgentLoopStatus::Blocked,
                    GoalStatus::NeedsConfirmation => AgentLoopStatus::AwaitingApproval,
                };

                state.observations.push(base_observation(
                    "finish",
                    "planner judged the task as converged",
                    state.plan_summary.clone().unwrap_or_default(),
                    summary.clone(),
                    format!("finish: {}", summary),
                    match goal_status {
                        GoalStatus::Done => AgentStepStatus::Completed,
                        GoalStatus::Blocked | GoalStatus::NeedsConfirmation => AgentStepStatus::Blocked,
                    },
                    !matches!(goal_status, GoalStatus::Done),
                    matches!(goal_status, GoalStatus::NeedsConfirmation),
                ));

                return Ok(build_terminal_response(
                    goal_status,
                    summary,
                    verification,
                    what_changed,
                    next_step,
                    &state,
                ));
            }
        }
    }
}

enum ActionExecutionOutcome {
    Observation(AgentObservation),
    Finish {
        goal_status: GoalStatus,
        summary: String,
        verification: String,
        what_changed: Option<Vec<String>>,
        next_step: Option<String>,
    },
}

async fn execute_action(
    workspace_root: &str,
    action: &AgentAction,
    observations: &[AgentObservation],
) -> ActionExecutionOutcome {
    match action {
        AgentAction::RetrieveContext {
            reason,
            query,
            intent,
            preferred_sources,
            max_results,
        } => {
            let input_summary = format!(
                "query={}, intent={:?}, preferred_sources={:?}, max_results={}",
                query,
                intent,
                preferred_sources,
                max_results.unwrap_or(8)
            );
            match retrieve_context(
                workspace_root,
                query,
                intent,
                preferred_sources.as_deref(),
                max_results.unwrap_or(8),
            )
            .await
            {
                Ok(response) => {
                    let mut parts = response
                        .hits
                        .iter()
                        .map(|hit| {
                            format!(
                                "{} | {} | confidence={:.1}\n{}\nnext_hint={}",
                                hit.source_type, hit.location, hit.confidence, hit.snippet, hit.next_hint
                            )
                        })
                        .collect::<Vec<_>>();
                    for source in &response.unavailable_sources {
                        parts.push(format!("{source}: unavailable"));
                    }
                    let result_summary = truncate(&parts.join("\n\n"), MAX_FILE_SNIPPET_CHARS);
                    ActionExecutionOutcome::Observation(AgentObservation {
                        action_name: "retrieve_context".into(),
                        reason: reason.clone(),
                        input_summary,
                        result_summary: result_summary.clone(),
                        summary: format!("命中 {} 条上下文结果", response.hits.len()),
                        status: AgentStepStatus::Completed,
                        is_error: false,
                        requires_confirmation: false,
                        preview_type: Some(AgentPreviewType::Snippet),
                        before_preview: None,
                        after_preview: Some(result_summary),
                        diff_preview: None,
                        changed_ranges: None,
                    })
                }
                Err(error) => ActionExecutionOutcome::Observation(error_observation(
                    "retrieve_context",
                    reason,
                    input_summary,
                    error,
                )),
            }
        }
        AgentAction::ReadFiles { reason, files } => {
            let input_summary = files.join(", ");
            let mut results: Vec<String> = Vec::new();
            for file in files {
                match read_workspace_file(workspace_root, file).await {
                    Ok(content) => results.push(format!(
                        "## {}\n{}",
                        file,
                        truncate(&content, MAX_FILE_SNIPPET_CHARS)
                    )),
                    Err(error) => {
                        return ActionExecutionOutcome::Observation(error_observation(
                            "read_files",
                            reason,
                            input_summary,
                            format!("{}: {}", file, error),
                        ));
                    }
                }
            }
            let result_summary = truncate(&results.join("\n\n"), MAX_FILE_SNIPPET_CHARS);
            ActionExecutionOutcome::Observation(AgentObservation {
                action_name: "read_files".into(),
                reason: reason.clone(),
                input_summary,
                result_summary: result_summary.clone(),
                summary: format!("成功读取 {} 个文件", files.len()),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Full),
                before_preview: None,
                after_preview: Some(result_summary),
                diff_preview: None,
                changed_ranges: None,
            })
        }
        AgentAction::WriteFiles { reason, files } => {
            let input_summary = summarize_write_requests(files);
            let mut wrote: Vec<String> = Vec::new();
            let mut diff_parts: Vec<String> = Vec::new();
            for file in files {
                let before = read_workspace_file(workspace_root, &file.path).await.ok();
                if let Err(error) = write_workspace_file(workspace_root, &file.path, &file.content).await {
                    return ActionExecutionOutcome::Observation(error_observation(
                        "write_files",
                        reason,
                        input_summary,
                        format!("{}: {}", file.path, error),
                    ));
                }
                wrote.push(format!("{} ({} bytes)", file.path, file.content.len()));
                diff_parts.push(build_diff_preview(&file.path, before.as_deref(), &file.content));
            }
            let result_summary = wrote.join("\n");
            ActionExecutionOutcome::Observation(AgentObservation {
                action_name: "write_files".into(),
                reason: reason.clone(),
                input_summary,
                result_summary,
                summary: format!("已写入 {} 个文件", files.len()),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Diff),
                before_preview: None,
                after_preview: None,
                diff_preview: Some(truncate(&diff_parts.join("\n\n"), MAX_FILE_SNIPPET_CHARS)),
                changed_ranges: Some(
                    files.iter().map(|file| format!("{}:1-{}", file.path, file.content.lines().count())).collect(),
                ),
            })
        }
        AgentAction::RunCommand { reason, command, cwd } => {
            let input_summary = format!("command={}, cwd={}", command, cwd.clone().unwrap_or_else(|| ".".into()));
            match run_guarded_command(workspace_root, command, cwd.as_deref()).await {
                GuardedCommandResult::Executed {
                    exit_code,
                    stdout,
                    stderr,
                } => {
                    let result_summary = format!(
                        "exit_code={}\nstdout:\n{}\nstderr:\n{}",
                        exit_code,
                        truncate(&stdout, MAX_COMMAND_OUTPUT_CHARS),
                        truncate(&stderr, MAX_COMMAND_OUTPUT_CHARS)
                    );
                    let is_error = exit_code != 0;
                    ActionExecutionOutcome::Observation(base_observation(
                        "run_command",
                        reason,
                        input_summary,
                        result_summary,
                        if is_error {
                            format!("命令执行失败，exit code {}", exit_code)
                        } else {
                            format!("命令执行成功，exit code {}", exit_code)
                        },
                        if is_error {
                            AgentStepStatus::Failed
                        } else {
                            AgentStepStatus::Completed
                        },
                        is_error,
                        false,
                    ))
                }
                GuardedCommandResult::RequiresConfirmation { reason: block_reason } => {
                    ActionExecutionOutcome::Observation(base_observation(
                        "run_command",
                        reason,
                        input_summary,
                        block_reason.clone(),
                        format!("命令被权限门拦截: {}", block_reason),
                        AgentStepStatus::Blocked,
                        true,
                        true,
                    ))
                }
                GuardedCommandResult::SpawnFailed { reason: error } => {
                    ActionExecutionOutcome::Observation(error_observation(
                        "run_command",
                        reason,
                        input_summary,
                        error,
                    ))
                }
            }
        }
        AgentAction::SummarizeFindings {
            reason,
            focus,
            output_format,
        } => {
            let input_summary = format!("focus={}, output_format={:?}", focus, output_format);
            let output_key = match output_format {
                SummaryFormat::BulletReport => "bullet_report",
                SummaryFormat::ChangeSummary => "change_summary",
                SummaryFormat::ResearchBrief => "research_brief",
            };
            let report = summarize_observations(observations, focus, output_key);
            ActionExecutionOutcome::Observation(AgentObservation {
                action_name: "summarize_findings".into(),
                reason: reason.clone(),
                input_summary,
                result_summary: truncate(&report, MAX_FILE_SNIPPET_CHARS),
                summary: "已整理阶段性结果".into(),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Snippet),
                before_preview: None,
                after_preview: Some(truncate(&report, MAX_FILE_SNIPPET_CHARS)),
                diff_preview: None,
                changed_ranges: None,
            })
        }
        AgentAction::AskUser { question, .. } => ActionExecutionOutcome::Finish {
            goal_status: GoalStatus::NeedsConfirmation,
            summary: question.clone(),
            verification: "planner 判断下一步需要用户确认".into(),
            what_changed: None,
            next_step: Some("用户回答后重新运行 Agent".into()),
        },
        AgentAction::Finish {
            goal_status,
            summary,
            verification,
            what_changed,
            next_step,
            ..
        } => ActionExecutionOutcome::Finish {
            goal_status: goal_status.clone(),
            summary: summary.clone(),
            verification: verification.clone(),
            what_changed: what_changed.clone(),
            next_step: next_step.clone(),
        },
    }
}

async fn format_workspace_listing(workspace_root: &str) -> String {
    let response = retrieve_context(
        workspace_root,
        "README docs src package cargo agent planner",
        &RetrievalIntent::UnderstandExistingSystem,
        Some(&[ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs]),
        12,
    )
    .await;

    match response {
        Ok(response) => truncate(
            &response
                .hits
                .iter()
                .map(|hit| format!("{} | {}", hit.source_type, hit.location))
                .collect::<Vec<_>>()
                .join("\n"),
            5000,
        ),
        Err(_) => String::new(),
    }
}

fn base_observation(
    action_name: &str,
    reason: &str,
    input_summary: String,
    result_summary: String,
    summary: String,
    status: AgentStepStatus,
    is_error: bool,
    requires_confirmation: bool,
) -> AgentObservation {
    AgentObservation {
        action_name: action_name.into(),
        reason: reason.into(),
        input_summary,
        result_summary,
        summary,
        status,
        is_error,
        requires_confirmation,
        preview_type: None,
        before_preview: None,
        after_preview: None,
        diff_preview: None,
        changed_ranges: None,
    }
}

fn error_observation(
    action_name: &str,
    reason: &str,
    input_summary: String,
    error: String,
) -> AgentObservation {
    base_observation(
        action_name,
        reason,
        input_summary,
        truncate(&error, MAX_COMMAND_OUTPUT_CHARS),
        format!("{} 执行失败", action_name),
        AgentStepStatus::Failed,
        true,
        false,
    )
}

fn blocked_observation(action: &AgentAction, reason: &str, risk_level: &str) -> AgentObservation {
    base_observation(
        action_name(action),
        action_reason(action),
        action_input_summary(action),
        format!("risk_level={risk_level}; {reason}"),
        format!("{} 需要用户确认", action_name(action)),
        AgentStepStatus::Blocked,
        true,
        true,
    )
}

fn action_name(action: &AgentAction) -> &'static str {
    match action {
        AgentAction::RetrieveContext { .. } => "retrieve_context",
        AgentAction::ReadFiles { .. } => "read_files",
        AgentAction::WriteFiles { .. } => "write_files",
        AgentAction::RunCommand { .. } => "run_command",
        AgentAction::SummarizeFindings { .. } => "summarize_findings",
        AgentAction::AskUser { .. } => "ask_user",
        AgentAction::Finish { .. } => "finish",
    }
}

fn action_reason(action: &AgentAction) -> &str {
    match action {
        AgentAction::RetrieveContext { reason, .. }
        | AgentAction::ReadFiles { reason, .. }
        | AgentAction::WriteFiles { reason, .. }
        | AgentAction::RunCommand { reason, .. }
        | AgentAction::SummarizeFindings { reason, .. }
        | AgentAction::AskUser { reason, .. }
        | AgentAction::Finish { reason, .. } => reason,
    }
}

fn action_input_summary(action: &AgentAction) -> String {
    match action {
        AgentAction::RetrieveContext {
            query,
            intent,
            preferred_sources,
            max_results,
            ..
        } => format!(
            "query={}, intent={:?}, preferred_sources={:?}, max_results={}",
            query,
            intent,
            preferred_sources,
            max_results.unwrap_or(8)
        ),
        AgentAction::ReadFiles { files, .. } => files.join(", "),
        AgentAction::WriteFiles { files, .. } => summarize_write_requests(files),
        AgentAction::RunCommand { command, cwd, .. } => format!(
            "command={}, cwd={}",
            command,
            cwd.clone().unwrap_or_else(|| ".".into())
        ),
        AgentAction::SummarizeFindings { focus, output_format, .. } => {
            format!("focus={}, output_format={:?}", focus, output_format)
        }
        AgentAction::AskUser { question, .. } => question.clone(),
        AgentAction::Finish { summary, .. } => summary.clone(),
    }
}

fn summarize_write_requests(files: &[WriteFileRequest]) -> String {
    files
        .iter()
        .map(|file| format!("{} ({} bytes)", file.path, file.content.len()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_diff_preview(path: &str, before: Option<&str>, after: &str) -> String {
    let before_preview = truncate(before.unwrap_or("(new file)"), 500);
    let after_preview = truncate(after, 500);
    format!(
        "--- {}\n{}\n+++ {}\n{}",
        path, before_preview, path, after_preview
    )
}

fn update_plan_summary(previous: Option<&str>, action: &AgentAction) -> String {
    let suffix = match action {
        AgentAction::RetrieveContext { query, .. } => format!("已检索上下文：{}", query),
        AgentAction::ReadFiles { files, .. } => format!("已读取 {}", files.join(", ")),
        AgentAction::WriteFiles { files, .. } => format!(
            "已写入 {}",
            files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>().join(", ")
        ),
        AgentAction::RunCommand { command, .. } => format!("已执行 {}", command),
        AgentAction::SummarizeFindings { focus, .. } => format!("已整理结果：{}", focus),
        AgentAction::AskUser { question, .. } => format!("等待用户确认: {}", question),
        AgentAction::Finish { summary, .. } => format!("已收敛: {}", summary),
    };

    match previous {
        Some(previous) if !previous.trim().is_empty() => format!("{previous}；{suffix}"),
        _ => suffix,
    }
}

fn build_terminal_response(
    goal_status: GoalStatus,
    summary: String,
    verification: String,
    what_changed: Option<Vec<String>>,
    next_step: Option<String>,
    state: &AgentTaskState,
) -> AgentRunResponse {
    let mut lines = vec![summary.clone(), String::new(), format!("验证: {}", verification)];
    if let Some(report) = state.report_draft.as_ref() {
        lines.push(String::new());
        lines.push("整理结果:".into());
        lines.push(report.clone());
    }
    if let Some(what_changed) = what_changed.filter(|items| !items.is_empty()) {
        lines.push("变更:".into());
        lines.extend(what_changed.into_iter().map(|item| format!("- {}", item)));
    }
    if !state.collected_context_refs.is_empty() {
        lines.push("上下文命中:".into());
        lines.extend(
            state
                .collected_context_refs
                .iter()
                .take(8)
                .map(|location| format!("- {}", location)),
        );
    }
    if let Some(next_step) = next_step.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("下一步: {}", next_step));
    }

    AgentRunResponse {
        final_response: lines.join("\n"),
        goal_status: goal_status_label(&goal_status).to_string(),
        steps: state.observations.clone(),
    }
}

fn parse_collected_refs(result_summary: &str) -> Vec<String> {
    result_summary
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('|').map(str::trim);
            let _source = parts.next()?;
            let location = parts.next()?;
            if location.is_empty() {
                None
            } else {
                Some(location.to_string())
            }
        })
        .collect()
}

fn update_source_usage(source_usage: &mut BTreeMap<String, usize>, result_summary: &str) {
    for line in result_summary.lines() {
        if let Some((source, _rest)) = line.split_once('|') {
            let key = source.trim().to_string();
            *source_usage.entry(key).or_insert(0) += 1;
        }
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let truncated: String = value.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_diff_preview, goal_status_label, parse_collected_refs, should_stop,
        summarize_write_requests, update_plan_summary,
    };
    use crate::agent::actions::{
        AgentAction, ContextSource, GoalStatus, RetrievalIntent, SummaryFormat, WriteFileRequest,
    };

    #[test]
    fn stops_when_step_budget_is_exhausted() {
        assert!(should_stop(12, 12));
        assert!(should_stop(13, 12));
        assert!(!should_stop(3, 12));
    }

    #[test]
    fn serializes_goal_status_to_frontend_value() {
        assert_eq!(goal_status_label(&GoalStatus::Done), "done");
        assert_eq!(goal_status_label(&GoalStatus::Blocked), "blocked");
        assert_eq!(
            goal_status_label(&GoalStatus::NeedsConfirmation),
            "needs_confirmation"
        );
    }

    #[test]
    fn write_summary_includes_paths_and_sizes() {
        let summary = summarize_write_requests(&[WriteFileRequest {
            path: "src/App.tsx".into(),
            content: "hello".into(),
        }]);
        assert!(summary.contains("src/App.tsx"));
        assert!(summary.contains("5 bytes"));
    }

    #[test]
    fn plan_summary_accumulates_progress() {
        let updated = update_plan_summary(
            Some("先统一检索"),
            &AgentAction::RetrieveContext {
                reason: "inspect".into(),
                query: "agent orchestrator".into(),
                intent: RetrievalIntent::UnderstandExistingSystem,
                preferred_sources: Some(vec![ContextSource::WorkspaceCode]),
                max_results: Some(6),
            },
        );

        assert!(updated.contains("先统一检索"));
        assert!(updated.contains("agent orchestrator"));
    }

    #[test]
    fn parses_locations_from_retrieval_summary() {
        let refs = parse_collected_refs(
            "workspace_code | src/agent/orchestrator.rs | confidence=4.0\nworkspace_docs | docs/agent.md | confidence=3.0",
        );
        assert_eq!(refs, vec!["src/agent/orchestrator.rs", "docs/agent.md"]);
    }

    #[test]
    fn diff_preview_includes_before_and_after() {
        let diff = build_diff_preview("src/App.tsx", Some("old"), "new");
        assert!(diff.contains("--- src/App.tsx"));
        assert!(diff.contains("+++ src/App.tsx"));
    }

    #[test]
    fn summarize_action_is_supported_in_plan_updates() {
        let updated = update_plan_summary(
            None,
            &AgentAction::SummarizeFindings {
                reason: "wrap up".into(),
                focus: "agent capability".into(),
                output_format: SummaryFormat::ChangeSummary,
            },
        );
        assert!(updated.contains("agent capability"));
    }
}
