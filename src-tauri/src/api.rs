use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::agent::permissions::resolve_workspace_path;

// ---------- 配置 ----------

#[derive(Debug, Deserialize)]
struct DeepSeekConfig {
    api_key: Option<String>,
    base_url: Option<String>,
    default_text_model: Option<String>,
    #[serde(default)]
    providers: Providers,
    #[serde(default)]
    mcp: McpConfig,
}

#[derive(Debug, Default, Deserialize)]
struct Providers {
    deepseek: Option<ProviderConfig>,
}

#[derive(Debug, Deserialize)]
struct ProviderConfig {
    api_key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct McpConfig {
    #[serde(default)]
    servers: BTreeMap<String, McpServerTomlConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct McpServerTomlConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

fn load_config() -> Result<DeepSeekConfig, String> {
    let home = std::env::var("HOME").map_err(|_| "无法获取 HOME 目录".to_string())?;
    let config_path = PathBuf::from(home).join(".deepseek").join("config.toml");

    let content =
        std::fs::read_to_string(&config_path).map_err(|e| format!("读取 TUI 配置失败: {}", e))?;

    toml::from_str(&content).map_err(|e| format!("解析 TUI 配置失败: {}", e))
}

/// 读取 API 配置（base_url / api_key / model），支持 GUI 覆盖 api_key
pub fn get_api_config(gui_api_key: Option<&str>) -> Result<ApiConfig, String> {
    let config = load_config()?;

    let base_url = config
        .base_url
        .unwrap_or_else(|| "https://api.deepseek.com".to_string());

    // api_key 优先级：GUI 设置 > provider.deepseek.api_key > 顶层 api_key
    let api_key = if let Some(key) = gui_api_key {
        if !key.is_empty() {
            key.to_string()
        } else {
            config
                .providers
                .deepseek
                .as_ref()
                .and_then(|p| p.api_key.clone())
                .or(config.api_key)
                .unwrap_or_default()
        }
    } else {
        config
            .providers
            .deepseek
            .as_ref()
            .and_then(|p| p.api_key.clone())
            .or(config.api_key)
            .unwrap_or_default()
    };

    let model = config
        .default_text_model
        .unwrap_or_else(|| "deepseek-chat".to_string());

    Ok(ApiConfig {
        base_url,
        api_key,
        model,
    })
}

pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestLogEntry {
    timestamp: String,
    request_id: String,
    endpoint: String,
    model: String,
    web_search_enabled: bool,
    request_body: serde_json::Value,
    status_code: Option<u16>,
    duration_ms: u128,
    success: bool,
    error: Option<String>,
    response_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmDebugResponse {
    pub request_id: String,
    pub endpoint: String,
    pub model: String,
    pub web_search_enabled: bool,
    pub duration_ms: u128,
    pub response_text: String,
}

pub struct ChatCompletionResult {
    pub content: Value,
    pub debug: LlmDebugResponse,
}

// ---------- API 请求/响应 ----------

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_search: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: Value,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    role: String,
    content: Value,
}

/// 调用 DeepSeek chat completions API
/// 返回 assistant 的 content（Value 类型，可能是字符串或 ContentBlock 数组）
pub async fn chat_completion_with_options(
    config: &ApiConfig,
    messages: Vec<Value>,
    web_search_enabled: bool,
) -> Result<Value, String> {
    chat_completion_with_debug(config, messages, web_search_enabled)
        .await
        .map(|result| result.content)
}

pub async fn chat_completion_with_debug(
    config: &ApiConfig,
    messages: Vec<Value>,
    web_search_enabled: bool,
) -> Result<ChatCompletionResult, String> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let api_messages: Vec<Message> = messages
        .into_iter()
        .map(|m| {
            let role = m["role"].as_str().unwrap_or("user").to_string();
            let content = m["content"].clone();
            Message { role, content }
        })
        .collect();

    let body = ChatRequest {
        model: config.model.clone(),
        messages: api_messages,
        stream: false,
        web_search: if web_search_enabled {
            Some(serde_json::json!({"enable": true}))
        } else {
            None
        },
    };
    let request_body =
        serde_json::to_value(&body).unwrap_or_else(|_| serde_json::json!({"serialize_error": true}));
    let request_id = uuid::Uuid::new_v4().to_string();
    let started_at = Instant::now();

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    let resp = match resp {
        Ok(resp) => resp,
        Err(error) => {
            append_llm_log(LlmRequestLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                request_id,
                endpoint: url,
                model: config.model.clone(),
                web_search_enabled,
                request_body,
                status_code: None,
                duration_ms: started_at.elapsed().as_millis(),
                success: false,
                error: Some(format!("API 请求失败: {}", error)),
                response_text: None,
            });
            return Err(format!("API 请求失败: {}", error));
        }
    };

    let status = resp.status();
    let status_code = status.as_u16();
    let resp_text_result = resp
        .text()
        .await;

    let resp_text = match resp_text_result {
        Ok(resp_text) => resp_text,
        Err(error) => {
            append_llm_log(LlmRequestLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                request_id,
                endpoint: url,
                model: config.model.clone(),
                web_search_enabled,
                request_body,
                status_code: Some(status_code),
                duration_ms: started_at.elapsed().as_millis(),
                success: false,
                error: Some(format!("读取响应失败: {}", error)),
                response_text: None,
            });
            return Err(format!("读取响应失败: {}", error));
        }
    };

    if !status.is_success() {
        append_llm_log(LlmRequestLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id,
            endpoint: url,
            model: config.model.clone(),
            web_search_enabled,
            request_body,
            status_code: Some(status_code),
            duration_ms: started_at.elapsed().as_millis(),
            success: false,
            error: Some(format!("API 返回错误 ({})", status)),
            response_text: Some(resp_text.clone()),
        });
        return Err(format!("API 返回错误 ({}): {}", status, resp_text));
    }

    let chat_resp: ChatResponse = match serde_json::from_str(&resp_text) {
        Ok(chat_resp) => chat_resp,
        Err(error) => {
            append_llm_log(LlmRequestLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                request_id,
                endpoint: url,
                model: config.model.clone(),
                web_search_enabled,
                request_body,
                status_code: Some(status_code),
                duration_ms: started_at.elapsed().as_millis(),
                success: false,
                error: Some(format!("解析响应失败: {}", error)),
                response_text: Some(resp_text.clone()),
            });
            return Err(format!("解析响应失败: {}", error));
        }
    };

    let content = chat_resp
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .ok_or_else(|| "API 返回空响应".to_string());

    match content {
        Ok(content) => {
            append_llm_log(LlmRequestLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                request_id: request_id.clone(),
                endpoint: url.clone(),
                model: config.model.clone(),
                web_search_enabled,
                request_body,
                status_code: Some(status_code),
                duration_ms: started_at.elapsed().as_millis(),
                success: true,
                error: None,
                response_text: Some(resp_text.clone()),
            });
            Ok(ChatCompletionResult {
                content,
                debug: LlmDebugResponse {
                    request_id,
                    endpoint: url,
                    model: config.model.clone(),
                    web_search_enabled,
                    duration_ms: started_at.elapsed().as_millis(),
                    response_text: resp_text,
                },
            })
        }
        Err(error) => {
            append_llm_log(LlmRequestLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                request_id,
                endpoint: url,
                model: config.model.clone(),
                web_search_enabled,
                request_body,
                status_code: Some(status_code),
                duration_ms: started_at.elapsed().as_millis(),
                success: false,
                error: Some(error.clone()),
                response_text: Some(resp_text),
            });
            Err(error)
        }
    }
}

pub async fn chat_completion(config: &ApiConfig, messages: Vec<Value>) -> Result<Value, String> {
    chat_completion_with_options(config, messages, true).await
}

pub fn read_llm_logs(limit: usize) -> Result<Vec<LlmRequestLogEntry>, String> {
    let path = llm_log_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path).map_err(|error| format!("读取 LLM 日志失败: {}", error))?;
    let mut entries: Vec<LlmRequestLogEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<LlmRequestLogEntry>(line).ok())
        .collect();
    if limit > 0 && entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }
    Ok(entries)
}

fn append_llm_log(entry: LlmRequestLogEntry) {
    let Ok(path) = llm_log_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let Ok(line) = serde_json::to_string(&entry) else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

fn llm_log_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "无法获取 HOME 目录".to_string())?;
    #[cfg(target_os = "macos")]
    {
        return Ok(
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.deepseekx.desktop")
                .join("logs")
                .join("llm-requests.jsonl"),
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PathBuf::from(home).join(".deepseek").join("logs").join("llm-requests.jsonl"))
    }
}

/// 从 ContentBlock 数组（Value 类型）提取纯文本
/// 用于前端显示
pub fn extract_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => {
            let texts: Vec<String> = blocks
                .iter()
                .filter_map(|block| {
                    let t = block.get("type")?.as_str()?;
                    match t {
                        "text" => block.get("text")?.as_str().map(|s| s.to_string()),
                        "search_result" => {
                            // 将搜索结果格式化为可读文本
                            let title = block.get("title").and_then(|v| v.as_str()).unwrap_or("");
                            let url = block.get("url").and_then(|v| v.as_str()).unwrap_or("");
                            let snippet =
                                block.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            Some(format!("🔍 [{}]({})\n{}", title, url, snippet))
                        }
                        "search" => {
                            // web_search 工具调用中的搜索结果
                            block
                                .get("content")
                                .and_then(|v| v.as_str())
                                .map(|s| format!("🌐 搜索结果:\n{}", s))
                        }
                        _ => None,
                    }
                })
                .collect();
            texts.join("\n")
        }
        _ => content.to_string(),
    }
}

/// 从 AI 回复文本中提取 file: 标记的代码块
/// 匹配格式: ```lang file:相对路径\n...\n```
/// 返回 Vec<(文件相对路径, 代码内容)>
pub fn parse_file_blocks(text: &str) -> Vec<(String, String)> {
    let mut results: Vec<(String, String)> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // 匹配包含 ``` 和 file: 的行
        // 格式: ```lang file:相对路径  或  ```file:相对路径  或  文字 ```file:路径
        if line.contains("```") && line.contains("file:") {
            // 提取 file: 后的路径（去除可能的语言标识符）
            let file_path = if let Some(pos) = line.find("file:") {
                line[pos + 5..].trim().to_string()
            } else {
                String::new()
            };

            // 安全检查：路径不为空，不含 ".."，不以 "/" 开头
            if file_path.is_empty() || file_path.contains("..") || file_path.starts_with('/') {
                i += 1;
                continue;
            }

            // 收集代码块内容，直到遇到闭合的 ```
            i += 1;
            let mut code_lines: Vec<&str> = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            let code = code_lines.join("\n");
            if !code.is_empty() {
                results.push((file_path, code));
            }
        }
        i += 1;
    }
    results
}

/// 将文件写入工作目录，返回成功写入的文件名列表
pub fn write_files_to_disk(
    workspace_root: &str,
    files: &[(String, String)],
) -> Result<Vec<String>, String> {
    let root = std::path::Path::new(workspace_root);
    let mut written: Vec<String> = Vec::new();

    for (rel_path, content) in files {
        // 安全检查：相对路径不能包含 ".."
        if rel_path.contains("..") {
            return Err(format!("不安全路径: {}", rel_path));
        }
        if rel_path.starts_with('/') {
            return Err(format!("不允许绝对路径: {}", rel_path));
        }

        let full_path = resolve_workspace_path(root, rel_path)
            .map_err(|e| format!("不安全路径 ({}): {}", rel_path, e))?;

        // 创建父目录
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败 ({}): {}", parent.display(), e))?;
        }

        // 写入文件
        std::fs::write(&full_path, content)
            .map_err(|e| format!("写入文件失败 ({}): {}", full_path.display(), e))?;

        written.push(rel_path.clone());
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::write_files_to_disk;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn make_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deepseekx-api-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_safe_files_inside_workspace() {
        let workspace = make_temp_dir();
        let written = write_files_to_disk(
            &workspace.to_string_lossy(),
            &[("src/main.rs".to_string(), "fn main() {}\n".to_string())],
        )
        .unwrap();

        assert_eq!(written, vec!["src/main.rs".to_string()]);
        assert_eq!(
            fs::read_to_string(workspace.join("src").join("main.rs")).unwrap(),
            "fn main() {}\n"
        );

        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn rejects_symlink_escape_outside_workspace() {
        let workspace = make_temp_dir();
        let outside = make_temp_dir();
        symlink(&outside, workspace.join("escape")).unwrap();

        let err = write_files_to_disk(
            &workspace.to_string_lossy(),
            &[("escape/payload.txt".to_string(), "owned".to_string())],
        )
        .unwrap_err();

        assert!(err.contains("outside workspace"));
        assert!(!outside.join("payload.txt").exists());

        fs::remove_dir_all(workspace).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}

// ---------- MCP 查询 ----------

/// 返回 config.toml 中定义的 MCP 服务器名称列表
pub fn list_available_mcp_servers() -> Vec<String> {
    load_config()
        .map(|config| config.mcp.servers.into_keys().collect())
        .unwrap_or_default()
}

pub fn get_mcp_server_configs(selected_names: &[String]) -> Result<Vec<McpServerConfig>, String> {
    let config = load_config()?;
    let mut resolved = Vec::new();

    for name in selected_names {
        let Some(server) = config.mcp.servers.get(name) else {
            continue;
        };
        let command = server.command.trim();
        if command.is_empty() {
            continue;
        }

        resolved.push(McpServerConfig {
            name: name.clone(),
            command: command.to_string(),
            args: server.args.clone(),
            env: server.env.clone(),
        });
    }

    Ok(resolved)
}
