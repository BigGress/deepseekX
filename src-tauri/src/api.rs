use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::agent::permissions::resolve_workspace_path;

// ---------- 配置 ----------

#[derive(Debug, Deserialize)]
struct DeepSeekConfig {
    api_key: Option<String>,
    base_url: Option<String>,
    default_text_model: Option<String>,
    #[serde(default)]
    providers: Providers,
}

#[derive(Debug, Default, Deserialize)]
struct Providers {
    deepseek: Option<ProviderConfig>,
}

#[derive(Debug, Deserialize)]
struct ProviderConfig {
    api_key: Option<String>,
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

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    let status = resp.status();
    let resp_text = resp.text().await.map_err(|e| format!("读取响应失败: {}", e))?;

    if !status.is_success() {
        return Err(format!("API 返回错误 ({}): {}", status, resp_text));
    }

    let chat_resp: ChatResponse =
        serde_json::from_str(&resp_text).map_err(|e| format!("解析响应失败: {}", e))?;

    chat_resp
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .ok_or_else(|| "API 返回空响应".to_string())
}

pub async fn chat_completion(
    config: &ApiConfig,
    messages: Vec<Value>,
) -> Result<Value, String> {
    chat_completion_with_options(config, messages, true).await
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
                            let snippet = block.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            Some(format!("🔍 [{}]({})\n{}", title, url, snippet))
                        }
                        "search" => {
                            // web_search 工具调用中的搜索结果
                            block.get("content").and_then(|v| v.as_str()).map(|s| {
                                format!("🌐 搜索结果:\n{}", s)
                            })
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
    let mut servers: Vec<String> = Vec::new();

    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return servers,
    };

    let config_path = PathBuf::from(&home).join(".deepseek").join("config.toml");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        // 简单解析 [mcp.servers.xxx] 段中的服务器名称
        for line in content.lines() {
            let line = line.trim();
            // 匹配 [mcp.servers.xxx] 格式
            if line.starts_with("[mcp.servers.") && line.ends_with(']') {
                let name = &line["[mcp.servers.".len()..line.len() - 1];
                if !name.is_empty() {
                    servers.push(name.to_string());
                }
            }
        }
    }
    servers
}
