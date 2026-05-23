mod api;
mod db;
mod agent;
mod session;

use db::{ConversationRow, Database, MessageRow, ProjectRow};
use session::{SessionSummary, SessionMessages, TurnBasedSession};
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
) -> Result<ProjectRow, String> {
    let id = Uuid::new_v4().to_string();
    let pinned = pinned_files.unwrap_or_default();
    db.create_project(&id, &name, &description, &root_path, &instructions, &model, &pinned, "[]", "[]")
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
) -> Result<(), String> {
    let pinned = pinned_files.unwrap_or_default();
    let sk = skills.unwrap_or_else(|| "[]".to_string());
    let mcp = mcp_servers.unwrap_or_else(|| "[]".to_string());
    db.update_project(&id, &name, &description, &instructions, &model, &pinned, &sk, &mcp)
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
        let size = if is_dir { None } else { entry.metadata().ok().map(|m| m.len()) };

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
        let dir = std::env::temp_dir().join(format!("deepseekx-list-files-{}", uuid::Uuid::new_v4()));
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

        assert!(
            agent.children
                .as_ref()
                .unwrap()
                .iter()
                .any(|node| node.path == planner_path)
        );

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
            Some(PathBuf::from("src").join("agent").to_string_lossy().to_string())
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
                items.push(format!("助手: {}", normalize_context_line(final_response, 320)));
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
        if entry.depth() == 0 { continue; }
        if lines.len() >= max_entries { break; }
        let rel = entry.path().strip_prefix(root)
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
    let mut paths: Vec<String> = vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
    ];

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
async fn send_message_via_api(
    db: tauri::State<'_, Arc<Database>>,
    session_id: String,
    content: String,
    title: String,
    workspace_root: String,
    instructions: Option<String>,
    skills: Option<String>,
    mcp_servers: Option<String>,
) -> Result<String, String> {
    use serde_json::json;

    // 1. 获取 API 配置（GUI 设置的 api_key 优先）
    let gui_key = db.get_setting("api_key").unwrap_or(None);
    let config = api::get_api_config(gui_key.as_deref())?;

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
                  ## 联网搜索\n\n\
                  你拥有联网搜索能力，当用户询问实时信息时请主动搜索获取最新数据。",
                workspace_root
            );

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

            // 注入技能和 MCP 信息
            let skills_json = skills.unwrap_or_else(|| "[]".to_string());
            let mcp_json = mcp_servers.unwrap_or_else(|| "[]".to_string());
            let skill_names: Vec<String> = serde_json::from_str(&skills_json).unwrap_or_default();
            let mcp_names: Vec<String> = serde_json::from_str(&mcp_json).unwrap_or_default();

            if !skill_names.is_empty() {
                sp.push_str("\n\n## 项目技能\n\n以下技能已启用：\n\n");
                let all_skills = session::list_available_skills();
                for name in &skill_names {
                    for s in &all_skills {
                        if s.name == *name {
                            sp.push_str(&format!("- **{}**: {}\n", s.name, s.description));
                            break;
                        }
                    }
                }
            }

            if !mcp_names.is_empty() {
                sp.push_str("\n\n## MCP 服务器\n\n以下 MCP 服务器可用：\n\n");
                for name in &mcp_names {
                    sp.push_str(&format!("- {}\n", name));
                }
            }

            if let Some(ref instr) = instructions {
                if !instr.trim().is_empty() {
                    sp.push_str(&format!("\n\n## 项目指令\n\n{}", instr));
                }
            }
            (vec![json!({"role": "system", "content": &sp})], Some(sp))
        }
    };

    let user_msg = json!({
        "role": "user",
        "content": [{"type": "text", "text": content}]
    });

    // 4. 确保 system prompt 包含联网搜索提示（兼容已有对话）
    let mut api_messages = existing_messages;
    if let Some(first) = api_messages.first_mut() {
        if first.get("role").and_then(|r| r.as_str()) == Some("system") {
            if let Some(content) = first.get("content").and_then(|c| c.as_str()) {
                if !content.contains("联网搜索") {
                    let enhanced = format!("{}\n\n你可以使用联网搜索获取实时信息。当用户询问股价、新闻、天气等实时数据时，直接进行搜索。", content);
                    *first = json!({"role": "system", "content": enhanced});
                }
            }
        }
    }
    api_messages.push(user_msg.clone());

    let assistant_content = api::chat_completion(&config, api_messages).await?;

    // 4.5. 从 AI 回复中提取 file: 标记的代码块并写入磁盘
    let raw_text = api::extract_text(&assistant_content);
    let file_blocks = api::parse_file_blocks(&raw_text);
    let mut response_text = raw_text.clone();
    if !file_blocks.is_empty() {
        match api::write_files_to_disk(&workspace_root, &file_blocks) {
            Ok(written) => {
                let summary = written
                    .iter()
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n");
                response_text.push_str(
                    &format!("\n\n---\n\n✅ 已自动写入 {} 个文件到 `{}`:\n\n{}",
                        written.len(), workspace_root, summary)
                );
            }
            Err(e) => {
                response_text.push_str(
                    &format!("\n\n---\n\n⚠️ 文件写入失败: {}", e)
                );
            }
        }
    }

    // 5. 构建 assistant 消息
    let assistant_msg = json!({
        "role": "assistant",
        "content": assistant_content
    });

    // 6. 持久化到 session 文件（存在则追加，不存在则创建）
    let exists = std::path::PathBuf::from(
        std::env::var("HOME").unwrap_or_default()
    ).join(".deepseek").join("sessions").join(format!("{}.json", session_id)).exists();

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
    _skills: Option<String>,
    _mcp_servers: Option<String>,
) -> Result<agent::orchestrator::AgentRunResponse, String> {
    use serde_json::json;

    let gui_key = db.get_setting("api_key").unwrap_or(None);
    let config = api::get_api_config(gui_key.as_deref())?;
    let conversation_context = build_recent_conversation_context(&session_id);
    let result = agent::orchestrator::run_agent_loop(
        &config,
        agent::orchestrator::AgentRunRequest {
            goal: &content,
            conversation_context: conversation_context.as_deref(),
            workspace_root: &workspace_root,
            project_instructions: instructions.as_deref(),
        },
    )
    .await?;
    let agent::orchestrator::AgentRunResponse {
        final_response,
        goal_status,
        steps,
    } = result.clone();
    let user_msg = json!({
        "role": "user",
        "content": content
    });
    let assistant_msg = json!({
        "role": "assistant",
        "content": final_response,
        "agent_goal_status": goal_status,
        "agent_steps": steps.clone()
    });

    let session_file = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".deepseek")
        .join("sessions")
        .join(format!("{}.json", session_id));

    if session_file.exists() {
        session::append_messages(&session_id, user_msg, assistant_msg.clone())?;
    } else {
        let display_title = truncate_title(&title, 30);
        let system_prompt = instructions.unwrap_or_else(|| "Agent mode".to_string());
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

    Ok(result)
}

// ---------- App entry ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            let database = Database::new(app_data_dir)
                .expect("初始化数据库失败");
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
            read_tui_session_turns,
            send_message_via_api,
            run_agent_task,
        ])
        .run(tauri::generate_context!())
        .expect("启动 DeepSeekX 失败");
}
