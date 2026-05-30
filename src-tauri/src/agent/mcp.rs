use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::api::McpServerConfig;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct McpToolDescriptor {
    pub server: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct McpDiscoveryResult {
    pub tools: Vec<McpToolDescriptor>,
    pub unavailable_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct McpToolCallResult {
    pub server: String,
    pub tool: String,
    pub content: String,
    pub is_error: bool,
}

pub async fn discover_tools(
    server_configs: &[McpServerConfig],
) -> Result<McpDiscoveryResult, String> {
    let mut tools = Vec::new();
    let mut unavailable_servers = Vec::new();

    for server in server_configs {
        match list_server_tools(server).await {
            Ok(mut entries) => tools.append(&mut entries),
            Err(error) => unavailable_servers.push(format!("{}: {}", server.name, error)),
        }
    }

    Ok(McpDiscoveryResult {
        tools,
        unavailable_servers,
    })
}

pub async fn call_tool(
    server_configs: &[McpServerConfig],
    server_name: &str,
    tool_name: &str,
    arguments: &Value,
) -> Result<McpToolCallResult, String> {
    let server = server_configs
        .iter()
        .find(|server| server.name == server_name)
        .ok_or_else(|| format!("unknown mcp server: {}", server_name))?;

    let mut client = McpClient::start(server).await?;
    client.initialize().await?;

    let result = client
        .request(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments,
            }),
        )
        .await?;

    let content = if let Some(items) = result.get("content").and_then(|value| value.as_array()) {
        items
            .iter()
            .map(format_content_item)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
    };

    Ok(McpToolCallResult {
        server: server_name.to_string(),
        tool: tool_name.to_string(),
        content,
        is_error: result
            .get("isError")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    })
}

async fn list_server_tools(server: &McpServerConfig) -> Result<Vec<McpToolDescriptor>, String> {
    let mut client = McpClient::start(server).await?;
    client.initialize().await?;
    let result = client.request("tools/list", json!({})).await?;
    let tools = result
        .get("tools")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "tools/list missing tools array".to_string())?;

    Ok(tools
        .iter()
        .filter_map(|tool| {
            let name = tool.get("name")?.as_str()?.to_string();
            Some(McpToolDescriptor {
                server: server.name.clone(),
                name,
                description: tool
                    .get("description")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string()),
                input_schema: tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            })
        })
        .collect())
}

fn format_content_item(item: &Value) -> String {
    match item.get("type").and_then(|value| value.as_str()) {
        Some("text") => item
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        Some(other) => {
            let payload = serde_json::to_string_pretty(item).unwrap_or_else(|_| item.to_string());
            format!("[{}]\n{}", other, payload)
        }
        None => serde_json::to_string_pretty(item).unwrap_or_else(|_| item.to_string()),
    }
}

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr_capture: Arc<Mutex<String>>,
    next_id: u64,
}

impl McpClient {
    async fn start(server: &McpServerConfig) -> Result<Self, String> {
        let command_path = resolve_program_path(&server.command);
        let mut command = Command::new(&command_path);
        command.args(&server.args);
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        for (key, value) in build_env_map(&server.env) {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to spawn mcp server {} ({}): {}",
                server.name, server.command, error
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "mcp server stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "mcp server stdout unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "mcp server stderr unavailable".to_string())?;
        let stderr_capture = Arc::new(Mutex::new(String::new()));
        let stderr_capture_task = Arc::clone(&stderr_capture);
        tokio::spawn(async move {
            let mut stderr = stderr;
            let mut buffer = Vec::new();
            let _ = stderr.read_to_end(&mut buffer).await;
            if let Ok(text) = String::from_utf8(buffer) {
                if let Ok(mut captured) = stderr_capture_task.lock() {
                    *captured = text;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            stderr_capture,
            next_id: 1,
        })
    }

    async fn initialize(&mut self) -> Result<(), String> {
        let _ = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "DeepSeekX",
                        "version": "0.1.0",
                    }
                }),
            )
            .await?;
        self.send_notification("notifications/initialized", json!({}))
            .await?;
        Ok(())
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_message(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .await?;

        loop {
            let message = self.read_message().await?;
            match message.get("id").and_then(|value| value.as_u64()) {
                Some(response_id) if response_id == id => {
                    if let Some(error) = message.get("error") {
                        return Err(format!(
                            "mcp {} failed: {}",
                            method,
                            serde_json::to_string(error).unwrap_or_else(|_| error.to_string())
                        ));
                    }
                    return Ok(message.get("result").cloned().unwrap_or_else(|| json!({})));
                }
                _ => continue,
            }
        }
    }

    async fn send_notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send_message(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
        .await
    }

    async fn send_message(&mut self, value: &Value) -> Result<(), String> {
        let payload =
            serde_json::to_vec(value).map_err(|error| format!("mcp encode failed: {}", error))?;
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        self.stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|error| format!("mcp write header failed: {}", error))?;
        self.stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("mcp write payload failed: {}", error))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| format!("mcp flush failed: {}", error))
    }

    async fn read_message(&mut self) -> Result<Value, String> {
        let mut content_length: Option<usize> = None;

        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|error| format!("mcp read header failed: {}", error))?;
            if read == 0 {
                return Err(self.eof_error("mcp server closed stdout while reading headers"));
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }

            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                let parsed = rest.trim().parse::<usize>().map_err(|error| {
                    format!("invalid mcp content-length {}: {}", rest.trim(), error)
                })?;
                content_length = Some(parsed);
            }
        }

        let length =
            content_length.ok_or_else(|| "mcp missing Content-Length header".to_string())?;
        let mut payload = vec![0u8; length];
        self.stdout
            .read_exact(&mut payload)
            .await
            .map_err(|error| format!("mcp read payload failed: {}", error))?;
        serde_json::from_slice(&payload).map_err(|error| format!("mcp decode failed: {}", error))
    }

    fn eof_error(&mut self, prefix: &str) -> String {
        let stderr = self
            .stderr_capture
            .lock()
            .ok()
            .map(|captured| captured.trim().to_string())
            .filter(|captured| !captured.is_empty());

        match self.child.try_wait() {
            Ok(Some(status)) => match stderr {
                Some(stderr) => format!("{} (status: {}; stderr: {})", prefix, status, stderr),
                None => format!("{} (status: {})", prefix, status),
            },
            Ok(None) => match stderr {
                Some(stderr) => format!("{} (stderr: {})", prefix, stderr),
                None => prefix.to_string(),
            },
            Err(error) => match stderr {
                Some(stderr) => format!("{} ({}; stderr: {})", prefix, error, stderr),
                None => format!("{} ({})", prefix, error),
            },
        }
    }
}

fn resolve_program_path(command: &str) -> String {
    if PathBuf::from(command).is_absolute() {
        return command.to_string();
    }

    which::which_in(command, Some(&build_search_path()), ".")
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| command.to_string())
}

fn build_search_path() -> String {
    let mut paths: Vec<String> = std::env::var("PATH")
        .ok()
        .map(|current| current.split(':').map(|item| item.to_string()).collect())
        .unwrap_or_default();

    if let Ok(home) = std::env::var("HOME") {
        paths.push(format!("{}/.local/bin", home));
        paths.push(format!("{}/.npm-global/bin", home));
        paths.push(format!("{}/.cargo/bin", home));
        paths.push(format!("{}/.local/share/pnpm", home));
    }

    paths.push("/opt/homebrew/bin".into());
    paths.push("/usr/local/bin".into());

    let mut deduped: Vec<String> = Vec::new();
    for path in paths {
        if path.is_empty() || deduped.iter().any(|existing| existing == &path) {
            continue;
        }
        deduped.push(path);
    }

    deduped.join(":")
}

fn build_env_map(extra_env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("PATH".into(), build_search_path());
    for (key, value) in extra_env {
        env.insert(key.clone(), value.clone());
    }
    env
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{call_tool, discover_tools};
    use crate::api::McpServerConfig;

    fn make_fake_mcp_server_script() -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("deepseekx-mcp-test-{}", unique));
        fs::create_dir_all(&dir).unwrap();
        let script_path = dir.join("fake_mcp_server.py");
        fs::write(
            &script_path,
            r#"import json, sys

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode("utf-8").strip()
        if not line:
            break
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
    length = int(headers.get("content-length", "0"))
    payload = sys.stdin.buffer.read(length)
    return json.loads(payload.decode("utf-8"))

def send_message(obj):
    payload = json.dumps(obj).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(payload)}\r\n\r\n".encode("utf-8"))
    sys.stdout.buffer.write(payload)
    sys.stdout.buffer.flush()

while True:
    message = read_message()
    if message is None:
        break
    method = message.get("method")
    if method == "initialize":
        send_message({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": {"protocolVersion": "2024-11-05", "capabilities": {}}
        })
    elif method == "notifications/initialized":
        continue
    elif method == "tools/list":
        send_message({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": {
                "tools": [{
                    "name": "echo_context",
                    "description": "Echoes the provided query",
                    "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}}
                }]
            }
        })
    elif method == "tools/call":
        query = message.get("params", {}).get("arguments", {}).get("query", "")
        send_message({
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": {
                "content": [{"type": "text", "text": f"echo:{query}"}],
                "isError": False
            }
        })
"#,
        )
        .unwrap();
        script_path.to_string_lossy().to_string()
    }

    fn fake_server_config() -> McpServerConfig {
        McpServerConfig {
            name: "fake".into(),
            command: "python3".into(),
            args: vec![make_fake_mcp_server_script()],
            env: BTreeMap::new(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn discovers_tools_from_fake_server() {
        let discovery = discover_tools(&[fake_server_config()]).await.unwrap();
        assert!(discovery.unavailable_servers.is_empty());
        assert_eq!(discovery.tools.len(), 1);
        assert_eq!(discovery.tools[0].server, "fake");
        assert_eq!(discovery.tools[0].name, "echo_context");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn calls_tool_on_fake_server() {
        let result = call_tool(
            &[fake_server_config()],
            "fake",
            "echo_context",
            &json!({"query": "ping"}),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "echo:ping");
    }
}
