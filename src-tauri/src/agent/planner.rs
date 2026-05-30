use serde::Serialize;
use serde_json::json;

use crate::agent::actions::AgentAction;
use crate::agent::state::AgentObservation;
use crate::api::{self, ApiConfig, LlmDebugResponse};

#[derive(Debug, Clone, Serialize)]
pub struct PlannerRequest<'a> {
    pub goal: &'a str,
    pub conversation_context: Option<&'a str>,
    pub workspace_root: &'a str,
    pub project_instructions: Option<&'a str>,
    pub workspace_listing: &'a str,
    pub plan_summary: Option<&'a str>,
    pub session_state_summary: Option<&'a str>,
    pub observations: &'a [AgentObservation],
    pub available_mcp_tools: Option<&'a str>,
    pub max_steps: usize,
    pub remaining_steps: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannerDecision {
    pub action: AgentAction,
    pub llm_debug_response: LlmDebugResponse,
}

#[derive(Debug, Clone)]
pub struct PlannerError {
    pub message: String,
    pub llm_debug_response: Option<LlmDebugResponse>,
}

pub async fn plan_next_action(
    config: &ApiConfig,
    request: &PlannerRequest<'_>,
) -> Result<AgentAction, String> {
    plan_next_action_with_debug(config, request)
        .await
        .map(|decision| decision.action)
        .map_err(|error| error.message)
}

pub async fn plan_next_action_with_debug(
    config: &ApiConfig,
    request: &PlannerRequest<'_>,
) -> Result<PlannerDecision, PlannerError> {
    let messages = vec![
        json!({
            "role": "system",
            "content": build_planner_system_prompt(),
        }),
        json!({
            "role": "user",
            "content": build_planner_user_prompt(request),
        }),
    ];

    let response = api::chat_completion_with_debug(config, messages, false)
        .await
        .map_err(|message| PlannerError {
            message,
            llm_debug_response: None,
        })?;
    let raw = api::extract_text(&response.content);
    let action = parse_planner_action(&raw).map_err(|message| PlannerError {
        message,
        llm_debug_response: Some(response.debug.clone()),
    })?;

    Ok(PlannerDecision {
        action,
        llm_debug_response: response.debug,
    })
}

fn build_planner_system_prompt() -> String {
    [
        "你是 DeepSeekX 应用内 agent planner。",
        "你的任务不是直接回复用户，而是只输出一个 JSON 动作。",
        "硬性规则：",
        "1. 只能输出 JSON，对象最外层必须包含 action 和 reason。",
        "2. 每轮只允许一个动作。",
        "3. 不能输出 markdown、代码块、解释文本。",
        "4. 只允许以下 action：retrieve_context, read_files, create_files, rename_files, delete_files, apply_patch, write_files, run_command, fetch_url, verify_checks, call_mcp_tool, summarize_findings, ask_user, finish。",
        "5. 当需要高风险 shell、网络副作用、删除文件、全局安装依赖、git push 或超出权限的操作时，必须输出 ask_user。",
        "6. 当目标已足够完成时，必须输出 finish。",
        "7. create_files、rename_files、delete_files、apply_patch 和 write_files 只能使用相对路径；run_command、verify_checks 的 cwd 只能是工作区内目录，优先使用 . 或相对路径。",
        "8. ask_user 必须包含 question，question 是要直接展示给用户的提问句。",
        "9. finish 必须包含 goal_status, summary, verification，可选 what_changed, next_step。",
        "10. 如果当前信息不足，优先 retrieve_context 或 read_files，而不是猜测。",
        "11. retrieve_context 负责找值得看的代码或文档；read_files 负责把文件读明白。",
        "12. summarize_findings 用于把 observation 整理为阶段性报告或变更说明，不等同于 finish。",
        "13. 优先以目标达成为准，不要求必须测试全绿，但 verification 要写清楚实际验证证据。",
        "14. 默认尽量自主执行，不要因为轻微不确定就 ask_user。能通过 retrieve_context、read_files、run_command、fetch_url、verify_checks 自己确认的，就继续执行。",
        "15. ask_user 只用于三类情况：用户明确偏好缺失且无法合理推断；权限门需要确认；操作存在明显外部副作用或高风险不可逆后果。",
        "16. 修改现有文件里的小范围文本时优先使用 apply_patch；新建文件时使用 create_files；重命名使用 rename_files；删除使用 delete_files；只有需要整文件覆盖现有内容时才用 write_files。",
        "17. 读取外部网页或 API 时优先使用 fetch_url，不要把外部 curl/wget 放进 run_command；run_command 更适合工作区内命令和本地进程检查。",
        "18. 需要验证 build/test/lint 等结果时，优先使用 verify_checks，一次性给出多个检查项，而不是连续输出多个 run_command。",
        "动作字段要求：",
        "- retrieve_context: action, reason, query, intent，可选 preferred_sources, max_results",
        "- read_files: action, reason, files",
        "- create_files: action, reason, files",
        "- rename_files: action, reason, renames（每项包含 from_path, to_path）",
        "- delete_files: action, reason, paths",
        "- apply_patch: action, reason, patches（每个 patch 包含 path 和 hunks；每个 hunk 包含 old_text, new_text，可选 occurrence）",
        "- write_files: action, reason, files",
        "- run_command: action, reason, command，可选 cwd",
        "- fetch_url: action, reason, url, method，可选 max_chars",
        "- verify_checks: action, reason, checks（每项包含 label, command，可选 cwd）",
        "- call_mcp_tool: action, reason, server, tool, arguments",
        "- summarize_findings: action, reason, focus, output_format",
        "- ask_user: action, reason, question",
        "- finish: action, reason, goal_status, summary, verification，可选 what_changed, next_step",
    ]
    .join("\n")
}

fn build_planner_user_prompt(request: &PlannerRequest<'_>) -> String {
    let observations = if request.observations.is_empty() {
        "暂无 observation".to_string()
    } else {
        request
            .observations
            .iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|observation| {
                format!(
                    "- {} | status={} | reason={} | input={} | result={}",
                    observation.action_name,
                    serde_json::to_string(&observation.status)
                        .unwrap_or_else(|_| "\"unknown\"".into()),
                    observation.reason,
                    observation.input_summary,
                    observation.result_summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let instructions = request
        .project_instructions
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("(无额外项目指令)");

    format!(
        "用户目标:\n{}\n\n最近对话上下文:\n{}\n\n项目工作区:\n{}\n\n项目指令:\n{}\n\n可用 MCP 工具:\n{}\n\n当前计划摘要:\n{}\n\n会话状态机摘要:\n{}\n\n工作区快照:\n{}\n\n最近 observation:\n{}\n\n执行预算:\n- 最大步数: {}\n- 剩余步数: {}\n\n请基于上述信息只输出下一步 JSON 动作。",
        request.goal,
        request
            .conversation_context
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("(无历史对话上下文)"),
        request.workspace_root,
        instructions,
        request
            .available_mcp_tools
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("(当前没有可调用的 MCP 工具)"),
        request.plan_summary.unwrap_or("(尚未建立计划摘要)"),
        request
            .session_state_summary
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("(暂无会话状态机摘要)"),
        if request.workspace_listing.trim().is_empty() {
            "(工作区快照为空)".to_string()
        } else {
            request.workspace_listing.to_string()
        },
        observations,
        request.max_steps,
        request.remaining_steps,
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_planner_action(raw: &str) -> Result<AgentAction, String> {
    let raw = extract_json_object(raw).ok_or_else(|| {
        "invalid planner json: could not find a top-level JSON object in planner output"
            .to_string()
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("invalid planner json: {}", e))?;
    let value = normalize_planner_action(value);

    let action = value
        .get("action")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "missing action".to_string())?
        .to_string();

    if !matches!(
        action.as_str(),
        "retrieve_context"
            | "read_files"
            | "create_files"
            | "rename_files"
            | "delete_files"
            | "apply_patch"
            | "write_files"
            | "run_command"
            | "fetch_url"
            | "verify_checks"
            | "call_mcp_tool"
            | "summarize_findings"
            | "ask_user"
            | "finish"
    ) {
        return Err(format!("unsupported action: {action}"));
    }

    serde_json::from_value(value).map_err(|e| format!("invalid planner json: {}", e))
}

fn normalize_planner_action(mut value: serde_json::Value) -> serde_json::Value {
    let Some(object) = value.as_object_mut() else {
        return value;
    };

    let original_action = object.get("action").and_then(|value| value.as_str());
    if let Some(action) = original_action {
        object.insert(
            "action".into(),
            serde_json::Value::String(normalize_token(action)),
        );
    }

    let action = object
        .get("action")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    if action.as_deref() == Some("ask_user") && !object.contains_key("question") {
        if let Some(reason) = object.get("reason").and_then(|value| value.as_str()) {
            object.insert(
                "question".into(),
                serde_json::Value::String(reason.to_string()),
            );
        } else {
            object.insert(
                "question".into(),
                serde_json::Value::String("需要更多信息或确认后才能继续，是否补充说明？".into()),
            );
        }
    }

    if action.as_deref() == Some("fetch_url") {
        normalize_string_field(object, "method");
    }

    if action.as_deref() == Some("retrieve_context") {
        let raw_intent = object
            .get("intent")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        normalize_string_field(object, "intent");
        if let Some(preferred_sources) = object
            .get_mut("preferred_sources")
            .and_then(|value| value.as_array_mut())
        {
            for source in preferred_sources.iter_mut() {
                if let Some(name) = source.as_str() {
                    *source = serde_json::Value::String(normalize_token(name));
                }
            }
        }
        repair_retrieve_context_fields(object, raw_intent);
    }

    if action.as_deref() == Some("summarize_findings") {
        normalize_summary_format_field(object, "output_format");
    }

    if action.as_deref() == Some("finish") {
        normalize_goal_status_field(object, "goal_status");
        normalize_optional_string_list_field(object, "what_changed");
    }

    value
}

fn normalize_string_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) {
    if let Some(value) = object.get(field).and_then(|value| value.as_str()) {
        object.insert(
            field.into(),
            serde_json::Value::String(normalize_token(value)),
        );
    }
}

fn normalize_goal_status_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) {
    if let Some(value) = object.get(field).and_then(|value| value.as_str()) {
        let normalized_value = normalize_token(value);
        let normalized = match normalized_value.as_str() {
            "achieved" | "completed" | "complete" | "success" | "succeeded" => "done",
            "needs_confirmation" | "need_confirmation" | "awaiting_confirmation" => {
                "needs_confirmation"
            }
            other => other,
        };
        object.insert(
            field.into(),
            serde_json::Value::String(normalized.to_string()),
        );
    }
}

fn normalize_summary_format_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) {
    if let Some(value) = object.get(field).and_then(|value| value.as_str()) {
        let normalized_value = normalize_token(value);
        let normalized = match normalized_value.as_str() {
            "markdown" | "structured_report" | "structured_brief" | "report" | "summary" => {
                "research_brief"
            }
            "bullet" | "bullets" | "bullet_list" => "bullet_report",
            "changes" | "change_report" | "diff_summary" => "change_summary",
            other if is_known_summary_format(other) => other,
            _ => infer_summary_format(value),
        };
        object.insert(
            field.into(),
            serde_json::Value::String(normalized.to_string()),
        );
    }
}

fn normalize_optional_string_list_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) {
    let Some(value) = object.get(field).cloned() else {
        return;
    };

    match value {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            let items = if trimmed.is_empty()
                || trimmed.contains("无文件变更")
                || trimmed.contains("无代码变更")
                || trimmed.contains("无变更")
            {
                Vec::new()
            } else {
                vec![trimmed.to_string()]
            };
            object.insert(
                field.into(),
                serde_json::Value::Array(
                    items
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect::<Vec<_>>(),
                ),
            );
        }
        serde_json::Value::Null => {
            object.remove(field);
        }
        _ => {}
    }
}

fn is_known_summary_format(value: &str) -> bool {
    matches!(value, "bullet_report" | "change_summary" | "research_brief")
}

fn infer_summary_format(text: &str) -> &'static str {
    let normalized = text.to_lowercase();
    if normalized.contains("变更")
        || normalized.contains("diff")
        || normalized.contains("patch")
        || normalized.contains("修改")
        || normalized.contains("change")
    {
        "change_summary"
    } else if normalized.contains("结构化")
        || normalized.contains("汇报")
        || normalized.contains("报告")
        || normalized.contains("research")
        || normalized.contains("brief")
        || normalized.contains("业务概述")
        || normalized.contains("盈利模式")
        || normalized.contains("路线图")
    {
        "research_brief"
    } else if normalized.contains("bullet")
        || normalized.contains("bullets")
        || normalized.contains("list")
        || normalized.contains("列表")
        || normalized.contains("要点")
        || normalized.contains("分点")
        || normalized.contains("条目")
    {
        "bullet_report"
    } else {
        "research_brief"
    }
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .replace('-', "_")
        .chars()
        .map(|ch| if ch.is_ascii_uppercase() { ch.to_ascii_lowercase() } else { ch })
        .collect()
}

fn extract_json_object(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }

    let mut depth = 0usize;
    let mut start = None;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in raw.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = start {
                        return Some(raw[start..=idx].trim().to_string());
                    }
                }
            }
            _ => {}
        }
    }

    None
}

fn repair_retrieve_context_fields(
    object: &mut serde_json::Map<String, serde_json::Value>,
    raw_intent: Option<String>,
) {
    let current_query = object
        .get("query")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let current_intent = object
        .get("intent")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    let canonical_intent = if is_known_retrieval_intent(&current_intent) {
        current_intent
    } else {
        let fallback_text = raw_intent
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&current_query);
        infer_retrieval_intent(fallback_text).to_string()
    };
    object.insert(
        "intent".into(),
        serde_json::Value::String(canonical_intent),
    );

    if current_query.is_empty() {
        if let Some(raw_intent) = raw_intent {
            if !raw_intent.is_empty() && !is_known_retrieval_intent(&normalize_token(&raw_intent)) {
                object.insert("query".into(), serde_json::Value::String(raw_intent));
            }
        }
    }
}

fn is_known_retrieval_intent(value: &str) -> bool {
    matches!(
        value,
        "understand_existing_system"
            | "locate_implementation"
            | "find_documentation"
            | "gather_evidence"
            | "prepare_report"
    )
}

fn infer_retrieval_intent(text: &str) -> &'static str {
    let normalized = text.to_lowercase();
    if normalized.contains("readme")
        || normalized.contains("docs")
        || normalized.contains("documentation")
        || normalized.contains("文档")
        || normalized.contains("说明")
    {
        "find_documentation"
    } else if normalized.contains("实现")
        || normalized.contains("代码")
        || normalized.contains("组件")
        || normalized.contains("函数")
        || normalized.contains("模块")
        || normalized.contains("implementation")
        || normalized.contains("source")
    {
        "locate_implementation"
    } else if normalized.contains("报告")
        || normalized.contains("总结")
        || normalized.contains("summary")
        || normalized.contains("report")
    {
        "prepare_report"
    } else if normalized.contains("架构")
        || normalized.contains("流程")
        || normalized.contains("整体")
        || normalized.contains("system")
        || normalized.contains("orchestrator")
    {
        "understand_existing_system"
    } else {
        "gather_evidence"
    }
}

#[cfg(test)]
mod tests {
    use super::{build_planner_user_prompt, parse_planner_action, PlannerRequest};
    use crate::agent::actions::{AgentAction, GoalStatus};
    use crate::agent::state::{AgentObservation, AgentStepStatus};

    #[test]
    fn parses_read_files_action() {
        let raw = r#"{
            "action": "read_files",
            "reason": "inspect current ui flow",
            "files": ["src/App.tsx", "src/components/ChatInput.tsx"]
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::ReadFiles { reason, files } => {
                assert_eq!(reason, "inspect current ui flow");
                assert_eq!(files.len(), 2);
                assert_eq!(files[0], "src/App.tsx");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_retrieve_context_action() {
        let raw = r#"{
            "action": "retrieve_context",
            "reason": "需要先理解现有 agent 相关实现",
            "query": "agent orchestrator planner",
            "intent": "understand_existing_system",
            "preferred_sources": ["workspace_code", "workspace_docs"],
            "max_results": 8
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::RetrieveContext {
                query, max_results, ..
            } => {
                assert_eq!(query, "agent orchestrator planner");
                assert_eq!(max_results, Some(8));
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_apply_patch_action() {
        let raw = r#"{
            "action": "apply_patch",
            "reason": "小范围修改现有按钮文案",
            "patches": [{
                "path": "src/App.tsx",
                "hunks": [{
                    "old_text": "旧文案",
                    "new_text": "新文案",
                    "occurrence": 1
                }]
            }]
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::ApplyPatch { reason, patches } => {
                assert_eq!(reason, "小范围修改现有按钮文案");
                assert_eq!(patches.len(), 1);
                assert_eq!(patches[0].path, "src/App.tsx");
                assert_eq!(patches[0].hunks[0].old_text, "旧文案");
                assert_eq!(patches[0].hunks[0].new_text, "新文案");
                assert_eq!(patches[0].hunks[0].occurrence, Some(1));
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_rename_files_action() {
        let raw = r#"{
            "action": "rename_files",
            "reason": "整理文件命名",
            "renames": [{
                "from_path": "src/old.ts",
                "to_path": "src/new.ts"
            }]
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::RenameFiles { reason, renames } => {
                assert_eq!(reason, "整理文件命名");
                assert_eq!(renames.len(), 1);
                assert_eq!(renames[0].from_path, "src/old.ts");
                assert_eq!(renames[0].to_path, "src/new.ts");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn rejects_unknown_action() {
        let raw = r#"{"action":"shell","reason":"nope"}"#;
        let err = parse_planner_action(raw).unwrap_err();
        assert!(err.contains("unsupported action"));
    }

    #[test]
    fn parses_finish_action() {
        let raw = r#"{
            "action": "finish",
            "reason": "task complete",
            "goal_status": "done",
            "summary": "implemented planner parser",
            "verification": "cargo test",
            "what_changed": ["src-tauri/src/agent/planner.rs"],
            "next_step": null
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::Finish {
                reason,
                goal_status,
                summary,
                verification,
                what_changed,
                next_step,
            } => {
                assert_eq!(reason, "task complete");
                assert_eq!(goal_status, GoalStatus::Done);
                assert_eq!(summary, "implemented planner parser");
                assert_eq!(verification, "cargo test");
                assert_eq!(
                    what_changed.unwrap(),
                    vec!["src-tauri/src/agent/planner.rs".to_string()]
                );
                assert_eq!(next_step, None);
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn fills_question_for_ask_user_when_missing() {
        let raw = r#"{
            "action": "ask_user",
            "reason": "请确认是否允许我运行带副作用的网络命令"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::AskUser { reason, question } => {
                assert_eq!(reason, "请确认是否允许我运行带副作用的网络命令");
                assert_eq!(question, reason);
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_fetch_url_action() {
        let raw = r#"{
            "action": "fetch_url",
            "reason": "check the latest docs page",
            "url": "https://example.com/docs",
            "method": "get",
            "max_chars": 2000
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::FetchUrl {
                reason,
                url,
                method,
                max_chars,
            } => {
                assert_eq!(reason, "check the latest docs page");
                assert_eq!(url, "https://example.com/docs");
                assert_eq!(format!("{:?}", method), "Get");
                assert_eq!(max_chars, Some(2000));
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_fetch_url_action_from_code_fence_and_uppercase_method() {
        let raw = r#"```json
        {
            "action": "fetch_url",
            "reason": "check the latest docs page",
            "url": "https://example.com/docs",
            "method": "GET",
            "max_chars": 2000
        }
        ```"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::FetchUrl { method, .. } => {
                assert_eq!(format!("{:?}", method), "Get");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_finish_action_with_uppercase_goal_status() {
        let raw = r#"{
            "action": "finish",
            "reason": "task complete",
            "goal_status": "DONE",
            "summary": "implemented planner parser",
            "verification": "cargo test"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::Finish { goal_status, .. } => {
                assert_eq!(goal_status, GoalStatus::Done);
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_finish_action_with_achieved_status_and_string_what_changed() {
        let raw = r#"{
            "action": "finish",
            "reason": "task complete",
            "goal_status": "achieved",
            "summary": "implemented planner parser",
            "verification": "cargo test",
            "what_changed": "无文件变更，仅完成信息查询。"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::Finish {
                goal_status,
                what_changed,
                ..
            } => {
                assert_eq!(goal_status, GoalStatus::Done);
                assert_eq!(what_changed, Some(vec![]));
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_summarize_findings_with_markdown_alias() {
        let raw = r#"{
            "action": "summarize_findings",
            "reason": "整理调研结果",
            "focus": "SOXL 价格与策略",
            "output_format": "markdown"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::SummarizeFindings { output_format, .. } => {
                assert_eq!(format!("{:?}", output_format), "ResearchBrief");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_summarize_findings_with_chinese_descriptive_output_format() {
        let raw = r#"{
            "action": "summarize_findings",
            "reason": "整理竞争对手调研",
            "focus": "Snowflake 竞争对手对比",
            "output_format": "结构化汇报，包含：业务概述、盈利模式、关键技术要点、创业路线图建议"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::SummarizeFindings { output_format, .. } => {
                assert_eq!(format!("{:?}", output_format), "ResearchBrief");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_summarize_findings_with_chinese_bullet_output_format() {
        let raw = r#"{
            "action": "summarize_findings",
            "reason": "整理执行结果",
            "focus": "本轮执行要点",
            "output_format": "分点列出关键要点和结论"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::SummarizeFindings { output_format, .. } => {
                assert_eq!(format!("{:?}", output_format), "BulletReport");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_summarize_findings_with_change_description_output_format() {
        let raw = r#"{
            "action": "summarize_findings",
            "reason": "整理代码修改",
            "focus": "补丁改动总结",
            "output_format": "按变更说明总结这次修改"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::SummarizeFindings { output_format, .. } => {
                assert_eq!(format!("{:?}", output_format), "ChangeSummary");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn parses_verify_checks_action() {
        let raw = r#"{
            "action": "verify_checks",
            "reason": "confirm the workspace still builds",
            "checks": [
                {"label": "typecheck", "command": "npm run build", "cwd": "."},
                {"label": "rust tests", "command": "cargo test", "cwd": "src-tauri"}
            ]
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::VerifyChecks { reason, checks } => {
                assert_eq!(reason, "confirm the workspace still builds");
                assert_eq!(checks.len(), 2);
                assert_eq!(checks[0].label, "typecheck");
                assert_eq!(checks[1].command, "cargo test");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn repairs_retrieve_context_with_natural_language_intent() {
        let raw = r#"{
            "action": "retrieve_context",
            "reason": "先收集市场数据",
            "query": "SOXL ETF 当日价格与投资策略",
            "intent": "获取soxl etf最新价格、涨跌幅、技术面及分析师投资策略建议"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::RetrieveContext { query, intent, .. } => {
                assert_eq!(query, "SOXL ETF 当日价格与投资策略");
                assert_eq!(format!("{:?}", intent), "GatherEvidence");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn repairs_retrieve_context_when_query_is_missing_and_intent_contains_query_text() {
        let raw = r#"{
            "action": "retrieve_context",
            "reason": "先收集市场数据",
            "intent": "查找soxl etf当日价格、涨跌数据以及近期投资策略建议"
        }"#;

        let parsed = parse_planner_action(raw).unwrap();
        match parsed {
            AgentAction::RetrieveContext { query, intent, .. } => {
                assert_eq!(query, "查找soxl etf当日价格、涨跌数据以及近期投资策略建议");
                assert_eq!(format!("{:?}", intent), "GatherEvidence");
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }

    #[test]
    fn rejects_missing_action_field() {
        let raw = r#"{"reason":"missing action"}"#;
        let err = parse_planner_action(raw).unwrap_err();
        assert!(err.contains("missing action"));
    }

    #[test]
    fn planner_prompt_includes_recent_observations() {
        let observations = vec![AgentObservation {
            action_name: "read_files".into(),
            reason: "inspect".into(),
            input_summary: "src/App.tsx".into(),
            result_summary: "read ok".into(),
            summary: "read ok".into(),
            status: AgentStepStatus::Completed,
            is_error: false,
            requires_confirmation: false,
            preview_type: None,
            before_preview: None,
            after_preview: None,
            diff_preview: None,
            changed_ranges: None,
            file_operations: None,
        }];

        let prompt = build_planner_user_prompt(&PlannerRequest {
            goal: "完成 agent loop",
            conversation_context: Some("用户先要求完成 docs 里的功能，再要求检查运行结果"),
            workspace_root: "/tmp/demo",
            project_instructions: Some("优先保持现有 UI 结构"),
            workspace_listing: "src/\nsrc/App.tsx",
            plan_summary: Some("先看入口，再改后端和前端"),
            session_state_summary: Some("当前阶段: verifying\n已验证来源: docs/agent.md"),
            observations: &observations,
            available_mcp_tools: Some("filesystem.read_text(server=filesystem)"),
            max_steps: 12,
            remaining_steps: 11,
        });

        assert!(prompt.contains("完成 agent loop"));
        assert!(prompt.contains("最近对话上下文"));
        assert!(prompt.contains("用户先要求完成 docs 里的功能"));
        assert!(prompt.contains("read_files"));
        assert!(prompt.contains("filesystem.read_text"));
        assert!(prompt.contains("先看入口，再改后端和前端"));
        assert!(prompt.contains("会话状态机摘要"));
        assert!(prompt.contains("当前阶段: verifying"));
    }
}
