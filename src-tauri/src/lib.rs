mod agent;
mod api;
mod db;
mod preview;
mod session;

use db::{ConversationRow, Database, MessageRow, ProjectRow};
use session::{SessionMessages, SessionSummary, TurnBasedSession};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Manager;
use tokio::process::Command;
use uuid::Uuid;
use walkdir::WalkDir;

// ---------- Project commands ----------

#[tauri::command]
fn create_project(
    db: tauri::State<Arc<Database>>,
    name: String,
    description: String,
    root_path: String,
    instructions: String,
    model: String,
    pinned_files: Option<String>,
    retrieval_sources: Option<String>,
) -> Result<ProjectRow, String> {
    let id = Uuid::new_v4().to_string();
    let pinned = pinned_files.unwrap_or_default();
    let retrieval = retrieval_sources.unwrap_or_else(default_retrieval_sources_json);
    db.create_project(
        &id,
        &name,
        &description,
        &root_path,
        &instructions,
        &model,
        &pinned,
        "[]",
        "[]",
        &retrieval,
    )
    .map_err(|e| format!("创建项目失败: {}", e))?;
    db.get_project(&id)
        .map_err(|e| format!("读取项目失败: {}", e))?
        .ok_or_else(|| "项目创建后未找到".into())
}

#[tauri::command]
fn list_projects(db: tauri::State<Arc<Database>>) -> Result<Vec<ProjectRow>, String> {
    db.list_projects()
        .map_err(|e| format!("获取项目列表失败: {}", e))
}

#[tauri::command]
fn get_project(db: tauri::State<Arc<Database>>, id: String) -> Result<ProjectRow, String> {
    db.get_project(&id)
        .map_err(|e| format!("读取项目失败: {}", e))?
        .ok_or_else(|| "项目不存在".into())
}

#[tauri::command]
fn update_project(
    db: tauri::State<Arc<Database>>,
    id: String,
    name: String,
    description: String,
    instructions: String,
    model: String,
    pinned_files: Option<String>,
    skills: Option<String>,
    mcp_servers: Option<String>,
    retrieval_sources: Option<String>,
) -> Result<(), String> {
    let pinned = pinned_files.unwrap_or_default();
    let sk = skills.unwrap_or_else(|| "[]".to_string());
    let mcp = mcp_servers.unwrap_or_else(|| "[]".to_string());
    let retrieval = retrieval_sources.unwrap_or_else(default_retrieval_sources_json);
    db.update_project(
        &id,
        &name,
        &description,
        &instructions,
        &model,
        &pinned,
        &sk,
        &mcp,
        &retrieval,
    )
    .map_err(|e| format!("更新项目失败: {}", e))
}

#[tauri::command]
fn delete_project(db: tauri::State<Arc<Database>>, id: String) -> Result<(), String> {
    db.delete_project(&id)
        .map_err(|e| format!("删除项目失败: {}", e))
}

// ---------- Conversation commands ----------

#[tauri::command]
fn create_conversation(
    db: tauri::State<Arc<Database>>,
    project_id: String,
    title: String,
) -> Result<ConversationRow, String> {
    let id = Uuid::new_v4().to_string();
    db.create_conversation(&id, &project_id, &title)
        .map_err(|e| format!("创建对话失败: {}", e))?;
    Ok(ConversationRow {
        id,
        project_id,
        title,
        created_at: chrono::Utc::now().timestamp_millis(),
    })
}

#[tauri::command]
fn list_conversations(
    db: tauri::State<Arc<Database>>,
    project_id: String,
) -> Result<Vec<ConversationRow>, String> {
    db.list_conversations(&project_id)
        .map_err(|e| format!("获取对话列表失败: {}", e))
}

#[tauri::command]
fn delete_conversation(db: tauri::State<Arc<Database>>, id: String) -> Result<(), String> {
    db.delete_conversation(&id)
        .map_err(|e| format!("删除对话失败: {}", e))
}

// ---------- Message commands ----------

#[tauri::command]
fn save_message(
    db: tauri::State<Arc<Database>>,
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    timestamp: i64,
) -> Result<(), String> {
    db.save_message(&id, &conversation_id, &role, &content, timestamp)
        .map_err(|e| format!("保存消息失败: {}", e))
}

#[tauri::command]
fn list_messages(
    db: tauri::State<Arc<Database>>,
    conversation_id: String,
) -> Result<Vec<MessageRow>, String> {
    db.list_messages(&conversation_id)
        .map_err(|e| format!("获取消息列表失败: {}", e))
}

// ---------- File commands ----------

#[derive(Debug, Clone, serde::Serialize)]
struct FileNode {
    name: String,
    path: String,
    is_directory: bool,
    children: Option<Vec<FileNode>>,
    size: Option<u64>,
}

#[tauri::command]
fn list_files(root_path: String, depth: Option<usize>) -> Result<Vec<FileNode>, String> {
    let max_depth = depth.unwrap_or(1);
    let mut root_nodes: Vec<FileNode> = Vec::new();

    for entry in WalkDir::new(&root_path)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok())
    {
        let depth = entry.depth();
        if depth == 0 {
            continue; // skip root itself
        }
        let rel_path = entry
            .path()
            .strip_prefix(&root_path)
            .map_err(|e| format!("路径解析错误: {}", e))?
            .to_string_lossy()
            .to_string();

        let file_type = entry.file_type();
        let is_dir = file_type.is_dir();
        let size = if is_dir {
            None
        } else {
            entry.metadata().ok().map(|m| m.len())
        };

        let node = FileNode {
            name: entry.file_name().to_string_lossy().to_string(),
            path: rel_path,
            is_directory: is_dir,
            children: if is_dir { Some(Vec::new()) } else { None },
            size,
        };

        insert_file_node(&mut root_nodes, node)?;
    }
    Ok(root_nodes)
}

fn insert_file_node(nodes: &mut Vec<FileNode>, node: FileNode) -> Result<(), String> {
    let Some(parent_path) = parent_relative_path(&node.path) else {
        nodes.push(node);
        return Ok(());
    };
    let parent = find_node_by_path_mut(nodes, &parent_path)
        .ok_or_else(|| format!("未找到父目录: {}", parent_path))?;

    if let Some(children) = parent.children.as_mut() {
        children.push(node);
        Ok(())
    } else {
        Err(format!("父节点不是目录: {}", parent_path))
    }
}

fn parent_relative_path(path: &str) -> Option<String> {
    let mut components = Path::new(path).components().peekable();
    let mut parent = PathBuf::new();

    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        parent.push(component.as_os_str());
    }

    if parent.as_os_str().is_empty() {
        None
    } else {
        Some(parent.to_string_lossy().to_string())
    }
}

fn find_node_by_path_mut<'a>(nodes: &'a mut [FileNode], path: &str) -> Option<&'a mut FileNode> {
    for node in nodes.iter_mut() {
        if node.path == path {
            return Some(node);
        }

        if let Some(children) = node.children.as_mut() {
            if let Some(found) = find_node_by_path_mut(children, path) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(test)]
mod file_tree_tests {
    use super::{list_files, parent_relative_path, truncate_title};
    use std::fs;
    use std::path::PathBuf;

    fn make_temp_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("deepseekx-list-files-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn list_files_includes_nested_entries_for_depth_three() {
        let root = make_temp_dir();
        let nested_dir = root.join("src").join("agent");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("planner.rs"), "fn main() {}").unwrap();

        let nodes = list_files(root.to_string_lossy().to_string(), Some(3)).unwrap();
        let src_path = PathBuf::from("src").to_string_lossy().to_string();
        let agent_path = PathBuf::from("src")
            .join("agent")
            .to_string_lossy()
            .to_string();
        let planner_path = PathBuf::from("src")
            .join("agent")
            .join("planner.rs")
            .to_string_lossy()
            .to_string();

        let src = nodes.iter().find(|node| node.path == src_path).unwrap();
        let agent = src
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|node| node.path == agent_path)
            .unwrap();

        assert!(agent
            .children
            .as_ref()
            .unwrap()
            .iter()
            .any(|node| node.path == planner_path));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parent_relative_path_uses_platform_path_rules() {
        let parent = parent_relative_path(
            &PathBuf::from("src")
                .join("agent")
                .join("planner.rs")
                .to_string_lossy(),
        );

        assert_eq!(
            parent,
            Some(
                PathBuf::from("src")
                    .join("agent")
                    .to_string_lossy()
                    .to_string()
            )
        );
    }

    #[test]
    fn truncate_title_handles_utf8_without_panicking() {
        let title = "你好，DeepSeekX Agent 模式";
        let truncated = truncate_title(title, 5);

        assert_eq!(truncated, "你好，De...");
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.') || s == "node_modules" || s == "target")
        .unwrap_or(false)
}

fn truncate_title(title: &str, max_chars: usize) -> String {
    let truncated: String = title.chars().take(max_chars).collect();
    if title.chars().count() > max_chars {
        format!("{}...", truncated)
    } else {
        title.to_string()
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let truncated: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        format!("{}...", truncated)
    } else {
        text.to_string()
    }
}

fn normalize_context_line(text: &str, max_chars: usize) -> String {
    truncate_text(&text.replace('\n', " ").replace('\r', " "), max_chars)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSelections {
    content: String,
    explicit_skills: Vec<String>,
    explicit_mcp_servers: Vec<String>,
}

fn parse_runtime_selections(raw_content: &str) -> RuntimeSelections {
    let mut explicit_skills = Vec::new();
    let mut explicit_mcp_servers = Vec::new();
    let mut body_lines = Vec::new();

    for line in raw_content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("本次请求显式附加技能:") {
            explicit_skills.extend(parse_name_list(rest));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("本次请求显式附加 MCP:") {
            explicit_mcp_servers.extend(parse_name_list(rest));
            continue;
        }
        body_lines.push(line);
    }

    RuntimeSelections {
        content: body_lines.join("\n").trim().to_string(),
        explicit_skills: dedupe_names(explicit_skills),
        explicit_mcp_servers: dedupe_names(explicit_mcp_servers),
    }
}

fn parse_name_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn parse_json_name_list(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default()
}

fn default_retrieval_sources() -> Vec<agent::actions::ContextSource> {
    use agent::actions::ContextSource;

    vec![
        ContextSource::WorkspaceCode,
        ContextSource::WorkspaceDocs,
        ContextSource::UserKnowledgeBase,
        ContextSource::WebSearch,
    ]
}

fn default_retrieval_sources_json() -> String {
    serde_json::to_string(&default_retrieval_sources()).unwrap_or_else(|_| "[]".to_string())
}

fn parse_json_context_sources(raw: Option<&str>) -> Vec<agent::actions::ContextSource> {
    raw.and_then(|value| {
        serde_json::from_str::<Vec<agent::actions::ContextSource>>(value).ok()
    })
    .filter(|sources| !sources.is_empty())
    .unwrap_or_else(default_retrieval_sources)
}

fn parse_line_separated_paths(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn parse_line_separated_values(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn dedupe_names(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut ordered = Vec::new();
    for name in names {
        if seen.insert(name.clone()) {
            ordered.push(name);
        }
    }
    ordered
}

fn merge_named_selections(base: &[String], explicit: &[String]) -> Vec<String> {
    let mut merged = Vec::with_capacity(base.len() + explicit.len());
    merged.extend(base.iter().cloned());
    merged.extend(explicit.iter().cloned());
    dedupe_names(merged)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutomationCapability {
    label: &'static str,
    skill_matches: Vec<String>,
    mcp_matches: Vec<String>,
}

fn detect_browser_capability(
    selected_skills: &[String],
    selected_mcp_servers: &[String],
) -> AutomationCapability {
    detect_automation_capability(
        "浏览器自动化",
        selected_skills,
        selected_mcp_servers,
        &["browser", "agent-browser", "chrome"],
        &["browser", "chrome", "playwright", "puppeteer"],
    )
}

fn detect_desktop_capability(
    selected_skills: &[String],
    selected_mcp_servers: &[String],
) -> AutomationCapability {
    detect_automation_capability(
        "桌面自动化",
        selected_skills,
        selected_mcp_servers,
        &["computer-use"],
        &["computer", "desktop", "macos", "ui"],
    )
}

fn detect_automation_capability(
    label: &'static str,
    selected_skills: &[String],
    selected_mcp_servers: &[String],
    skill_keywords: &[&str],
    mcp_keywords: &[&str],
) -> AutomationCapability {
    AutomationCapability {
        label,
        skill_matches: selected_skills
            .iter()
            .filter(|name| contains_any_keyword(name, skill_keywords))
            .cloned()
            .collect(),
        mcp_matches: selected_mcp_servers
            .iter()
            .filter(|name| contains_any_keyword(name, mcp_keywords))
            .cloned()
            .collect(),
    }
}

fn contains_any_keyword(value: &str, keywords: &[&str]) -> bool {
    let normalized = value.to_ascii_lowercase();
    keywords.iter().any(|keyword| normalized.contains(keyword))
}

fn load_skill_runtime_sections(skill_names: &[String]) -> Vec<String> {
    const MAX_SKILL_CHARS: usize = 8_000;
    let all_skills = session::list_available_skills();
    skill_names
        .iter()
        .filter_map(|name| {
            let skill = all_skills.iter().find(|item| item.name == *name)?;
            let skill_md = PathBuf::from(&skill.path).join("SKILL.md");
            let content = std::fs::read_to_string(&skill_md).ok()?;
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return None;
            }

            let rendered = if trimmed.chars().count() > MAX_SKILL_CHARS {
                format!(
                    "{}\n\n[truncated to first {} chars by DeepSeekX runtime]",
                    trimmed.chars().take(MAX_SKILL_CHARS).collect::<String>(),
                    MAX_SKILL_CHARS
                )
            } else {
                trimmed.to_string()
            };

            Some(format!(
                "### Skill: {}\n来源: {}\n\n{}",
                skill.name,
                skill_md.display(),
                rendered
            ))
        })
        .collect()
}

fn build_runtime_instruction_context(
    instructions: Option<&str>,
    selected_skills: &[String],
    selected_mcp_servers: &[String],
    retrieval_sources: &[agent::actions::ContextSource],
    explicit_skills: &[String],
    explicit_mcp_servers: &[String],
) -> Option<String> {
    let mut sections = Vec::new();

    if let Some(instructions) = instructions
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!("## 项目指令\n\n{}", instructions));
    }

    if !explicit_skills.is_empty() || !explicit_mcp_servers.is_empty() {
        let mut lines = Vec::new();
        if !explicit_skills.is_empty() {
            lines.push(format!("- 显式技能附件: {}", explicit_skills.join(", ")));
        }
        if !explicit_mcp_servers.is_empty() {
            lines.push(format!(
                "- 显式 MCP 附件: {}",
                explicit_mcp_servers.join(", ")
            ));
        }
        sections.push(format!(
            "## 当前请求显式附件\n\n{}\n\n这些附件来自用户本次请求的 `/skill` 或 `/mcp` 选择，执行时应优先遵循。",
            lines.join("\n")
        ));
    }

    if !selected_skills.is_empty() {
        let skill_sections = load_skill_runtime_sections(selected_skills);
        if !skill_sections.is_empty() {
            sections.push(format!(
                "## 已启用 Skills\n\n以下 Skill 指令已加载到运行时，请按其中工作流执行：\n\n{}",
                skill_sections.join("\n\n")
            ));
        } else {
            sections.push(format!(
                "## 已启用 Skills\n\n已选择的 Skills: {}\n\n注意：未能读取对应的 SKILL.md 内容，请至少遵循这些技能名称表达的能力范围。",
                selected_skills.join(", ")
            ));
        }
    }

    if !selected_mcp_servers.is_empty() {
        sections.push(format!(
            "## 已启用 MCP 服务器\n\n{}\n\n这些 MCP 服务器来自项目配置或用户本次请求的显式选择。若当前运行时无法直接调用工具，必须明确说明限制，而不是假装已经执行。",
            selected_mcp_servers
                .iter()
                .map(|name| format!("- {}", name))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    let browser_capability = detect_browser_capability(selected_skills, selected_mcp_servers);
    if !browser_capability.skill_matches.is_empty() || !browser_capability.mcp_matches.is_empty() {
        let browser_skill_list = if browser_capability.skill_matches.is_empty() {
            "(未启用)".to_string()
        } else {
            browser_capability.skill_matches.join(", ")
        };
        let browser_mcp_list = if browser_capability.mcp_matches.is_empty() {
            "(未启用)".to_string()
        } else {
            browser_capability.mcp_matches.join(", ")
        };
        let execution_note = if browser_capability.mcp_matches.is_empty() {
            "当前只检测到浏览器相关 skill，没有匹配到浏览器类 MCP server；你可以提供浏览器自动化建议，但不能假装已经真正打开页面或点击元素。"
        } else {
            "当前已检测到浏览器相关 MCP server，可以在需要页面导航、抓取、UI 验证时优先使用对应工具。"
        };
        sections.push(format!(
            "## 浏览器自动化能力\n\n- Skills: {}\n- MCP Servers: {}\n\n{}",
            browser_skill_list,
            browser_mcp_list,
            execution_note
        ));
    }

    let desktop_capability = detect_desktop_capability(selected_skills, selected_mcp_servers);
    if !desktop_capability.skill_matches.is_empty() || !desktop_capability.mcp_matches.is_empty() {
        let desktop_skill_list = if desktop_capability.skill_matches.is_empty() {
            "(未启用)".to_string()
        } else {
            desktop_capability.skill_matches.join(", ")
        };
        let desktop_mcp_list = if desktop_capability.mcp_matches.is_empty() {
            "(未启用)".to_string()
        } else {
            desktop_capability.mcp_matches.join(", ")
        };
        let execution_note = if desktop_capability.mcp_matches.is_empty() {
            "当前只检测到桌面/Computer Use 相关 skill，没有匹配到桌面类 MCP server；你可以描述操作计划，但不能假装已经操控本地应用。"
        } else {
            "当前已检测到桌面/Computer Use 相关 MCP server，可以在需要原生应用交互时优先使用对应工具。"
        };
        sections.push(format!(
            "## 桌面自动化能力\n\n- Skills: {}\n- MCP Servers: {}\n\n{}",
            desktop_skill_list,
            desktop_mcp_list,
            execution_note
        ));
    }

    if !retrieval_sources.is_empty() {
        let source_labels = retrieval_sources
            .iter()
            .map(|source| match source {
                agent::actions::ContextSource::WorkspaceCode => "workspace_code（工作区代码）",
                agent::actions::ContextSource::WorkspaceDocs => "workspace_docs（工作区文档）",
                agent::actions::ContextSource::UserKnowledgeBase => {
                    "user_knowledge_base（用户知识库）"
                }
                agent::actions::ContextSource::WebSearch => "web_search（联网搜索）",
            })
            .collect::<Vec<_>>();
        sections.push(format!(
            "## 当前项目允许的检索来源\n\n{}\n\n只能在这些来源内检索信息；若某来源未启用，就不要假设它可用。",
            source_labels
                .iter()
                .map(|label| format!("- {}", label))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

fn build_recent_conversation_context(session_id: &str) -> Option<String> {
    let session_data = session::read_session(session_id).ok()?;
    let turns = session::group_into_turns(session_id, &session_data.messages);
    if turns.is_empty() {
        return None;
    }

    let lines = turns
        .iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .flat_map(|turn| {
            let mut items = Vec::new();
            let user_input = normalize_context_line(turn.user_input.trim(), 240);
            if !user_input.is_empty() {
                items.push(format!("用户: {}", user_input));
            }
            if let Some(final_response) = turn
                .final_response
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                items.push(format!(
                    "助手: {}",
                    normalize_context_line(final_response, 320)
                ));
            }
            items
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

#[derive(Debug)]
struct ChatToolLoopResult {
    final_response: String,
    goal_status: Option<String>,
    steps: Vec<agent::state::AgentObservation>,
    llm_debug_responses: Vec<api::LlmDebugResponse>,
}

fn build_chat_tool_protocol(mcp_catalog: Option<&str>) -> String {
    let mcp_section = mcp_catalog
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("可用 MCP 工具:\n{}\n", value))
        .unwrap_or_else(|| "可用 MCP 工具:\n(当前没有可调用的 MCP 工具)\n".to_string());

    format!(
        "## 普通 Chat 工具协议\n\n\
         你当前运行在普通 Chat 模式。默认优先直接回答用户。\n\
         只有在确实需要读取工作区、修改文件、执行验证命令、联网抓取信息或调用 MCP 时，才可以请求一个工具动作。\n\n\
         如果需要工具，请只输出一个 JSON 对象，不要加 markdown 代码块，也不要附带解释文字。\n\
         允许的 action: retrieve_context, read_files, create_files, rename_files, delete_files, apply_patch, write_files, run_command, fetch_url, verify_checks, call_mcp_tool, summarize_findings, ask_user, finish。\n\
         规则:\n\
         - 每次最多请求一个 action。\n\
         - 外部网页/API 抓取优先用 fetch_url，不要把外部 curl/wget 放进 run_command。\n\
         - build/test/lint 等验证优先用 verify_checks。\n\
         - 小范围修改优先用 apply_patch。\n\
         - 如果你已经得到足够信息，直接输出自然语言最终答复，不必额外输出 finish。\n\
         - 如果需要用户确认或权限批准，输出 ask_user。\n\
         - 当系统返回 `<tool_result>` 后，请基于结果继续决定下一步或直接回答。\n\n\
         字段要求:\n\
         - run_command: action, reason, command, 可选 cwd\n\
         - fetch_url: action, reason, url, method, 可选 max_chars\n\
         - verify_checks: action, reason, checks（每项包含 label, command, 可选 cwd）\n\
         - call_mcp_tool: action, reason, server, tool, arguments\n\
         - 其它 action 字段与 Agent 模式保持一致。\n\n\
         {}\n\
         如果不需要工具，请直接正常回答用户。",
        mcp_section
    )
}

fn summarize_chat_mcp_catalog(tools: &[agent::mcp::McpToolDescriptor]) -> Option<String> {
    if tools.is_empty() {
        return None;
    }

    Some(
        tools
            .iter()
            .map(|tool| {
                let description = tool.description.as_deref().unwrap_or("(无描述)");
                format!("- {}.{}: {}", tool.server, tool.name, description)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn build_chat_action_observation(
    action_name: &str,
    reason: String,
    input_summary: String,
    result_summary: String,
    summary: String,
    status: &str,
    is_error: bool,
    requires_confirmation: bool,
) -> agent::state::AgentObservation {
    use agent::state::{AgentObservation, AgentStepStatus};

    AgentObservation {
        action_name: action_name.to_string(),
        reason,
        input_summary,
        result_summary,
        summary,
        status: match status {
            "blocked" => AgentStepStatus::Blocked,
            "failed" => AgentStepStatus::Failed,
            _ => AgentStepStatus::Completed,
        },
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

fn append_file_block_summary(
    workspace_root: &str,
    response_text: &str,
    steps: &mut Vec<agent::state::AgentObservation>,
) -> String {
    let file_blocks = api::parse_file_blocks(response_text);
    if file_blocks.is_empty() {
        return response_text.to_string();
    }

    let mut updated = response_text.to_string();
    match api::write_files_to_disk(workspace_root, &file_blocks) {
        Ok(written) => {
            let summary = written
                .iter()
                .map(|f| format!("- {}", f))
                .collect::<Vec<_>>()
                .join("\n");
            updated.push_str(&format!(
                "\n\n---\n\n✅ 已自动写入 {} 个文件到 `{}`:\n\n{}",
                written.len(),
                workspace_root,
                summary
            ));
            steps.push(build_chat_action_observation(
                "write_files",
                "模型输出了带 file: 标记的代码块".into(),
                written.join(", "),
                summary.clone(),
                format!("已自动写入 {} 个文件", written.len()),
                "completed",
                false,
                false,
            ));
        }
        Err(error) => {
            updated.push_str(&format!("\n\n---\n\n⚠️ 文件写入失败: {}", error));
            steps.push(build_chat_action_observation(
                "write_files",
                "模型输出了带 file: 标记的代码块".into(),
                file_blocks
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                error.clone(),
                "自动写入 file: 代码块失败".into(),
                "failed",
                true,
                false,
            ));
        }
    }

    updated
}

fn summarize_error_stage(steps: &[agent::state::AgentObservation]) -> Option<String> {
    steps.iter()
        .rev()
        .find(|step| step.is_error || matches!(step.status, agent::state::AgentStepStatus::Blocked))
        .map(|step| step.action_name.clone())
}

fn summarize_retry_count(steps: &[agent::state::AgentObservation]) -> u64 {
    let error_count = steps.iter().filter(|step| step.is_error).count();
    error_count.saturating_sub(1) as u64
}

async fn run_chat_tool_loop(
    config: &api::ApiConfig,
    mut messages: Vec<serde_json::Value>,
    workspace_root: &str,
    mcp_servers: &[api::McpServerConfig],
    retrieval_sources: &[agent::actions::ContextSource],
    user_knowledge_base_paths: &[String],
    command_allowlist: &[String],
) -> Result<ChatToolLoopResult, String> {
    use agent::actions::GoalStatus;
    use agent::orchestrator::{execute_action, ActionExecutionOutcome};
    use agent::permissions::{evaluate_action_permission, PermissionOutcome};
    use serde_json::json;

    const MAX_CHAT_TOOL_STEPS: usize = 6;
    let mut steps: Vec<agent::state::AgentObservation> = Vec::new();
    let mut llm_debug_responses: Vec<api::LlmDebugResponse> = Vec::new();

    for _ in 0..MAX_CHAT_TOOL_STEPS {
        let assistant_result =
            api::chat_completion_with_debug(config, messages.clone(), true).await?;
        llm_debug_responses.push(assistant_result.debug.clone());
        let raw_text = api::extract_text(&assistant_result.content);

        if let Ok(action) = agent::planner::parse_planner_action(&raw_text) {
            match evaluate_action_permission(Path::new(workspace_root), &action, command_allowlist)? {
                PermissionOutcome::Allowed => {}
                PermissionOutcome::RequiresConfirmation { reason, risk_level } => {
                    let input_summary = format!("{:?}", action);
                    steps.push(build_chat_action_observation(
                        "permission_gate",
                        "权限门拦截了普通 Chat 模式下的工具请求".into(),
                        input_summary,
                        format!("risk_level={}; {}", risk_level, reason),
                        "工具动作需要用户确认".into(),
                        "blocked",
                        true,
                        true,
                    ));
                    return Ok(ChatToolLoopResult {
                        final_response: format!("这个请求需要你确认后我才能继续执行：{}", reason),
                        goal_status: Some("needs_confirmation".into()),
                        steps,
                        llm_debug_responses,
                    });
                }
            }

            match execute_action(
                config,
                workspace_root,
                &action,
                &steps,
                mcp_servers,
                retrieval_sources,
                user_knowledge_base_paths,
                command_allowlist,
            )
            .await
            {
                ActionExecutionOutcome::Observation { observation, .. } => {
                    let tool_result_prompt = format!(
                        "<tool_result>\naction={}\nstatus={}\nsummary={}\nresult={}\n</tool_result>",
                        observation.action_name,
                        serde_json::to_string(&observation.status)
                            .unwrap_or_else(|_| "\"unknown\"".into()),
                        observation.summary,
                        observation.result_summary,
                    );
                    steps.push(observation);
                    messages.push(json!({
                        "role": "assistant",
                        "content": raw_text,
                    }));
                    messages.push(json!({
                        "role": "user",
                        "content": [{"type": "text", "text": tool_result_prompt}],
                    }));
                }
                ActionExecutionOutcome::Finish {
                    goal_status,
                    summary,
                    verification,
                    ..
                } => {
                    let final_response = match goal_status {
                        GoalStatus::NeedsConfirmation => summary.clone(),
                        _ => format!("{}\n\n验证信息：{}", summary, verification),
                    };
                    let status_label = agent::orchestrator::goal_status_label(&goal_status);
                    steps.push(build_chat_action_observation(
                        "finish",
                        "普通 Chat 工具循环判断任务已收敛".into(),
                        raw_text.clone(),
                        verification,
                        summary,
                        if status_label == "blocked" {
                            "blocked"
                        } else {
                            "completed"
                        },
                        status_label != "done",
                        status_label == "needs_confirmation",
                    ));
                    return Ok(ChatToolLoopResult {
                        final_response,
                        goal_status: Some(status_label.to_string()),
                        steps,
                        llm_debug_responses,
                    });
                }
            }
        } else {
            let final_response = append_file_block_summary(workspace_root, &raw_text, &mut steps);
            return Ok(ChatToolLoopResult {
                final_response,
                goal_status: None,
                steps,
                llm_debug_responses,
            });
        }
    }

    steps.push(build_chat_action_observation(
        "tool_loop_budget",
        "普通 Chat 工具循环达到步数上限".into(),
        "max_steps=6".into(),
        "普通 Chat 模式下的工具请求过多，已停止继续循环".into(),
        "已达到普通 Chat 工具步数上限".into(),
        "failed",
        true,
        false,
    ));

    Ok(ChatToolLoopResult {
        final_response:
            "我已经执行了多轮工具步骤，但还没有收敛出稳定答复。请缩小范围，或切换到 Agent 模式继续。"
                .into(),
        goal_status: None,
        steps,
        llm_debug_responses,
    })
}

#[cfg(test)]
mod runtime_selection_tests {
    use super::{
        build_runtime_instruction_context, detect_browser_capability,
        detect_desktop_capability, merge_named_selections, parse_runtime_selections,
    };
    use crate::agent::actions::ContextSource;

    #[test]
    fn parse_runtime_selections_extracts_explicit_attachments() {
        let parsed = parse_runtime_selections(
            "本次请求显式附加技能: browser, gmail\n本次请求显式附加 MCP: fetch, search\n\n请帮我整理这个页面",
        );

        assert_eq!(parsed.content, "请帮我整理这个页面");
        assert_eq!(parsed.explicit_skills, vec!["browser", "gmail"]);
        assert_eq!(parsed.explicit_mcp_servers, vec!["fetch", "search"]);
    }

    #[test]
    fn merge_named_selections_preserves_order_and_deduplicates() {
        let merged = merge_named_selections(
            &["browser".into(), "gmail".into()],
            &["gmail".into(), "search".into()],
        );

        assert_eq!(merged, vec!["browser", "gmail", "search"]);
    }

    #[test]
    fn detects_browser_and_desktop_capabilities_from_selected_names() {
        let browser = detect_browser_capability(
            &["browser".into(), "computer-use".into()],
            &["browser".into(), "filesystem".into()],
        );
        let desktop = detect_desktop_capability(
            &["browser".into(), "computer-use".into()],
            &["computer-use".into(), "filesystem".into()],
        );

        assert_eq!(browser.skill_matches, vec!["browser"]);
        assert_eq!(browser.mcp_matches, vec!["browser"]);
        assert_eq!(desktop.skill_matches, vec!["computer-use"]);
        assert_eq!(desktop.mcp_matches, vec!["computer-use"]);
    }

    #[test]
    fn runtime_instruction_context_mentions_browser_and_desktop_capabilities() {
        let context = build_runtime_instruction_context(
            Some("优先验证 UI"),
            &["browser".into(), "computer-use".into()],
            &["browser".into(), "computer-use".into()],
            &[ContextSource::WorkspaceCode, ContextSource::WebSearch],
            &[],
            &[],
        )
        .unwrap();

        assert!(context.contains("浏览器自动化能力"));
        assert!(context.contains("桌面自动化能力"));
        assert!(context.contains("页面导航"));
        assert!(context.contains("原生应用交互"));
    }
}

/// 列出工作目录中的文件（纯文本，用于 system prompt 上下文）
fn list_workspace_files(root: &str, depth: usize, max_entries: usize) -> Result<String, String> {
    if root.is_empty() {
        return Ok(String::new());
    }
    let root_path = std::path::Path::new(root);
    if !root_path.exists() {
        return Ok(String::new());
    }
    let mut lines: Vec<String> = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(depth)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok())
    {
        if entry.depth() == 0 {
            continue;
        }
        if lines.len() >= max_entries {
            break;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if entry.file_type().is_dir() {
            lines.push(format!("  {}/", rel));
        } else {
            lines.push(format!("  {}", rel));
        }
    }
    Ok(lines.join("\n"))
}

#[tauri::command]
async fn read_file_content(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取文件失败: {}", e))
}

// ---------- Settings commands ----------

#[tauri::command]
fn get_setting(db: tauri::State<Arc<Database>>, key: String) -> Result<Option<String>, String> {
    db.get_setting(&key)
        .map_err(|e| format!("读取设置失败: {}", e))
}

#[tauri::command]
fn set_setting(db: tauri::State<Arc<Database>>, key: String, value: String) -> Result<(), String> {
    db.set_setting(&key, &value)
        .map_err(|e| format!("保存设置失败: {}", e))
}

// ---------- Chat command ----------

fn build_search_path() -> String {
    let mut paths: Vec<String> = vec!["/opt/homebrew/bin".into(), "/usr/local/bin".into()];

    if let Ok(home) = std::env::var("HOME") {
        paths.push(format!("{}/.local/bin", home));
        paths.push(format!("{}/.npm-global/bin", home));
        paths.push(format!("{}/.cargo/bin", home));
        paths.push(format!("{}/.local/share/pnpm", home));
        paths.push(format!("{}/.nvm/versions/node/*/bin", home));

        // fnm 管理的 node 版本：glob 查找
        if let Ok(entries) = std::fs::read_dir(format!("{}/.local/state/fnm_multishells", home)) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin");
                if bin.exists() {
                    paths.push(bin.to_string_lossy().to_string());
                }
            }
        }
    }

    if let Ok(sys_path) = std::env::var("PATH") {
        paths.push(sys_path);
    }

    paths.join(":")
}

fn resolve_deepseek_path() -> String {
    let search_path = build_search_path();

    // 用扩展后的 PATH 搜索
    if let Ok(path) = which::which_in("deepseek", Some(&search_path), ".") {
        return path.to_string_lossy().to_string();
    }

    // 最终回退
    "deepseek".to_string()
}

#[tauri::command]
async fn send_message(
    db: tauri::State<'_, Arc<Database>>,
    conversation_id: String,
    prompt: String,
) -> Result<String, String> {
    let deepseek_path = resolve_deepseek_path();

    let mut cmd = Command::new(&deepseek_path);
    cmd.arg("-p")
        .arg(&prompt)
        .env("DEEPSEEK_CONVERSATION_ID", &conversation_id)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // 注入用户配置的 API key（优先级高于 config.toml）
    if let Ok(Some(api_key)) = db.get_setting("api_key") {
        if !api_key.is_empty() {
            cmd.env("DEEPSEEK_API_KEY", &api_key);
        }
    }

    // macOS 应用环境 PATH 受限，手动注入完整 PATH
    if let Ok(path) = std::env::var("PATH") {
        let extended = format!(
            "/opt/homebrew/bin:/usr/local/bin:{}/.local/bin:{}/.npm-global/bin:{}/.cargo/bin:{}",
            std::env::var("HOME").unwrap_or_default(),
            std::env::var("HOME").unwrap_or_default(),
            std::env::var("HOME").unwrap_or_default(),
            path
        );
        cmd.env("PATH", &extended);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 DeepSeek CLI ({}): {}", deepseek_path, e))?;

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("等待 DeepSeek CLI 失败: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("DeepSeek CLI 返回错误: {}", stderr))
    }
}

// ---------- TUI Session commands ----------

#[tauri::command]
fn list_tui_sessions(workspace: Option<String>) -> Result<Vec<SessionSummary>, String> {
    session::list_sessions(workspace.as_deref())
}

#[tauri::command]
fn read_tui_session(session_id: String) -> Result<SessionMessages, String> {
    session::read_session(&session_id)
}

#[tauri::command]
fn delete_tui_session(session_id: String) -> Result<(), String> {
    session::delete_session(&session_id)
}

#[tauri::command]
fn read_tui_session_turns(session_id: String) -> Result<TurnBasedSession, String> {
    let session_data = session::read_session(&session_id)?;
    let turns = session::group_into_turns(&session_id, &session_data.messages);
    Ok(TurnBasedSession {
        id: session_data.id,
        title: session_data.title,
        turns,
        system_prompt: session_data.system_prompt,
    })
}

#[tauri::command]
fn list_available_skills() -> Vec<session::SkillInfo> {
    session::list_available_skills()
}

#[tauri::command]
fn list_available_mcp_servers() -> Vec<String> {
    api::list_available_mcp_servers()
}

#[tauri::command]
fn read_llm_logs(limit: Option<usize>) -> Result<Vec<api::LlmRequestLogEntry>, String> {
    api::read_llm_logs(limit.unwrap_or(20))
}

#[tauri::command]
async fn send_message_via_api(
    db: tauri::State<'_, Arc<Database>>,
    session_id: String,
    content: String,
    title: String,
    workspace_root: String,
    instructions: Option<String>,
    skills: Option<String>,
    mcp_servers: Option<String>,
    retrieval_sources: Option<String>,
) -> Result<String, String> {
    use serde_json::json;
    let started_at = std::time::Instant::now();

    let gui_key = db.get_setting("api_key").unwrap_or(None);
    let config = api::get_api_config(gui_key.as_deref())?;
    let knowledge_base_paths =
        parse_line_separated_paths(db.get_setting("knowledge_base_paths").unwrap_or(None).as_deref());
    let command_allowlist =
        parse_line_separated_values(db.get_setting("command_allowlist").unwrap_or(None).as_deref());
    let runtime_selections = parse_runtime_selections(&content);
    let selected_project_skills = parse_json_name_list(skills.as_deref());
    let selected_project_mcp_servers = parse_json_name_list(mcp_servers.as_deref());
    let selected_retrieval_sources = parse_json_context_sources(retrieval_sources.as_deref());
    let effective_skills = merge_named_selections(
        &selected_project_skills,
        &runtime_selections.explicit_skills,
    );
    let effective_mcp_servers = merge_named_selections(
        &selected_project_mcp_servers,
        &runtime_selections.explicit_mcp_servers,
    );
    let runtime_instruction_context = build_runtime_instruction_context(
        instructions.as_deref(),
        &effective_skills,
        &effective_mcp_servers,
        &selected_retrieval_sources,
        &runtime_selections.explicit_skills,
        &runtime_selections.explicit_mcp_servers,
    );
    let web_search_enabled = selected_retrieval_sources
        .iter()
        .any(|source| matches!(source, agent::actions::ContextSource::WebSearch));
    let resolved_mcp_servers = api::get_mcp_server_configs(&effective_mcp_servers)?;
    let discovery = agent::mcp::discover_tools(&resolved_mcp_servers).await?;
    let mcp_catalog = summarize_chat_mcp_catalog(&discovery.tools);

    // 2. 读取已有消息（如果 session 文件存在）
    let (existing_messages, system_prompt) = match session::read_session(&session_id) {
        Ok(session_data) => {
            let mut msgs: Vec<serde_json::Value> = Vec::new();
            if let Some(sp) = &session_data.system_prompt {
                msgs.push(json!({"role": "system", "content": sp}));
            }
            msgs.extend(session_data.messages);
            (msgs, session_data.system_prompt)
        }
        Err(_) => {
            // 新对话：构建 system prompt（包含项目指令 + 工作目录快照）
            let mut sp = format!(
                "## 语言要求\n\n你正在 DeepSeekX GUI 中运行。请用简体中文回复。\n\n\
                 ## 工作目录\n\n\
                 当前工作目录是: `{}`\n\
                 你对该目录有完整的读写权限。所有文件操作（创建、修改、删除）都应该在此目录下进行。\n\n\
                 ## 文件写入能力\n\n\
                 你可以直接向工作目录写入文件。使用以下格式创建文件：\n\n\
                 ```lang file:相对路径\n\
                 文件内容\n\
                 ```\n\n\
                 例如：\n\n\
                 ```go file:main.go\n\
                 package main\n\
                 import \"fmt\"\n\
                 func main() {{ fmt.Println(\"hello\") }}\n\
                 ```\n\n\
                 GUI 会自动将标记了 file: 的代码块保存到工作目录下的对应路径。\n\
                 支持嵌套目录（如 `file:cmd/server/main.go`），目录会自动创建。\n\
                  不需要用注释标明路径，直接在代码块第一行使用 file: 标记即可。\n\n\
                  ## 联网搜索\n\n",
                workspace_root
            );
            if web_search_enabled {
                sp.push_str("你拥有联网搜索能力，当用户询问实时信息时请主动搜索获取最新数据。");
            } else {
                sp.push_str("当前项目未启用联网搜索。请优先依赖本地工作区和已提供上下文，不要假装已经访问外部网站。");
            }

            // 列出工作目录下已有文件（最多 30 条）
            sp.push_str("\n\n## 当前目录内容\n\n");
            match list_workspace_files(&workspace_root, 1, 30) {
                Ok(listing) if !listing.is_empty() => {
                    sp.push_str(&listing);
                }
                _ => {
                    sp.push_str("(空目录或无法读取)\n");
                }
            }

            if let Some(runtime_instruction_context) = runtime_instruction_context.as_deref() {
                sp.push_str("\n\n");
                sp.push_str(runtime_instruction_context);
            } else if let Some(ref instr) = instructions {
                if !instr.trim().is_empty() {
                    sp.push_str(&format!("\n\n## 项目指令\n\n{}", instr));
                }
            }
            (vec![json!({"role": "system", "content": &sp})], Some(sp))
        }
    };

    let user_msg = json!({
        "role": "user",
        "content": [{"type": "text", "text": runtime_selections.content}]
    });

    // 4. 确保 system prompt 包含联网搜索提示（兼容已有对话）
    let mut api_messages = existing_messages;
    if let Some(first) = api_messages.first_mut() {
        if first.get("role").and_then(|r| r.as_str()) == Some("system") {
            if let Some(content) = first.get("content").and_then(|c| c.as_str()) {
                if web_search_enabled && !content.contains("联网搜索") {
                    let enhanced = format!("{}\n\n你可以使用联网搜索获取实时信息。当用户询问股价、新闻、天气等实时数据时，直接进行搜索。", content);
                    *first = json!({"role": "system", "content": enhanced});
                }
            }
        }
    }
    if let Some(runtime_instruction_context) = runtime_instruction_context.as_deref() {
        api_messages.push(json!({
            "role": "system",
            "content": runtime_instruction_context,
        }));
    }
    api_messages.push(json!({
        "role": "system",
        "content": build_chat_tool_protocol(mcp_catalog.as_deref()),
    }));
    api_messages.push(user_msg.clone());

    let chat_result = run_chat_tool_loop(
        &config,
        api_messages,
        &workspace_root,
        &resolved_mcp_servers,
        &selected_retrieval_sources,
        &knowledge_base_paths,
        &command_allowlist,
    )
    .await?;
    let response_text = chat_result.final_response.clone();
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let error_stage = summarize_error_stage(&chat_result.steps);
    let retry_count = summarize_retry_count(&chat_result.steps);

    // 5. 构建 assistant 消息
    let assistant_msg = json!({
        "role": "assistant",
        "content": response_text,
        "agent_goal_status": chat_result.goal_status,
        "agent_steps": chat_result.steps.clone(),
        "duration_ms": duration_ms,
        "error_stage": error_stage,
        "retry_count": retry_count,
        "llm_debug_responses": chat_result.llm_debug_responses,
    });

    // 6. 持久化到 session 文件（存在则追加，不存在则创建）
    let exists = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".deepseek")
        .join("sessions")
        .join(format!("{}.json", session_id))
        .exists();

    if exists {
        session::append_messages(&session_id, user_msg, assistant_msg.clone())?;
    } else {
        let display_title = truncate_title(&title, 30);
        let sp = system_prompt.unwrap_or_default();
        session::create_session(
            &session_id,
            &display_title,
            &workspace_root,
            &config.model,
            &sp,
            user_msg,
            assistant_msg.clone(),
        )?;
    }

    // 7. 返回文本（含文件写入摘要）给前端
    Ok(response_text)
}

#[tauri::command]
async fn run_agent_task(
    db: tauri::State<'_, Arc<Database>>,
    session_id: String,
    content: String,
    title: String,
    workspace_root: String,
    instructions: Option<String>,
    skills: Option<String>,
    mcp_servers: Option<String>,
    retrieval_sources: Option<String>,
) -> Result<agent::orchestrator::AgentRunResponse, String> {
    use serde_json::json;
    let started_at = std::time::Instant::now();

    let gui_key = db.get_setting("api_key").unwrap_or(None);
    let config = api::get_api_config(gui_key.as_deref())?;
    let knowledge_base_paths =
        parse_line_separated_paths(db.get_setting("knowledge_base_paths").unwrap_or(None).as_deref());
    let command_allowlist =
        parse_line_separated_values(db.get_setting("command_allowlist").unwrap_or(None).as_deref());
    let conversation_context = build_recent_conversation_context(&session_id);
    let restored_session_state = session::read_session_file(&session_id)
        .map(|file| session::conversation_state_or_default(&file, &session_id))
        .unwrap_or_else(|_| agent::state::AgentSessionStateMachine::new(session_id.clone()));
    let runtime_selections = parse_runtime_selections(&content);
    let selected_project_skills = parse_json_name_list(skills.as_deref());
    let selected_project_mcp_servers = parse_json_name_list(mcp_servers.as_deref());
    let selected_retrieval_sources = parse_json_context_sources(retrieval_sources.as_deref());
    let effective_skills = merge_named_selections(
        &selected_project_skills,
        &runtime_selections.explicit_skills,
    );
    let effective_mcp_servers = merge_named_selections(
        &selected_project_mcp_servers,
        &runtime_selections.explicit_mcp_servers,
    );
    let runtime_instruction_context = build_runtime_instruction_context(
        instructions.as_deref(),
        &effective_skills,
        &effective_mcp_servers,
        &selected_retrieval_sources,
        &runtime_selections.explicit_skills,
        &runtime_selections.explicit_mcp_servers,
    );
    let resolved_mcp_servers = api::get_mcp_server_configs(&effective_mcp_servers)?;
    let discovery = agent::mcp::discover_tools(&resolved_mcp_servers).await?;
    let result = agent::orchestrator::run_agent_loop(
        &config,
        agent::orchestrator::AgentRunRequest {
            goal: &runtime_selections.content,
            conversation_context: conversation_context.as_deref(),
            workspace_root: &workspace_root,
            restored_session_state,
            project_instructions: runtime_instruction_context.as_deref(),
            mcp_servers: resolved_mcp_servers,
            mcp_tools: discovery.tools,
            retrieval_sources: selected_retrieval_sources,
            user_knowledge_base_paths: knowledge_base_paths,
            command_allowlist,
        },
    )
    .await?;
    let agent::orchestrator::AgentRunResponse {
        final_response,
        goal_status,
        steps,
        llm_debug_responses,
        session_state,
    } = result.clone();
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let error_stage = summarize_error_stage(&steps);
    let retry_count = summarize_retry_count(&steps);
    let user_msg = json!({
        "role": "user",
        "content": runtime_selections.content
    });
    let assistant_msg = json!({
        "role": "assistant",
        "content": final_response,
        "agent_goal_status": goal_status,
        "agent_steps": steps.clone(),
        "duration_ms": duration_ms,
        "error_stage": error_stage,
        "retry_count": retry_count,
        "llm_debug_responses": llm_debug_responses,
    });

    let session_file = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".deepseek")
        .join("sessions")
        .join(format!("{}.json", session_id));

    if session_file.exists() {
        session::append_messages(&session_id, user_msg, assistant_msg.clone())?;
    } else {
        let display_title = truncate_title(&title, 30);
        let system_prompt = runtime_instruction_context.unwrap_or_else(|| "Agent mode".to_string());
        session::create_session(
            &session_id,
            &display_title,
            &workspace_root,
            "agent-loop",
            &system_prompt,
            user_msg,
            assistant_msg.clone(),
        )?;
    }
    session::persist_agent_session_state(&session_id, &session_state)?;

    Ok(result)
}

// ---------- App entry ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("无法获取应用数据目录");
            let database = Database::new(app_data_dir).expect("初始化数据库失败");
            app.manage(Arc::new(database));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Project
            create_project,
            list_projects,
            get_project,
            update_project,
            delete_project,
            // Conversation
            create_conversation,
            list_conversations,
            delete_conversation,
            // Message
            save_message,
            list_messages,
            // File
            list_files,
            read_file_content,
            // Preview
            preview::describe_file_preview,
            preview::resolve_file_preview,
            // Settings
            get_setting,
            set_setting,
            // Chat
            send_message,
            // TUI Session
            list_tui_sessions,
            read_tui_session,
            delete_tui_session,
            list_available_skills,
            list_available_mcp_servers,
            read_llm_logs,
            read_tui_session_turns,
            send_message_via_api,
            run_agent_task,
        ])
        .run(tauri::generate_context!())
        .expect("启动 DeepSeekX 失败");
}
