use serde::Serialize;
use serde_json::json;

use crate::agent::actions::AgentAction;
use crate::agent::state::AgentObservation;
use crate::api::{self, ApiConfig};

#[derive(Debug, Clone, Serialize)]
pub struct PlannerRequest<'a> {
    pub goal: &'a str,
    pub conversation_context: Option<&'a str>,
    pub workspace_root: &'a str,
    pub project_instructions: Option<&'a str>,
    pub workspace_listing: &'a str,
    pub plan_summary: Option<&'a str>,
    pub observations: &'a [AgentObservation],
    pub max_steps: usize,
    pub remaining_steps: usize,
}

pub async fn plan_next_action(
    config: &ApiConfig,
    request: &PlannerRequest<'_>,
) -> Result<AgentAction, String> {
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

    let content = api::chat_completion_with_options(config, messages, false).await?;
    let raw = api::extract_text(&content);
    parse_planner_action(&raw)
}

fn build_planner_system_prompt() -> String {
    [
        "你是 DeepSeekX 应用内 agent planner。",
        "你的任务不是直接回复用户，而是只输出一个 JSON 动作。",
        "硬性规则：",
        "1. 只能输出 JSON，对象最外层必须包含 action 和 reason。",
        "2. 每轮只允许一个动作。",
        "3. 不能输出 markdown、代码块、解释文本。",
        "4. 只允许以下 action：retrieve_context, read_files, write_files, run_command, summarize_findings, ask_user, finish。",
        "5. 当需要高风险 shell、网络副作用、删除文件、全局安装依赖、git push 或超出权限的操作时，必须输出 ask_user。",
        "6. 当目标已足够完成时，必须输出 finish。",
        "7. write_files 只能写相对路径；run_command 的 cwd 只能是工作区内目录，优先使用 . 或相对路径。",
        "8. ask_user 必须包含 question，question 是要直接展示给用户的提问句。",
        "9. finish 必须包含 goal_status, summary, verification，可选 what_changed, next_step。",
        "10. 如果当前信息不足，优先 retrieve_context 或 read_files，而不是猜测。",
        "11. retrieve_context 负责找值得看的代码或文档；read_files 负责把文件读明白。",
        "12. summarize_findings 用于把 observation 整理为阶段性报告或变更说明，不等同于 finish。",
        "13. 优先以目标达成为准，不要求必须测试全绿，但 verification 要写清楚实际验证证据。",
        "14. 默认尽量自主执行，不要因为轻微不确定就 ask_user。能通过 retrieve_context、read_files、run_command 自己确认的，就继续执行。",
        "15. ask_user 只用于三类情况：用户明确偏好缺失且无法合理推断；权限门需要确认；操作存在明显外部副作用或高风险不可逆后果。",
        "动作字段要求：",
        "- retrieve_context: action, reason, query, intent，可选 preferred_sources, max_results",
        "- read_files: action, reason, files",
        "- write_files: action, reason, files",
        "- run_command: action, reason, command，可选 cwd",
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
                    serde_json::to_string(&observation.status).unwrap_or_else(|_| "\"unknown\"".into()),
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
        "用户目标:\n{}\n\n最近对话上下文:\n{}\n\n项目工作区:\n{}\n\n项目指令:\n{}\n\n当前计划摘要:\n{}\n\n工作区快照:\n{}\n\n最近 observation:\n{}\n\n执行预算:\n- 最大步数: {}\n- 剩余步数: {}\n\n请基于上述信息只输出下一步 JSON 动作。",
        request.goal,
        request
            .conversation_context
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("(无历史对话上下文)"),
        request.workspace_root,
        instructions,
        request.plan_summary.unwrap_or("(尚未建立计划摘要)"),
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
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("invalid planner json: {}", e))?;
    let value = normalize_planner_action(value);

    let action = value
        .get("action")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "missing action".to_string())?
        .to_string();

    serde_json::from_value(value).map_err(|e| {
        if e.to_string().contains("unknown variant") {
            format!("unsupported action: {action}")
        } else {
            format!("invalid planner json: {}", e)
        }
    })
}

fn normalize_planner_action(mut value: serde_json::Value) -> serde_json::Value {
    let Some(object) = value.as_object_mut() else {
        return value;
    };

    let action = object.get("action").and_then(|value| value.as_str());
    if action == Some("ask_user") && !object.contains_key("question") {
        if let Some(reason) = object.get("reason").and_then(|value| value.as_str()) {
            object.insert("question".into(), serde_json::Value::String(reason.to_string()));
        } else {
            object.insert(
                "question".into(),
                serde_json::Value::String("需要更多信息或确认后才能继续，是否补充说明？".into()),
            );
        }
    }

    value
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
            AgentAction::RetrieveContext { query, max_results, .. } => {
                assert_eq!(query, "agent orchestrator planner");
                assert_eq!(max_results, Some(8));
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
        }];

        let prompt = build_planner_user_prompt(&PlannerRequest {
            goal: "完成 agent loop",
            conversation_context: Some("用户先要求完成 docs 里的功能，再要求检查运行结果"),
            workspace_root: "/tmp/demo",
            project_instructions: Some("优先保持现有 UI 结构"),
            workspace_listing: "src/\nsrc/App.tsx",
            plan_summary: Some("先看入口，再改后端和前端"),
            observations: &observations,
            max_steps: 12,
            remaining_steps: 11,
        });

        assert!(prompt.contains("完成 agent loop"));
        assert!(prompt.contains("最近对话上下文"));
        assert!(prompt.contains("用户先要求完成 docs 里的功能"));
        assert!(prompt.contains("read_files"));
        assert!(prompt.contains("先看入口，再改后端和前端"));
    }
}
