use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::agent::actions::{
    AgentAction, ContextSource, GoalStatus, PatchFileRequest, RenameFileRequest,
    RetrievalIntent, SummaryFormat, VerificationCheck, WriteFileRequest,
};
use crate::agent::executor::{
    apply_workspace_patch, create_workspace_file, delete_workspace_entry, read_workspace_file,
    fetch_url, rename_workspace_entry, run_guarded_command, write_workspace_file,
    GuardedCommandResult,
};
use crate::agent::mcp::{call_tool, McpToolDescriptor};
use crate::agent::permissions::{evaluate_action_permission, validate_fetch_url, PermissionOutcome};
use crate::agent::planner::{plan_next_action_with_debug, PlannerRequest};
use crate::agent::report::summarize_observations;
use crate::agent::retrieval::retrieve_context;
use crate::agent::state::{
    AgentLoopStatus, AgentObservation, AgentPhase, AgentPreviewType, AgentSessionStateMachine,
    AgentStepStatus, AgentTaskState, FileOperationKind, FileOperationResult,
    WebCapabilityStatus,
};
use crate::api::{ApiConfig, LlmDebugResponse, McpServerConfig};

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
    pub restored_session_state: AgentSessionStateMachine,
    pub project_instructions: Option<&'a str>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub mcp_tools: Vec<McpToolDescriptor>,
    pub retrieval_sources: Vec<ContextSource>,
    pub user_knowledge_base_paths: Vec<String>,
    pub command_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentRunResponse {
    pub final_response: String,
    pub goal_status: String,
    pub steps: Vec<AgentObservation>,
    pub llm_debug_responses: Vec<LlmDebugResponse>,
    #[serde(skip_serializing)]
    pub session_state: AgentSessionStateMachine,
}

pub(crate) fn goal_status_label(status: &GoalStatus) -> &'static str {
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
    let workspace_listing = format_workspace_listing(config, request.workspace_root).await;
    let mut llm_debug_responses: Vec<LlmDebugResponse> = Vec::new();
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
        session_state: request.restored_session_state.clone(),
        observations: Vec::new(),
    };

    loop {
        if should_stop(state.step_index, state.max_steps) {
            state.status = AgentLoopStatus::Blocked;
            state.session_state.facts.last_error = Some("达到最大步数限制".into());
            set_session_phase(&mut state.session_state, AgentPhase::Blocked);
            update_session_timestamp(&mut state.session_state, true);
            return Ok(build_terminal_response(
                GoalStatus::Blocked,
                "达到最大步数限制，任务已停止。".into(),
                "Agent 已达到 12 步预算。".into(),
                Some(vec!["请根据当前 observation 拆分更小任务后继续。".into()]),
                Some("重新发起更小范围的 Agent 任务".into()),
                llm_debug_responses.clone(),
                &state,
            ));
        }

        if started_at.elapsed() >= LOOP_TIMEOUT {
            state.status = AgentLoopStatus::Blocked;
            state.session_state.facts.last_error = Some("达到总耗时上限".into());
            set_session_phase(&mut state.session_state, AgentPhase::Blocked);
            update_session_timestamp(&mut state.session_state, true);
            return Ok(build_terminal_response(
                GoalStatus::Blocked,
                "达到总耗时上限，任务已停止。".into(),
                "Agent 总耗时超过 180 秒。".into(),
                None,
                Some("缩小任务范围后重试".into()),
                llm_debug_responses.clone(),
                &state,
            ));
        }

        state.status = AgentLoopStatus::Planning;
        let mcp_catalog = summarize_mcp_catalog(&request.mcp_tools);
        let session_state_summary = summarize_session_state_for_planner(&state.session_state);
        let planner_request = PlannerRequest {
            goal: &state.goal,
            conversation_context: request.conversation_context,
            workspace_root: &state.workspace_root,
            project_instructions: request.project_instructions,
            workspace_listing: &workspace_listing,
            plan_summary: state.plan_summary.as_deref(),
            session_state_summary: Some(&session_state_summary),
            observations: &state.observations,
            available_mcp_tools: mcp_catalog.as_deref(),
            max_steps: state.max_steps,
            remaining_steps: state.max_steps.saturating_sub(state.step_index),
        };

        let action = match plan_next_action_with_debug(config, &planner_request).await {
            Ok(decision) => {
                llm_debug_responses.push(decision.llm_debug_response);
                decision.action
            }
            Err(error) => {
                if let Some(debug) = error.llm_debug_response {
                    llm_debug_responses.push(debug);
                }
                invalid_action_count += 1;
                state.observations.push(base_observation(
                    "agent_plan",
                    "planner",
                    "生成下一步动作".into(),
                    truncate(&error.message, 500),
                    format!("planner 输出无效: {}", truncate(&error.message, 200)),
                    AgentStepStatus::Failed,
                    true,
                    false,
                ));

                if invalid_action_count >= 2 {
                    state.status = AgentLoopStatus::Failed;
                    state.session_state.facts.last_error = Some("planner JSON 解析失败".into());
                    set_session_phase(&mut state.session_state, AgentPhase::Blocked);
                    update_session_timestamp(&mut state.session_state, true);
                    return Ok(build_terminal_response(
                        GoalStatus::Blocked,
                        "Planner 连续输出非法动作，任务已中止。".into(),
                        "planner JSON 解析失败".into(),
                        None,
                        Some("调整目标描述或稍后重试".into()),
                        llm_debug_responses.clone(),
                        &state,
                    ));
                }

                continue;
            }
        };

        invalid_action_count = 0;
        state.step_index += 1;

        match evaluate_action_permission(
            Path::new(&state.workspace_root),
            &action,
            &request.command_allowlist,
        )? {
            PermissionOutcome::Allowed => {}
            PermissionOutcome::RequiresConfirmation { reason, risk_level } => {
                state.status = AgentLoopStatus::AwaitingApproval;
                state.session_state.facts.last_confirmation_request =
                    Some(format!("{}（风险等级：{}）", reason, risk_level));
                set_session_phase(&mut state.session_state, AgentPhase::AwaitingConfirmation);
                update_session_timestamp(&mut state.session_state, true);
                state
                    .observations
                    .push(blocked_observation(&action, &reason, risk_level));
                return Ok(build_terminal_response(
                    GoalStatus::NeedsConfirmation,
                    format!("{} 需要用户确认后才能继续。", action_name(&action)),
                    format!("权限门拦截：{}（风险等级：{}）", reason, risk_level),
                    None,
                    Some("用户确认后重新发起任务".into()),
                    llm_debug_responses.clone(),
                    &state,
                ));
            }
        }

        state.status = AgentLoopStatus::Executing;
        let outcome = execute_action(
            config,
            &state.workspace_root,
            &action,
            &state.observations,
            &request.mcp_servers,
            &request.retrieval_sources,
            &request.user_knowledge_base_paths,
            &request.command_allowlist,
        )
        .await;
        state.status = AgentLoopStatus::Observing;

        match outcome {
            ActionExecutionOutcome::Observation {
                mut observation,
                llm_debug_responses: mut action_llm_debug_responses,
            } => {
                llm_debug_responses.append(&mut action_llm_debug_responses);
                if observation.action_name == "retrieve_context" {
                    for location in parse_collected_refs(&observation.result_summary) {
                        if !state.collected_context_refs.contains(&location) {
                            state.collected_context_refs.push(location);
                        }
                    }
                    update_source_usage(&mut state.source_usage, &observation.result_summary);
                    if observation
                        .result_summary
                        .contains("web_search: unavailable")
                    {
                        state.knowledge_gaps.push("web_search unavailable".into());
                    }
                    if observation
                        .result_summary
                        .contains("user_knowledge_base: unavailable")
                    {
                        state
                            .knowledge_gaps
                            .push("user_knowledge_base unavailable".into());
                    }
                }

                if observation.action_name == "summarize_findings" {
                    state.report_draft = Some(observation.result_summary.clone());
                }

                update_session_state_from_observation(&mut state, &action, &observation);

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

                state.plan_summary =
                    Some(update_plan_summary(state.plan_summary.as_deref(), &action));
                state.observations.push(observation);

                if failure_count >= MAX_FAILURES {
                    state.status = AgentLoopStatus::Failed;
                    state.session_state.facts.last_error = Some("连续执行失败次数过多".into());
                    set_session_phase(&mut state.session_state, AgentPhase::Blocked);
                    update_session_timestamp(&mut state.session_state, true);
                    return Ok(build_terminal_response(
                        GoalStatus::Blocked,
                        "连续执行失败次数过多，任务已停止。".into(),
                        "最近多步 action 均未成功执行。".into(),
                        None,
                        Some("检查错误 observation 后重新尝试".into()),
                        llm_debug_responses.clone(),
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
                state.session_state.phase = match goal_status {
                    GoalStatus::Done => AgentPhase::Completed,
                    GoalStatus::Blocked => AgentPhase::Blocked,
                    GoalStatus::NeedsConfirmation => AgentPhase::AwaitingConfirmation,
                };
                state.session_state.facts.last_completed_action = Some("finish".into());
                state.session_state.facts.last_error = if matches!(goal_status, GoalStatus::Done) {
                    None
                } else {
                    Some(truncate(&verification, 300))
                };
                update_session_timestamp(&mut state.session_state, true);

                state.observations.push(base_observation(
                    "finish",
                    "planner judged the task as converged",
                    state.plan_summary.clone().unwrap_or_default(),
                    summary.clone(),
                    format!("finish: {}", summary),
                    match goal_status {
                        GoalStatus::Done => AgentStepStatus::Completed,
                        GoalStatus::Blocked | GoalStatus::NeedsConfirmation => {
                            AgentStepStatus::Blocked
                        }
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
                        llm_debug_responses,
                    &state,
                ));
            }
        }
    }
}

pub(crate) enum ActionExecutionOutcome {
    Observation {
        observation: AgentObservation,
        llm_debug_responses: Vec<LlmDebugResponse>,
    },
    Finish {
        goal_status: GoalStatus,
        summary: String,
        verification: String,
        what_changed: Option<Vec<String>>,
        next_step: Option<String>,
    },
}

fn observation_outcome(observation: AgentObservation) -> ActionExecutionOutcome {
    ActionExecutionOutcome::Observation {
        observation,
        llm_debug_responses: Vec::new(),
    }
}

fn observation_outcome_with_debug(
    observation: AgentObservation,
    llm_debug_responses: Vec<LlmDebugResponse>,
) -> ActionExecutionOutcome {
    ActionExecutionOutcome::Observation {
        observation,
        llm_debug_responses,
    }
}

pub(crate) async fn execute_action(
    config: &ApiConfig,
    workspace_root: &str,
    action: &AgentAction,
    observations: &[AgentObservation],
    mcp_servers: &[McpServerConfig],
    allowed_retrieval_sources: &[ContextSource],
    user_knowledge_base_paths: &[String],
    command_allowlist: &[String],
) -> ActionExecutionOutcome {
    match action {
        AgentAction::RetrieveContext {
            reason,
            query,
            intent,
            preferred_sources,
            max_results,
        } => {
            let effective_sources = resolve_retrieval_sources(
                preferred_sources.as_deref(),
                allowed_retrieval_sources,
            );
            let input_summary = format!(
                "query={}, intent={:?}, preferred_sources={:?}, max_results={}",
                query,
                intent,
                effective_sources,
                max_results.unwrap_or(8)
            );
            match retrieve_context(
                Some(config),
                workspace_root,
                query,
                intent,
                Some(&effective_sources),
                max_results.unwrap_or(8),
                user_knowledge_base_paths,
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
                                hit.source_type,
                                hit.location,
                                hit.confidence,
                                hit.snippet,
                                hit.next_hint
                            )
                        })
                        .collect::<Vec<_>>();
                    for source in &response.unavailable_sources {
                        parts.push(format!("{source}: unavailable"));
                    }
                    let result_summary = truncate(&parts.join("\n\n"), MAX_FILE_SNIPPET_CHARS);
                    observation_outcome_with_debug(AgentObservation {
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
                        file_operations: None,
                    }, response.llm_debug_responses)
                }
                Err(error) => observation_outcome(error_observation(
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
                        return observation_outcome(error_observation(
                            "read_files",
                            reason,
                            input_summary,
                            format!("{}: {}", file, error),
                        ));
                    }
                }
            }
            let result_summary = truncate(&results.join("\n\n"), MAX_FILE_SNIPPET_CHARS);
            observation_outcome(AgentObservation {
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
                file_operations: None,
            })
        }
        AgentAction::CreateFiles { reason, files } => {
            let input_summary = summarize_write_requests(files);
            let mut wrote: Vec<String> = Vec::new();
            let mut diff_parts: Vec<String> = Vec::new();
            for file in files {
                if let Err(error) =
                    create_workspace_file(workspace_root, &file.path, &file.content).await
                {
                    return observation_outcome(error_observation(
                        "create_files",
                        reason,
                        input_summary,
                        format!("{}: {}", file.path, error),
                    ));
                }
                wrote.push(format!("{} ({} bytes)", file.path, file.content.len()));
                diff_parts.push(build_diff_preview(&file.path, None, &file.content));
            }
            let result_summary = wrote.join("\n");
            observation_outcome(AgentObservation {
                action_name: "create_files".into(),
                reason: reason.clone(),
                input_summary,
                result_summary,
                summary: format!("已创建 {} 个文件", files.len()),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Diff),
                before_preview: None,
                after_preview: None,
                diff_preview: Some(truncate(&diff_parts.join("\n\n"), MAX_FILE_SNIPPET_CHARS)),
                changed_ranges: Some(
                    files
                        .iter()
                        .map(|file| format!("{}:1-{}", file.path, file.content.lines().count()))
                        .collect(),
                ),
                file_operations: Some(
                    files
                        .iter()
                        .map(|file| FileOperationResult {
                            path: file.path.clone(),
                            operation: FileOperationKind::Create,
                            target_path: None,
                            changed_ranges: Some(vec![format!(
                                "{}:1-{}",
                                file.path,
                                file.content.lines().count()
                            )]),
                        })
                        .collect(),
                ),
            })
        }
        AgentAction::RenameFiles { reason, renames } => {
            let input_summary = summarize_rename_requests(renames);
            let mut results = Vec::new();
            for rename in renames {
                if let Err(error) =
                    rename_workspace_entry(workspace_root, &rename.from_path, &rename.to_path).await
                {
                    return observation_outcome(error_observation(
                        "rename_files",
                        reason,
                        input_summary,
                        format!("{} -> {}: {}", rename.from_path, rename.to_path, error),
                    ));
                }
                results.push(format!("{} -> {}", rename.from_path, rename.to_path));
            }
            let result_summary = results.join("\n");
            observation_outcome(AgentObservation {
                action_name: "rename_files".into(),
                reason: reason.clone(),
                input_summary,
                result_summary,
                summary: format!("已重命名 {} 个路径", renames.len()),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Snippet),
                before_preview: None,
                after_preview: Some(truncate(&results.join("\n"), MAX_FILE_SNIPPET_CHARS)),
                diff_preview: None,
                changed_ranges: None,
                file_operations: Some(
                    renames
                        .iter()
                        .map(|rename| FileOperationResult {
                            path: rename.from_path.clone(),
                            operation: FileOperationKind::Rename,
                            target_path: Some(rename.to_path.clone()),
                            changed_ranges: None,
                        })
                        .collect(),
                ),
            })
        }
        AgentAction::DeleteFiles { reason, paths } => {
            let input_summary = paths.join(", ");
            let mut results = Vec::new();
            let mut previews = Vec::new();
            for path in paths {
                let before = read_workspace_file(workspace_root, path).await.ok();
                if let Err(error) = delete_workspace_entry(workspace_root, path).await {
                    return observation_outcome(error_observation(
                        "delete_files",
                        reason,
                        input_summary,
                        format!("{}: {}", path, error),
                    ));
                }
                results.push(format!("deleted {}", path));
                previews.push(build_diff_preview(
                    path,
                    before.as_deref(),
                    "(deleted file)",
                ));
            }
            let result_summary = results.join("\n");
            observation_outcome(AgentObservation {
                action_name: "delete_files".into(),
                reason: reason.clone(),
                input_summary,
                result_summary,
                summary: format!("已删除 {} 个路径", paths.len()),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Diff),
                before_preview: None,
                after_preview: None,
                diff_preview: Some(truncate(&previews.join("\n\n"), MAX_FILE_SNIPPET_CHARS)),
                changed_ranges: None,
                file_operations: Some(
                    paths
                        .iter()
                        .map(|path| FileOperationResult {
                            path: path.clone(),
                            operation: FileOperationKind::Delete,
                            target_path: None,
                            changed_ranges: None,
                        })
                        .collect(),
                ),
            })
        }
        AgentAction::ApplyPatch { reason, patches } => {
            let input_summary = summarize_patch_requests(patches);
            let mut wrote: Vec<String> = Vec::new();
            let mut diff_parts: Vec<String> = Vec::new();
            let mut changed_ranges: Vec<String> = Vec::new();
            for patch in patches {
                let before = match read_workspace_file(workspace_root, &patch.path).await {
                    Ok(content) => content,
                    Err(error) => {
                        return observation_outcome(error_observation(
                            "apply_patch",
                            reason,
                            input_summary,
                            format!("{}: {}", patch.path, error),
                        ));
                    }
                };
                let applied = match apply_workspace_patch(workspace_root, &patch.path, &patch.hunks).await
                {
                    Ok(applied) => applied,
                    Err(error) => {
                        return observation_outcome(error_observation(
                            "apply_patch",
                            reason,
                            input_summary,
                            format!("{}: {}", patch.path, error),
                        ));
                    }
                };
                wrote.push(format!(
                    "{} ({} hunks)",
                    patch.path, applied.applied_hunks
                ));
                changed_ranges.extend(applied.changed_ranges.clone());
                diff_parts.push(build_diff_preview(
                    &patch.path,
                    Some(&before),
                    &applied.after,
                ));
            }
            let result_summary = wrote.join("\n");
            observation_outcome(AgentObservation {
                action_name: "apply_patch".into(),
                reason: reason.clone(),
                input_summary,
                result_summary,
                summary: format!("已应用 {} 个补丁文件", patches.len()),
                status: AgentStepStatus::Completed,
                is_error: false,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Diff),
                before_preview: None,
                after_preview: None,
                diff_preview: Some(truncate(&diff_parts.join("\n\n"), MAX_FILE_SNIPPET_CHARS)),
                changed_ranges: Some(changed_ranges.clone()),
                file_operations: Some(
                    patches
                        .iter()
                        .map(|patch| FileOperationResult {
                            path: patch.path.clone(),
                            operation: FileOperationKind::Modify,
                            target_path: None,
                            changed_ranges: Some(
                                changed_ranges
                                    .iter()
                                    .filter(|range| range.starts_with(&format!("{}:", patch.path)))
                                    .cloned()
                                    .collect(),
                            ),
                        })
                        .collect(),
                ),
            })
        }
        AgentAction::WriteFiles { reason, files } => {
            let input_summary = summarize_write_requests(files);
            let mut wrote: Vec<String> = Vec::new();
            let mut diff_parts: Vec<String> = Vec::new();
            let mut changed_ranges: Vec<String> = Vec::new();
            let mut file_operations: Vec<FileOperationResult> = Vec::new();
            for file in files {
                let before = read_workspace_file(workspace_root, &file.path).await.ok();
                let operation = if before.is_some() {
                    FileOperationKind::Overwrite
                } else {
                    FileOperationKind::Create
                };
                let file_range = format!("{}:1-{}", file.path, file.content.lines().count());
                if let Err(error) =
                    write_workspace_file(workspace_root, &file.path, &file.content).await
                {
                    return observation_outcome(error_observation(
                        "write_files",
                        reason,
                        input_summary,
                        format!("{}: {}", file.path, error),
                    ));
                }
                wrote.push(format!("{} ({} bytes)", file.path, file.content.len()));
                diff_parts.push(build_diff_preview(
                    &file.path,
                    before.as_deref(),
                    &file.content,
                ));
                changed_ranges.push(file_range.clone());
                file_operations.push(FileOperationResult {
                    path: file.path.clone(),
                    operation,
                    target_path: None,
                    changed_ranges: Some(vec![file_range]),
                });
            }
            let result_summary = wrote.join("\n");
            observation_outcome(AgentObservation {
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
                changed_ranges: Some(changed_ranges),
                file_operations: Some(file_operations),
            })
        }
        AgentAction::RunCommand {
            reason,
            command,
            cwd,
        } => {
            let input_summary = format!(
                "command={}, cwd={}",
                command,
                cwd.clone().unwrap_or_else(|| ".".into())
            );
            match run_guarded_command(
                workspace_root,
                command,
                cwd.as_deref(),
                command_allowlist,
            )
            .await
            {
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
                    observation_outcome(base_observation(
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
                GuardedCommandResult::RequiresConfirmation {
                    reason: block_reason,
                } => observation_outcome(base_observation(
                    "run_command",
                    reason,
                    input_summary,
                    block_reason.clone(),
                    format!("命令被权限门拦截: {}", block_reason),
                    AgentStepStatus::Blocked,
                    true,
                    true,
                )),
                GuardedCommandResult::SpawnFailed { reason: error } => {
                    observation_outcome(error_observation(
                        "run_command",
                        reason,
                        input_summary,
                        error,
                    ))
                }
            }
        }
        AgentAction::FetchUrl {
            reason,
            url,
            method,
            max_chars,
        } => {
            let input_summary = format!(
                "url={}, method={:?}, max_chars={}",
                url,
                method,
                max_chars.unwrap_or(4000)
            );
            if let Err(error) = validate_fetch_url(url) {
                return observation_outcome(error_observation(
                    "fetch_url",
                    reason,
                    input_summary,
                    error,
                ));
            }
            match fetch_url(url, method, max_chars.unwrap_or(4000)).await {
                Ok(result) => {
                    let mut result_parts = vec![
                        format!("status_code={}", result.status_code),
                        format!("final_url={}", result.url),
                    ];
                    if let Some(content_type) = result.content_type.as_ref() {
                        result_parts.push(format!("content_type={content_type}"));
                    }
                    if let Some(body_excerpt) = result.body_excerpt.as_ref() {
                        result_parts.push(format!(
                            "body:\n{}",
                            truncate(body_excerpt, MAX_FILE_SNIPPET_CHARS)
                        ));
                    }
                    let result_summary = result_parts.join("\n");
                    let is_error = result.status_code >= 400;
                    observation_outcome(AgentObservation {
                        action_name: "fetch_url".into(),
                        reason: reason.clone(),
                        input_summary,
                        result_summary: result_summary.clone(),
                        summary: if is_error {
                            format!("URL 抓取失败，HTTP {}", result.status_code)
                        } else {
                            format!("已抓取 URL，HTTP {}", result.status_code)
                        },
                        status: if is_error {
                            AgentStepStatus::Failed
                        } else {
                            AgentStepStatus::Completed
                        },
                        is_error,
                        requires_confirmation: false,
                        preview_type: Some(AgentPreviewType::Snippet),
                        before_preview: None,
                        after_preview: result
                            .body_excerpt
                            .map(|value| truncate(&value, MAX_FILE_SNIPPET_CHARS)),
                        diff_preview: None,
                        changed_ranges: None,
                        file_operations: None,
                    })
                }
                Err(error) => observation_outcome(error_observation(
                    "fetch_url",
                    reason,
                    input_summary,
                    error,
                )),
            }
        }
        AgentAction::VerifyChecks { reason, checks } => {
            let input_summary = summarize_verification_checks(checks);
            let mut passed = 0usize;
            let mut result_parts = Vec::new();
            let mut preview_lines = Vec::new();

            for check in checks {
                let cwd_label = check.cwd.as_deref().unwrap_or(".");
                match run_guarded_command(
                    workspace_root,
                    &check.command,
                    check.cwd.as_deref(),
                    command_allowlist,
                )
                .await
                {
                    GuardedCommandResult::Executed {
                        exit_code,
                        stdout,
                        stderr,
                    } => {
                        let ok = exit_code == 0;
                        if ok {
                            passed += 1;
                        }
                        preview_lines.push(format!(
                            "- [{}] {} ({})",
                            if ok { "pass" } else { "fail" },
                            check.label,
                            check.command
                        ));
                        result_parts.push(format!(
                            "## {}\ncommand={}\ncwd={}\nstatus={}\nexit_code={}\nstdout:\n{}\nstderr:\n{}",
                            check.label,
                            check.command,
                            cwd_label,
                            if ok { "pass" } else { "fail" },
                            exit_code,
                            truncate(&stdout, MAX_COMMAND_OUTPUT_CHARS),
                            truncate(&stderr, MAX_COMMAND_OUTPUT_CHARS),
                        ));
                    }
                    GuardedCommandResult::RequiresConfirmation { reason: block_reason } => {
                        return observation_outcome(base_observation(
                            "verify_checks",
                            reason,
                            input_summary,
                            block_reason.clone(),
                            format!("验证命令被权限门拦截: {}", block_reason),
                            AgentStepStatus::Blocked,
                            true,
                            true,
                        ));
                    }
                    GuardedCommandResult::SpawnFailed { reason: error } => {
                        result_parts.push(format!(
                            "## {}\ncommand={}\ncwd={}\nstatus=spawn_failed\nerror={}",
                            check.label, check.command, cwd_label, error
                        ));
                        preview_lines.push(format!("- [fail] {} ({})", check.label, check.command));
                    }
                }
            }

            let total = checks.len();
            let all_passed = passed == total;
            let result_summary = truncate(&result_parts.join("\n\n"), MAX_FILE_SNIPPET_CHARS);
            observation_outcome(AgentObservation {
                action_name: "verify_checks".into(),
                reason: reason.clone(),
                input_summary,
                result_summary,
                summary: if all_passed {
                    format!("验证通过：{passed}/{total}")
                } else {
                    format!("验证未全部通过：{passed}/{total}")
                },
                status: if all_passed {
                    AgentStepStatus::Completed
                } else {
                    AgentStepStatus::Failed
                },
                is_error: !all_passed,
                requires_confirmation: false,
                preview_type: Some(AgentPreviewType::Snippet),
                before_preview: None,
                after_preview: Some(truncate(&preview_lines.join("\n"), MAX_FILE_SNIPPET_CHARS)),
                diff_preview: None,
                changed_ranges: None,
                file_operations: None,
            })
        }
        AgentAction::CallMcpTool {
            reason,
            server,
            tool,
            arguments,
        } => {
            let input_summary = format!(
                "server={}, tool={}, arguments={}",
                server,
                tool,
                truncate(&arguments.to_string(), MAX_COMMAND_OUTPUT_CHARS)
            );
            match call_tool(mcp_servers, server, tool, arguments).await {
                Ok(result) => observation_outcome(base_observation(
                    "call_mcp_tool",
                    reason,
                    input_summary,
                    truncate(&result.content, MAX_FILE_SNIPPET_CHARS),
                    if result.is_error {
                        format!("MCP 工具 {}.{} 返回错误", server, tool)
                    } else {
                        format!("已调用 MCP 工具 {}.{}", server, tool)
                    },
                    if result.is_error {
                        AgentStepStatus::Failed
                    } else {
                        AgentStepStatus::Completed
                    },
                    result.is_error,
                    false,
                )),
                Err(error) => observation_outcome(error_observation(
                    "call_mcp_tool",
                    reason,
                    input_summary,
                    error,
                )),
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
            observation_outcome(AgentObservation {
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
                file_operations: None,
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

fn resolve_retrieval_sources(
    preferred_sources: Option<&[ContextSource]>,
    allowed_sources: &[ContextSource],
) -> Vec<ContextSource> {
    if allowed_sources.is_empty() {
        return preferred_sources.unwrap_or_default().to_vec();
    }

    if let Some(preferred_sources) = preferred_sources {
        let filtered = preferred_sources
            .iter()
            .filter(|source| allowed_sources.contains(source))
            .cloned()
            .collect::<Vec<_>>();
        if !filtered.is_empty() {
            return filtered;
        }
    }

    allowed_sources.to_vec()
}

async fn format_workspace_listing(config: &ApiConfig, workspace_root: &str) -> String {
    let response = retrieve_context(
        Some(config),
        workspace_root,
        "README docs src package cargo agent planner",
        &RetrievalIntent::UnderstandExistingSystem,
        Some(&[ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs]),
        12,
        &[],
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
        file_operations: None,
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
        AgentAction::CreateFiles { .. } => "create_files",
        AgentAction::RenameFiles { .. } => "rename_files",
        AgentAction::DeleteFiles { .. } => "delete_files",
        AgentAction::ApplyPatch { .. } => "apply_patch",
        AgentAction::WriteFiles { .. } => "write_files",
        AgentAction::RunCommand { .. } => "run_command",
        AgentAction::FetchUrl { .. } => "fetch_url",
        AgentAction::VerifyChecks { .. } => "verify_checks",
        AgentAction::CallMcpTool { .. } => "call_mcp_tool",
        AgentAction::SummarizeFindings { .. } => "summarize_findings",
        AgentAction::AskUser { .. } => "ask_user",
        AgentAction::Finish { .. } => "finish",
    }
}

fn action_reason(action: &AgentAction) -> &str {
    match action {
        AgentAction::RetrieveContext { reason, .. }
        | AgentAction::ReadFiles { reason, .. }
        | AgentAction::CreateFiles { reason, .. }
        | AgentAction::RenameFiles { reason, .. }
        | AgentAction::DeleteFiles { reason, .. }
        | AgentAction::ApplyPatch { reason, .. }
        | AgentAction::WriteFiles { reason, .. }
        | AgentAction::RunCommand { reason, .. }
        | AgentAction::FetchUrl { reason, .. }
        | AgentAction::VerifyChecks { reason, .. }
        | AgentAction::CallMcpTool { reason, .. }
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
        AgentAction::CreateFiles { files, .. } => summarize_write_requests(files),
        AgentAction::RenameFiles { renames, .. } => summarize_rename_requests(renames),
        AgentAction::DeleteFiles { paths, .. } => paths.join(", "),
        AgentAction::ApplyPatch { patches, .. } => summarize_patch_requests(patches),
        AgentAction::WriteFiles { files, .. } => summarize_write_requests(files),
        AgentAction::RunCommand { command, cwd, .. } => format!(
            "command={}, cwd={}",
            command,
            cwd.clone().unwrap_or_else(|| ".".into())
        ),
        AgentAction::FetchUrl {
            url,
            method,
            max_chars,
            ..
        } => format!(
            "url={}, method={:?}, max_chars={}",
            url,
            method,
            max_chars.unwrap_or(4000)
        ),
        AgentAction::VerifyChecks { checks, .. } => summarize_verification_checks(checks),
        AgentAction::CallMcpTool {
            server,
            tool,
            arguments,
            ..
        } => format!("server={}, tool={}, arguments={}", server, tool, arguments),
        AgentAction::SummarizeFindings {
            focus,
            output_format,
            ..
        } => {
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

fn summarize_patch_requests(patches: &[PatchFileRequest]) -> String {
    patches
        .iter()
        .map(|patch| format!("{} ({} hunks)", patch.path, patch.hunks.len()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn summarize_rename_requests(renames: &[RenameFileRequest]) -> String {
    renames
        .iter()
        .map(|rename| format!("{} -> {}", rename.from_path, rename.to_path))
        .collect::<Vec<_>>()
        .join(", ")
}

fn summarize_verification_checks(checks: &[VerificationCheck]) -> String {
    checks
        .iter()
        .map(|check| {
            format!(
                "{}: {} @ {}",
                check.label,
                check.command,
                check.cwd.as_deref().unwrap_or(".")
            )
        })
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

fn summarize_mcp_catalog(tools: &[McpToolDescriptor]) -> Option<String> {
    if tools.is_empty() {
        return None;
    }

    Some(
        tools
            .iter()
            .map(|tool| {
                let description = tool.description.as_deref().unwrap_or("(无描述)");
                format!(
                    "- {}.{}: {} | input_schema={}",
                    tool.server,
                    tool.name,
                    description,
                    truncate(&tool.input_schema.to_string(), 240)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn update_plan_summary(previous: Option<&str>, action: &AgentAction) -> String {
    let suffix = match action {
        AgentAction::RetrieveContext { query, .. } => format!("已检索上下文：{}", query),
        AgentAction::ReadFiles { files, .. } => format!("已读取 {}", files.join(", ")),
        AgentAction::CreateFiles { files, .. } => format!(
            "已创建 {}",
            files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AgentAction::RenameFiles { renames, .. } => format!(
            "已重命名 {}",
            renames
                .iter()
                .map(|rename| format!("{} -> {}", rename.from_path, rename.to_path))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AgentAction::DeleteFiles { paths, .. } => {
            format!("已删除 {}", paths.join(", "))
        }
        AgentAction::ApplyPatch { patches, .. } => format!(
            "已补丁修改 {}",
            patches
                .iter()
                .map(|patch| patch.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AgentAction::WriteFiles { files, .. } => format!(
            "已写入 {}",
            files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AgentAction::RunCommand { command, .. } => format!("已执行 {}", command),
        AgentAction::FetchUrl { url, .. } => format!("已抓取 {}", url),
        AgentAction::VerifyChecks { checks, .. } => format!(
            "已验证 {}",
            checks
                .iter()
                .map(|check| check.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AgentAction::CallMcpTool { server, tool, .. } => format!("已调用 {}.{}", server, tool),
        AgentAction::SummarizeFindings { focus, .. } => format!("已整理结果：{}", focus),
        AgentAction::AskUser { question, .. } => format!("等待用户确认: {}", question),
        AgentAction::Finish { summary, .. } => format!("已收敛: {}", summary),
    };

    match previous {
        Some(previous) if !previous.trim().is_empty() => format!("{previous}；{suffix}"),
        _ => suffix,
    }
}

fn update_session_timestamp(
    state: &mut AgentSessionStateMachine,
    facts_updated: bool,
) {
    let now = chrono::Utc::now().to_rfc3339();
    state.timestamps.updated_at = now.clone();
    if facts_updated {
        state.timestamps.facts_updated_at = Some(now);
    }
}

fn set_session_phase(state: &mut AgentSessionStateMachine, phase: AgentPhase) {
    state.phase = phase;
    update_session_timestamp(state, false);
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.iter().any(|item| item == &value) {
        items.push(value);
    }
}

fn record_verification_evidence(facts: &mut crate::agent::state::AgentFacts, evidence: String) {
    push_unique(&mut facts.verification_evidence, truncate(&evidence, 300));
}

fn sync_facts_from_runtime_state(state: &mut AgentTaskState) {
    state.session_state.facts.retrieved_refs = state.collected_context_refs.clone();
    state.session_state.facts.verified_sources = state.collected_context_refs.clone();
    state.session_state.facts.source_usage = state.source_usage.clone();
    state.session_state.facts.knowledge_gaps = state.knowledge_gaps.clone();
    update_session_timestamp(&mut state.session_state, true);
}

fn update_session_state_from_observation(
    state: &mut AgentTaskState,
    action: &AgentAction,
    observation: &AgentObservation,
) {
    if observation.requires_confirmation {
        state.session_state.facts.last_confirmation_request = Some(observation.summary.clone());
        set_session_phase(&mut state.session_state, AgentPhase::AwaitingConfirmation);
        update_session_timestamp(&mut state.session_state, true);
        return;
    }

    if observation.is_error {
        state.session_state.facts.last_error = Some(truncate(&observation.result_summary, 300));
        update_session_timestamp(&mut state.session_state, true);
        return;
    }

    state.session_state.facts.last_error = None;
    state.session_state.facts.last_completed_action = Some(observation.action_name.clone());

    match action {
        AgentAction::RetrieveContext { .. } => {
            set_session_phase(&mut state.session_state, AgentPhase::Retrieving);
            sync_facts_from_runtime_state(state);
        }
        AgentAction::ReadFiles { files, .. } => {
            set_session_phase(&mut state.session_state, AgentPhase::Reading);
            for file in files {
                push_unique(
                    &mut state.session_state.facts.verified_sources,
                    file.to_string(),
                );
            }
            update_session_timestamp(&mut state.session_state, true);
        }
        AgentAction::RunCommand { command, .. } => {
            set_session_phase(&mut state.session_state, AgentPhase::Verifying);
            record_verification_evidence(
                &mut state.session_state.facts,
                format!("run_command: {}", command),
            );
            update_session_timestamp(&mut state.session_state, true);
        }
        AgentAction::FetchUrl { url, .. } => {
            set_session_phase(&mut state.session_state, AgentPhase::Verifying);
            push_unique(
                &mut state.session_state.facts.verified_sources,
                url.to_string(),
            );
            record_verification_evidence(
                &mut state.session_state.facts,
                format!("fetch_url: {}", url),
            );
            update_session_timestamp(&mut state.session_state, true);
        }
        AgentAction::VerifyChecks { checks, .. } => {
            set_session_phase(&mut state.session_state, AgentPhase::Verifying);
            for check in checks {
                record_verification_evidence(
                    &mut state.session_state.facts,
                    format!("verify_checks [{}]: {}", check.label, check.command),
                );
            }
            update_session_timestamp(&mut state.session_state, true);
        }
        AgentAction::SummarizeFindings { .. } => {
            set_session_phase(&mut state.session_state, AgentPhase::Synthesizing);
        }
        AgentAction::CreateFiles { .. }
        | AgentAction::RenameFiles { .. }
        | AgentAction::DeleteFiles { .. }
        | AgentAction::ApplyPatch { .. }
        | AgentAction::WriteFiles { .. } => {
            if let Some(file_operations) = observation.file_operations.as_ref() {
                state.session_state.facts.applied_file_operations = file_operations.clone();
                update_session_timestamp(&mut state.session_state, true);
            }
        }
        AgentAction::CallMcpTool { server, tool, .. } => {
            push_unique(
                &mut state.session_state.facts.verification_evidence,
                format!("mcp_tool: {}.{}", server, tool),
            );
            update_session_timestamp(&mut state.session_state, true);
        }
        AgentAction::AskUser { question, .. } => {
            state.session_state.facts.last_confirmation_request = Some(question.clone());
            set_session_phase(&mut state.session_state, AgentPhase::AwaitingConfirmation);
            update_session_timestamp(&mut state.session_state, true);
        }
        AgentAction::Finish { .. } => {}
    }
}

fn summarize_session_state_for_planner(state: &AgentSessionStateMachine) -> String {
    let mut lines = vec![
        format!(
            "当前阶段: {}",
            serde_json::to_string(&state.phase)
                .unwrap_or_else(|_| "\"idle\"".into())
                .trim_matches('"')
        ),
        format!("状态版本: {}", state.version),
    ];

    if !state.facts.verified_sources.is_empty() {
        lines.push(format!(
            "已验证来源: {}",
            state.facts
                .verified_sources
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !state.facts.retrieved_refs.is_empty() {
        lines.push(format!(
            "已检索引用: {}",
            state.facts
                .retrieved_refs
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !state.facts.verification_evidence.is_empty() {
        lines.push(format!(
            "已完成验证: {}",
            state.facts
                .verification_evidence
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    if !state.facts.knowledge_gaps.is_empty() {
        lines.push(format!(
            "已知信息缺口: {}",
            state.facts
                .knowledge_gaps
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(last_error) = state.facts.last_error.as_ref() {
        lines.push(format!("最近错误: {}", last_error));
    }
    if let Some(last_confirmation_request) = state.facts.last_confirmation_request.as_ref() {
        lines.push(format!("最近待确认项: {}", last_confirmation_request));
    }

    lines.join("\n")
}

fn build_terminal_response(
    goal_status: GoalStatus,
    summary: String,
    verification: String,
    what_changed: Option<Vec<String>>,
    next_step: Option<String>,
    llm_debug_responses: Vec<LlmDebugResponse>,
    state: &AgentTaskState,
) -> AgentRunResponse {
    let mut lines = vec![
        summary.clone(),
        String::new(),
        format!("验证: {}", verification),
    ];
    if let Some(what_changed) = what_changed.filter(|items| !items.is_empty()) {
        lines.push("变更:".into());
        lines.extend(what_changed.into_iter().map(|item| format!("- {}", item)));
    }
    if let Some(next_step) = next_step.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("下一步: {}", next_step));
    }

    AgentRunResponse {
        final_response: lines.join("\n"),
        goal_status: goal_status_label(&goal_status).to_string(),
        steps: state.observations.clone(),
        llm_debug_responses,
        session_state: state.session_state.clone(),
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
        build_diff_preview, build_terminal_response, goal_status_label, parse_collected_refs,
        resolve_retrieval_sources, should_stop, summarize_patch_requests,
        summarize_rename_requests, summarize_session_state_for_planner, summarize_write_requests,
        update_plan_summary,
    };
    use crate::agent::actions::{
        AgentAction, ContextSource, GoalStatus, PatchFileRequest, PatchHunkRequest,
        RenameFileRequest, RetrievalIntent, SummaryFormat, WriteFileRequest,
    };
    use crate::agent::state::{
        AgentLoopStatus, AgentSessionStateMachine, AgentTaskState, WebCapabilityStatus,
    };
    use std::collections::BTreeMap;

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
    fn patch_summary_includes_paths_and_hunk_count() {
        let summary = summarize_patch_requests(&[PatchFileRequest {
            path: "src/App.tsx".into(),
            hunks: vec![PatchHunkRequest {
                old_text: "old".into(),
                new_text: "new".into(),
                occurrence: None,
            }],
        }]);
        assert!(summary.contains("src/App.tsx"));
        assert!(summary.contains("1 hunks"));
    }

    #[test]
    fn rename_summary_includes_from_and_to_paths() {
        let summary = summarize_rename_requests(&[RenameFileRequest {
            from_path: "src/old.ts".into(),
            to_path: "src/new.ts".into(),
        }]);
        assert!(summary.contains("src/old.ts"));
        assert!(summary.contains("src/new.ts"));
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
    fn terminal_response_omits_context_hit_urls_from_primary_result() {
        let state = AgentTaskState {
            goal: "research".into(),
            workspace_root: ".".into(),
            step_index: 1,
            max_steps: 12,
            status: AgentLoopStatus::Completed,
            plan_summary: None,
            collected_context_refs: vec![
                "https://example.com/article".into(),
                "https://example.com/strategy".into(),
            ],
            knowledge_gaps: Vec::new(),
            report_draft: None,
            source_usage: BTreeMap::new(),
            web_capability: WebCapabilityStatus::Unavailable,
            session_state: AgentSessionStateMachine::new("test-conversation".into()),
            observations: Vec::new(),
        };

        let response = build_terminal_response(
            GoalStatus::Done,
            "已完成结果整理".into(),
            "已联网检索并交叉核对".into(),
            None,
            Some("继续观察市场波动".into()),
            Vec::new(),
            &state,
        );

        assert!(!response.final_response.contains("上下文命中"));
        assert!(!response.final_response.contains("https://example.com/article"));
        assert!(response.final_response.contains("验证: 已联网检索并交叉核对"));
    }

    #[test]
    fn terminal_response_omits_report_draft_from_primary_result() {
        let state = AgentTaskState {
            goal: "research".into(),
            workspace_root: ".".into(),
            step_index: 1,
            max_steps: 12,
            status: AgentLoopStatus::Completed,
            plan_summary: None,
            collected_context_refs: Vec::new(),
            knowledge_gaps: Vec::new(),
            report_draft: Some(
                "聚焦主题: Snowflake公司调研\n\n- [retrieve_context] 命中 8 条上下文结果\n  结果: web_search | https://example.com/article".into(),
            ),
            source_usage: BTreeMap::new(),
            web_capability: WebCapabilityStatus::Unavailable,
            session_state: AgentSessionStateMachine::new("test-conversation".into()),
            observations: Vec::new(),
        };

        let response = build_terminal_response(
            GoalStatus::Done,
            "## Snowflake公司调研报告".into(),
            "通过网络搜索交叉验证".into(),
            None,
            Some("无后续步骤，目标已完成。".into()),
            Vec::new(),
            &state,
        );

        assert!(!response.final_response.contains("整理结果:"));
        assert!(!response.final_response.contains("[retrieve_context]"));
        assert!(!response.final_response.contains("web_search | https://example.com/article"));
        assert!(response.final_response.contains("## Snowflake公司调研报告"));
        assert!(response.final_response.contains("验证: 通过网络搜索交叉验证"));
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

    #[test]
    fn retrieval_sources_fall_back_to_allowed_project_sources() {
        let resolved = resolve_retrieval_sources(
            None,
            &[ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs],
        );
        assert_eq!(
            resolved,
            vec![ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs]
        );
    }

    #[test]
    fn retrieval_sources_filter_out_disallowed_web_search() {
        let resolved = resolve_retrieval_sources(
            Some(&[ContextSource::WebSearch, ContextSource::WorkspaceDocs]),
            &[ContextSource::WorkspaceDocs, ContextSource::UserKnowledgeBase],
        );
        assert_eq!(resolved, vec![ContextSource::WorkspaceDocs]);
    }

    #[test]
    fn session_state_summary_includes_phase_sources_and_gaps() {
        let mut state = AgentSessionStateMachine::new("conv-1".into());
        state.phase = crate::agent::state::AgentPhase::Verifying;
        state.facts.verified_sources = vec!["docs/agent.md".into()];
        state.facts.knowledge_gaps = vec!["web_search unavailable".into()];

        let summary = summarize_session_state_for_planner(&state);

        assert!(summary.contains("当前阶段: verifying"));
        assert!(summary.contains("已验证来源: docs/agent.md"));
        assert!(summary.contains("已知信息缺口: web_search unavailable"));
    }
}
