use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

// ---------- Session 文件结构 ----------

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionFile {
    pub schema_version: Option<i32>,
    pub metadata: SessionMetadata,
    pub system_prompt: Option<String>,
    pub messages: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionMetadata {
    pub id: Option<String>,
    pub title: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub message_count: Option<i64>,
    pub total_tokens: Option<i64>,
    pub model: Option<String>,
    pub workspace: Option<String>,
    pub mode: Option<String>,
}

// ---------- 前端用类型 ----------

#[derive(Debug, Serialize, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub message_count: i64,
    pub workspace: String,
    pub model: String,
    pub session_path: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SessionMessages {
    pub id: String,
    pub title: String,
    pub messages: Vec<Value>,
    pub system_prompt: Option<String>,
}

// ---------- Turn 分组类型 ----------

#[derive(Debug, Serialize, Clone)]
pub struct TurnBasedSession {
    pub id: String,
    pub title: String,
    pub turns: Vec<Turn>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Turn {
    pub id: String,
    pub user_input: String,
    pub thinking_steps: Vec<ThinkingStep>,
    pub final_response: Option<String>,
    pub agent_goal_status: Option<String>,
    pub agent_steps: Option<Vec<AgentStep>>,
}

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Eq)]
pub struct AgentStep {
    pub action_name: String,
    pub reason: Option<String>,
    pub input_summary: Option<String>,
    pub result_summary: Option<String>,
    pub summary: String,
    pub status: Option<String>,
    pub is_error: bool,
    pub requires_confirmation: Option<bool>,
    pub preview_type: Option<String>,
    pub before_preview: Option<String>,
    pub after_preview: Option<String>,
    pub diff_preview: Option<String>,
    pub changed_ranges: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThinkingStep {
    pub thinking: String,
    pub tool_calls: Vec<ToolCallSummary>,
    pub tool_results: Vec<ToolResultSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolCallSummary {
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_input: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolResultSummary {
    pub tool_call_id: String,
    pub is_error: bool,
    pub summary: String,
}

// ---------- Turn 分组算法 ----------

/// 从 session 消息块中提取纯文本用户输入（跳过 `<turn_meta>` 等系统块）
fn extract_user_input(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter_map(|b| {
            if b.get("type")?.as_str()? == "text" {
                let text = b.get("text")?.as_str().unwrap_or("");
                if text.trim_start().starts_with("<turn_meta>") {
                    None
                } else {
                    Some(text.to_string())
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 截断字符串到指定长度（按字符数）
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

/// 截断并格式化工具输入参数
fn summarize_tool_input(input: &Value, max_chars: usize) -> String {
    let s = match input {
        Value::Object(_) => serde_json::to_string(input).unwrap_or_default(),
        Value::String(s) => s.clone(),
        _ => input.to_string(),
    };
    truncate(&s, max_chars)
}

fn parse_agent_steps(value: Option<&Value>) -> Option<Vec<AgentStep>> {
    let steps = value?.as_array()?;
    let parsed: Vec<AgentStep> = steps
        .iter()
        .filter_map(|step| {
            Some(AgentStep {
                action_name: step.get("action_name")?.as_str()?.to_string(),
                reason: step.get("reason").and_then(|v| v.as_str()).map(|v| v.to_string()),
                input_summary: step.get("input_summary").and_then(|v| v.as_str()).map(|v| v.to_string()),
                result_summary: step.get("result_summary").and_then(|v| v.as_str()).map(|v| v.to_string()),
                summary: step.get("summary")?.as_str()?.to_string(),
                status: step.get("status").and_then(|v| v.as_str()).map(|v| v.to_string()),
                is_error: step.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
                requires_confirmation: step
                    .get("requires_confirmation")
                    .and_then(|v| v.as_bool()),
                preview_type: step.get("preview_type").and_then(|v| v.as_str()).map(|v| v.to_string()),
                before_preview: step.get("before_preview").and_then(|v| v.as_str()).map(|v| v.to_string()),
                after_preview: step.get("after_preview").and_then(|v| v.as_str()).map(|v| v.to_string()),
                diff_preview: step.get("diff_preview").and_then(|v| v.as_str()).map(|v| v.to_string()),
                changed_ranges: step.get("changed_ranges").and_then(|v| {
                    v.as_array().map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(|value| value.to_string()))
                            .collect::<Vec<_>>()
                    })
                }),
            })
        })
        .collect();

    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// 将 session 的原始消息分组为 Turn 列表
pub fn group_into_turns(
    session_id: &str,
    messages: &[Value],
) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut current_user_input: Option<String> = None;
    let mut current_steps: Vec<ThinkingStep> = Vec::new();
    let mut current_final: Option<String> = None;
    let mut current_agent_goal_status: Option<String> = None;
    let mut current_agent_steps: Option<Vec<AgentStep>> = None;
    let mut current_thinking: Option<String> = None;
    let mut current_tool_calls: Vec<ToolCallSummary> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let content = msg.get("content");

        // 兼容两种 content 格式：
        // - TUI CLI: [{"type": "text", "text": "..."}, ...]
        // - GUI API: "纯文本字符串"
        let blocks: Vec<&Value> = match content {
            Some(Value::Array(arr)) => arr.iter().collect(),
            _ => Vec::new(),
        };
        let is_plain_text = matches!(content, Some(Value::String(_)));

        let has_tool_result = blocks.iter().any(|b| {
            b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
        });

        if role == "user" && !has_tool_result {
            // 新用户输入 → 提交当前 Turn
            if let Some(user_input) = current_user_input.take() {
                if current_thinking.is_some() || !current_tool_calls.is_empty() {
                    current_steps.push(ThinkingStep {
                        thinking: current_thinking.take().unwrap_or_default(),
                        tool_calls: std::mem::take(&mut current_tool_calls),
                        tool_results: Vec::new(),
                    });
                }
                let turn_id = format!("{}-{}", session_id, turns.len());
                turns.push(Turn {
                    id: turn_id,
                    user_input: user_input.trim().to_string(),
                    thinking_steps: std::mem::take(&mut current_steps),
                    final_response: current_final.take(),
                    agent_goal_status: current_agent_goal_status.take(),
                    agent_steps: current_agent_steps.take(),
                });
            }
            if is_plain_text {
                // GUI API 格式：content 是纯文本字符串
                let text = content.and_then(|v| v.as_str()).unwrap_or("");
                // 跳过系统注入的 turn_meta
                if !text.trim_start().starts_with("<turn_meta>") {
                    current_user_input = Some(text.to_string());
                } else {
                    current_user_input = Some(String::new());
                }
            } else {
                let input_blocks: Vec<Value> = blocks.iter().map(|&v| v.clone()).collect();
                current_user_input = Some(extract_user_input(&input_blocks));
            }
            current_steps = Vec::new();
            current_final = None;
            current_agent_goal_status = None;
            current_agent_steps = None;
            current_thinking = None;
            current_tool_calls = Vec::new();
        } else if role == "assistant" {
            if let Some(goal_status) = msg.get("agent_goal_status").and_then(|v| v.as_str()) {
                current_agent_goal_status = Some(goal_status.to_string());
            }
            if let Some(agent_steps) = parse_agent_steps(msg.get("agent_steps")) {
                current_agent_steps = Some(agent_steps);
            }

            // GUI API 格式：纯文本字符串 → 直接作为 final_response
            if is_plain_text {
                let text = content.and_then(|v| v.as_str()).unwrap_or("");
                if !text.is_empty() {
                    current_final = Some(text.to_string());
                }
                continue; // 跳过 ContentBlock 解析逻辑
            }

            let mut has_thinking = false;
            let mut has_tool_use = false;
            let mut text_content: Option<String> = None;

            for block in &blocks {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "thinking" => {
                        has_thinking = true;
                        let thinking_text = block
                            .get("thinking")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        current_thinking = Some(truncate(thinking_text, 200));
                    }
                    "tool_use" => {
                        has_tool_use = true;
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let input = block.get("input").unwrap_or(&Value::Null);
                        current_tool_calls.push(ToolCallSummary {
                            tool_call_id: id.to_string(),
                            tool_name: name.to_string(),
                            tool_input: summarize_tool_input(input, 100),
                        });
                    }
                    "text" => {
                        text_content = block
                            .get("text")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                    _ => {}
                }
            }

            if has_tool_use {
                if current_thinking.is_some() || !current_tool_calls.is_empty() {
                    current_steps.push(ThinkingStep {
                        thinking: current_thinking.take().unwrap_or_default(),
                        tool_calls: std::mem::take(&mut current_tool_calls),
                        tool_results: Vec::new(),
                    });
                }
                current_thinking = None;
                current_tool_calls = Vec::new();
            } else if has_thinking && !has_tool_use && text_content.is_some() {
                if let Some(th) = current_thinking.take() {
                    current_steps.push(ThinkingStep {
                        thinking: th,
                        tool_calls: Vec::new(),
                        tool_results: Vec::new(),
                    });
                }
                current_final = text_content;
            } else if let Some(tc) = text_content {
                if !has_thinking && !has_tool_use {
                    current_final = Some(tc);
                }
            }
        } else if role == "user" && has_tool_result {
            let tool_results: Vec<ToolResultSummary> = blocks
                .iter()
                .filter_map(|block| {
                    if block.get("type")?.as_str()? != "tool_result" {
                        return None;
                    }
                    let tool_call_id = block
                        .get("tool_use_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let is_error = block
                        .get("is_error")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let raw_content = block
                        .get("content")
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let summary = truncate(&raw_content, 200);
                    Some(ToolResultSummary {
                        tool_call_id,
                        is_error,
                        summary,
                    })
                })
                .collect();

            if let Some(last_step) = current_steps.last_mut() {
                last_step.tool_results.extend(tool_results);
            } else if !tool_results.is_empty() {
                current_steps.push(ThinkingStep {
                    thinking: String::new(),
                    tool_calls: Vec::new(),
                    tool_results,
                });
            }
        }
    }

    // 提交最后一个 Turn
    if let Some(user_input) = current_user_input.take() {
        if current_thinking.is_some() || !current_tool_calls.is_empty() {
            current_steps.push(ThinkingStep {
                thinking: current_thinking.take().unwrap_or_default(),
                tool_calls: std::mem::take(&mut current_tool_calls),
                tool_results: Vec::new(),
            });
        }
        let turn_id = format!("{}-{}", session_id, turns.len());
        turns.push(Turn {
            id: turn_id,
            user_input: user_input.trim().to_string(),
            thinking_steps: current_steps,
            final_response: current_final,
            agent_goal_status: current_agent_goal_status,
            agent_steps: current_agent_steps,
        });
    }

    turns
}

// ---------- Skill / MCP 查询 ----------

/// 返回本地已安装的所有 skill（扫描 ~/.deepseek/skills/ 和 ~/.agents/skills/）
#[derive(Debug, serde::Serialize, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: String,
}

pub fn list_available_skills() -> Vec<SkillInfo> {
    let mut skills: Vec<SkillInfo> = Vec::new();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return skills,
    };
    let dirs = &[
        PathBuf::from(&home).join(".deepseek").join("skills"),
        PathBuf::from(&home).join(".agents").join("skills"),
    ];
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let skill_md = path.join("SKILL.md");
                    let description = if skill_md.exists() {
                        std::fs::read_to_string(&skill_md)
                            .ok()
                            .and_then(|s| {
                                s.lines()
                                    .skip_while(|l| !l.starts_with("- ") && !l.starts_with("> "))
                                    .next()
                                    .map(|l| l.trim_start_matches("- ").trim_start_matches("> ").to_string())
                            })
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    skills.push(SkillInfo {
                        name,
                        description,
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
    skills
}

// ---------- Session 文件路径 ----------

fn sessions_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "无法获取 HOME 目录".to_string())?;
    Ok(PathBuf::from(home).join(".deepseek").join("sessions"))
}

fn session_path(session_id: &str) -> Result<PathBuf, String> {
    // 安全检查：session_id 必须是合法 UUID
    if session_id.contains('/') || session_id.contains("..") {
        return Err("无效的会话 ID".to_string());
    }
    Ok(sessions_dir()?.join(format!("{}.json", session_id)))
}

// ---------- 读取操作 ----------

/// 列出所有 TUI sessions，可按 workspace 过滤
pub fn list_sessions(workspace_filter: Option<&str>) -> Result<Vec<SessionSummary>, String> {
    let dir = sessions_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut summaries: Vec<SessionSummary> = Vec::new();

    for entry in std::fs::read_dir(&dir).map_err(|e| format!("读取 sessions 目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if path.extension().map_or(true, |e| e != "json") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let session: SessionFile = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let workspace = session.metadata.workspace.unwrap_or_default();

        // 按 workspace 过滤
        if let Some(filter) = workspace_filter {
            if workspace != filter {
                continue;
            }
        }

        let id = session.metadata.id.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });

        summaries.push(SessionSummary {
            id,
            title: session.metadata.title.unwrap_or_else(|| "未命名对话".into()),
            message_count: session.metadata.message_count.unwrap_or(0),
            workspace,
            model: session.metadata.model.unwrap_or_else(|| "deepseek-chat".into()),
            session_path: path.to_string_lossy().to_string(),
            updated_at: session.metadata.updated_at.unwrap_or_default(),
        });
    }

    // 按更新时间降序排列
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(summaries)
}

/// 读取指定 session 的所有消息
pub fn read_session(session_id: &str) -> Result<SessionMessages, String> {
    let path = session_path(session_id)?;
    let content =
        std::fs::read_to_string(&path).map_err(|_| format!("会话 {} 不存在", session_id))?;

    let session: SessionFile =
        serde_json::from_str(&content).map_err(|e| format!("解析会话文件失败: {}", e))?;

    let id = session.metadata.id.clone().unwrap_or_else(|| session_id.to_string());

    Ok(SessionMessages {
        id,
        title: session
            .metadata
            .title
            .unwrap_or_else(|| "未命名对话".into()),
        messages: session.messages,
        system_prompt: session.system_prompt,
    })
}

/// 删除指定 session 文件
pub fn delete_session(session_id: &str) -> Result<(), String> {
    let path = session_path(session_id)?;
    std::fs::remove_file(&path).map_err(|e| format!("删除会话文件失败: {}", e))?;
    Ok(())
}

/// 将新的 user + assistant 消息对追加到 session 文件
pub fn append_messages(
    session_id: &str,
    user_msg: Value,
    assistant_msg: Value,
) -> Result<(), String> {
    let path = session_path(session_id)?;

    let content = std::fs::read_to_string(&path)
        .map_err(|_| format!("会话 {} 不存在或无法读取", session_id))?;

    let mut session: SessionFile =
        serde_json::from_str(&content).map_err(|e| format!("解析会话文件失败: {}", e))?;

    session.messages.push(user_msg);
    session.messages.push(assistant_msg);

    // 更新元数据
    if let Some(ref mut meta) = Some(&mut session.metadata) {
        meta.message_count = Some(session.messages.len() as i64);
        meta.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    let updated = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("序列化会话文件失败: {}", e))?;

    std::fs::write(&path, updated).map_err(|e| format!("写入会话文件失败: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        append_messages, create_session, delete_session, group_into_turns, list_sessions,
        read_session,
    };
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn test_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_home<T>(f: impl FnOnce(PathBuf) -> T) -> T {
        let _guard = test_env_lock().lock().unwrap();
        let original_home = std::env::var("HOME").ok();
        let temp_home = std::env::temp_dir().join(format!("deepseekx-session-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_home).unwrap();
        unsafe {
            std::env::set_var("HOME", &temp_home);
        }

        let result = f(temp_home.clone());

        if let Some(home) = original_home {
            unsafe {
                std::env::set_var("HOME", home);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
        let _ = fs::remove_dir_all(temp_home);
        result
    }

    #[test]
    fn preserves_agent_goal_status_and_steps_for_plain_text_assistant_messages() {
        let turns = group_into_turns(
            "session-1",
            &[
                json!({
                    "role": "user",
                    "content": "请帮我完成 Task3"
                }),
                json!({
                    "role": "assistant",
                    "content": "Task3 已完成",
                    "agent_goal_status": "done",
                    "agent_steps": [
                        {
                            "action_name": "agent_plan",
                            "summary": "已规划完成路径",
                            "is_error": false
                        }
                    ]
                }),
            ],
        );

        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].final_response.as_deref(), Some("Task3 已完成"));
        assert_eq!(turns[0].agent_goal_status.as_deref(), Some("done"));
        assert_eq!(
            turns[0].agent_steps.as_ref().map(|steps| steps.len()),
            Some(1)
        );
        assert_eq!(
            turns[0]
                .agent_steps
                .as_ref()
                .and_then(|steps| steps.first())
                .map(|step| step.action_name.as_str()),
            Some("agent_plan")
        );
    }

    #[test]
    fn append_messages_refreshes_updated_at_and_persists_agent_steps() {
        with_temp_home(|home| {
            let session_id = format!("session-{}", Uuid::new_v4());
            create_session(
                &session_id,
                "测试会话",
                "/tmp",
                "agent-loop",
                "Agent mode",
                json!({"role": "user", "content": "hello"}),
                json!({"role": "assistant", "content": "world"}),
            )
            .unwrap();

            let before = read_session(&session_id).unwrap();
            let path = home
                .join(".deepseek")
                .join("sessions")
                .join(format!("{}.json", session_id));
            let before_file = {
                let content = fs::read_to_string(&path).unwrap();
                serde_json::from_str::<super::SessionFile>(&content).unwrap()
            };

            append_messages(
                &session_id,
                json!({"role": "user", "content": "继续执行"}),
                json!({
                    "role": "assistant",
                    "content": "已完成",
                    "agent_goal_status": "done",
                    "agent_steps": [
                        {
                            "action_name": "run_command",
                            "summary": "cargo test",
                            "is_error": false
                        }
                    ]
                }),
            )
            .unwrap();

            let after = read_session(&session_id).unwrap();
            let content = fs::read_to_string(&path).unwrap();
            let after_file = serde_json::from_str::<super::SessionFile>(&content).unwrap();
            let turns = group_into_turns(&session_id, &after.messages);

            assert_eq!(before.messages.len() + 2, after.messages.len());
            assert_ne!(
                before_file.metadata.updated_at,
                after_file.metadata.updated_at
            );
            assert_eq!(after_file.metadata.message_count, Some(after.messages.len() as i64));
            assert_eq!(turns.len(), 2);
            assert_eq!(turns[1].agent_goal_status.as_deref(), Some("done"));
            assert_eq!(
                turns[1]
                    .agent_steps
                    .as_ref()
                    .and_then(|steps| steps.first())
                    .map(|step| step.summary.as_str()),
                Some("cargo test")
            );
        });
    }

    #[test]
    fn list_sessions_includes_session_path() {
        with_temp_home(|home| {
            let session_id = format!("session-{}", Uuid::new_v4());
            create_session(
                &session_id,
                "路径测试",
                "/tmp/workspace",
                "deepseek-chat",
                "system prompt",
                json!({"role": "user", "content": "hello"}),
                json!({"role": "assistant", "content": "world"}),
            )
            .unwrap();

            let summaries = list_sessions(None).unwrap();
            let summary = summaries
                .iter()
                .find(|item| item.id == session_id)
                .expect("summary should exist");

            assert_eq!(
                summary.session_path,
                home.join(".deepseek")
                    .join("sessions")
                    .join(format!("{}.json", session_id))
                    .to_string_lossy()
                    .to_string()
            );
        });
    }

    #[test]
    fn delete_session_removes_file_and_excludes_it_from_listing() {
        with_temp_home(|home| {
            let session_id = format!("session-{}", Uuid::new_v4());
            create_session(
                &session_id,
                "删除测试",
                "/tmp/workspace",
                "deepseek-chat",
                "system prompt",
                json!({"role": "user", "content": "hello"}),
                json!({"role": "assistant", "content": "world"}),
            )
            .unwrap();

            let path = home
                .join(".deepseek")
                .join("sessions")
                .join(format!("{}.json", session_id));
            assert!(path.exists(), "session file should exist before deletion");

            delete_session(&session_id).unwrap();

            assert!(!path.exists(), "session file should be removed from disk");
            assert!(read_session(&session_id).is_err(), "deleted session should not be readable");
            assert!(
                list_sessions(None)
                    .unwrap()
                    .iter()
                    .all(|summary| summary.id != session_id),
                "deleted session should not reappear in session listing"
            );
        });
    }
}

/// 创建新的 session 文件
pub fn create_session(
    session_id: &str,
    title: &str,
    workspace: &str,
    model: &str,
    system_prompt: &str,
    user_msg: Value,
    assistant_msg: Value,
) -> Result<(), String> {
    let path = session_path(session_id)?;

    // 确保 sessions 目录存在
    let dir = sessions_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 sessions 目录失败: {}", e))?;

    let now = chrono::Utc::now().to_rfc3339();

    let session = SessionFile {
        schema_version: Some(1),
        metadata: SessionMetadata {
            id: Some(session_id.to_string()),
            title: Some(title.to_string()),
            created_at: Some(now.clone()),
            updated_at: Some(now),
            message_count: Some(2),
            total_tokens: Some(0),
            model: Some(model.to_string()),
            workspace: Some(workspace.to_string()),
            mode: Some("gui".into()),
        },
        system_prompt: Some(system_prompt.to_string()),
        messages: vec![user_msg, assistant_msg],
    };

    let content = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("序列化会话文件失败: {}", e))?;

    std::fs::write(&path, content).map_err(|e| format!("写入会话文件失败: {}", e))?;

    Ok(())
}
