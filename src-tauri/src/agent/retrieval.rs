use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::agent::actions::{ContextSource, RetrievalIntent};
use crate::api::{self, ApiConfig, LlmDebugResponse};
const MAX_SNIPPET_CHARS: usize = 320;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ContextHit {
    pub source_type: String,
    pub title: String,
    pub location: String,
    pub snippet: String,
    pub confidence: f32,
    pub freshness: String,
    pub next_hint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RetrievalResponse {
    pub hits: Vec<ContextHit>,
    pub unavailable_sources: Vec<String>,
    pub llm_debug_responses: Vec<LlmDebugResponse>,
}

pub async fn retrieve_context(
    config: Option<&ApiConfig>,
    workspace_root: &str,
    query: &str,
    intent: &RetrievalIntent,
    preferred_sources: Option<&[ContextSource]>,
    max_results: usize,
    user_knowledge_base_paths: &[String],
) -> Result<RetrievalResponse, String> {
    let sources = expand_sources(preferred_sources);
    let mut hits: Vec<ContextHit> = Vec::new();
    let mut unavailable_sources: Vec<String> = Vec::new();
    let mut llm_debug_responses: Vec<LlmDebugResponse> = Vec::new();

    for source in sources {
        match source {
            ContextSource::WorkspaceCode => {
                hits.extend(
                    scan_source(workspace_root, query, SourceKind::Code, max_results).await?,
                );
            }
            ContextSource::WorkspaceDocs => {
                hits.extend(
                    scan_source(workspace_root, query, SourceKind::Docs, max_results).await?,
                );
            }
            ContextSource::UserKnowledgeBase => {
                match scan_user_knowledge_base(query, max_results, user_knowledge_base_paths).await
                {
                    Ok(mut kb_hits) if !kb_hits.is_empty() => hits.append(&mut kb_hits),
                    _ => unavailable_sources.push("user_knowledge_base".into()),
                }
            }
            ContextSource::WebSearch => {
                match search_web_context(config, query, max_results).await {
                    Ok((web_hits, mut debug_entries)) => {
                        hits.extend(web_hits);
                        llm_debug_responses.append(&mut debug_entries);
                    }
                    Err(_) => unavailable_sources.push("web_search".into()),
                }
            }
        }
    }

    rerank_hits(&mut hits, intent);
    dedupe_hits(&mut hits);
    hits.truncate(max_results);

    Ok(RetrievalResponse {
        hits,
        unavailable_sources,
        llm_debug_responses,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Code,
    Docs,
}

async fn scan_source(
    workspace_root: &str,
    query: &str,
    kind: SourceKind,
    max_results: usize,
) -> Result<Vec<ContextHit>, String> {
    let workspace_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("workspace root invalid ({}): {}", workspace_root, e))?;

    let tokens = normalize_query_tokens(query);
    let mut hits: Vec<ContextHit> = Vec::new();
    for entry in WalkDir::new(&workspace_root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(&workspace_root)
            .map_err(|e| format!("path strip failed: {}", e))?
            .to_string_lossy()
            .to_string();

        if !matches_source_kind(&rel, kind) {
            continue;
        }

        let content = match tokio::fs::read_to_string(entry.path()).await {
            Ok(content) => content,
            Err(_) => continue,
        };

        if let Some((snippet, confidence)) = match_content(&rel, &content, &tokens) {
            hits.push(ContextHit {
                source_type: source_name(kind).into(),
                title: rel
                    .rsplit('/')
                    .next()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| rel.clone()),
                location: rel.clone(),
                snippet,
                confidence,
                freshness: "local_workspace".into(),
                next_hint: format!("read_files:{}", rel),
            });
        }

        if hits.len() >= max_results.saturating_mul(3) {
            break;
        }
    }

    hits.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(Ordering::Equal)
    });
    hits.truncate(max_results.saturating_mul(2));
    Ok(hits)
}

async fn scan_user_knowledge_base(
    query: &str,
    max_results: usize,
    configured_paths: &[String],
) -> Result<Vec<ContextHit>, String> {
    let mut roots = configured_paths
        .iter()
        .map(|value| Path::new(value))
        .filter(|path| path.exists())
        .map(|path| path.to_path_buf())
        .collect::<Vec<_>>();

    if roots.is_empty() {
        if let Ok(home) = std::env::var("HOME") {
            let fallback = Path::new(&home).join(".deepseek").join("knowledge-base");
            if fallback.exists() {
                roots.push(fallback);
            }
        }
    }

    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let tokens = normalize_query_tokens(query);
    let mut hits = Vec::new();
    for root in roots {
        let root = match std::fs::canonicalize(&root) {
            Ok(root) => root,
            Err(_) => continue,
        };

        if root.is_file() {
            if let Some(hit) = scan_knowledge_base_file(&root, &root, &tokens).await {
                hits.push(hit);
            }
            continue;
        }

        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| !is_ignored(entry.path()))
            .filter_map(|entry| entry.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            if let Some(hit) = scan_knowledge_base_file(entry.path(), &root, &tokens).await {
                hits.push(hit);
            }

            if hits.len() >= max_results.saturating_mul(3) {
                break;
            }
        }
    }

    hits.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(Ordering::Equal)
    });
    hits.truncate(max_results.saturating_mul(2));
    Ok(hits)
}

async fn scan_knowledge_base_file(
    path: &Path,
    root: &Path,
    tokens: &[String],
) -> Option<ContextHit> {
    if !matches_knowledge_base_kind(path) {
        return None;
    }

    let content = read_knowledge_base_content(path).await.ok()?;
    let display_path = path
        .strip_prefix(root)
        .ok()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let (snippet, confidence) = match_content(&display_path, &content, tokens)?;
    Some(ContextHit {
        source_type: "user_knowledge_base".into(),
        title: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&display_path)
            .to_string(),
        location: path.to_string_lossy().to_string(),
        snippet,
        confidence,
        freshness: "local_knowledge_base".into(),
        next_hint: display_path,
    })
}

async fn read_knowledge_base_content(path: &Path) -> Result<String, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if extension == "pdf" {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|error| format!("read pdf bytes failed: {}", error))?;
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    tokio::fs::read_to_string(path)
        .await
        .map_err(|error| format!("read knowledge base file failed: {}", error))
}

async fn search_web_context(
    config: Option<&ApiConfig>,
    query: &str,
    max_results: usize,
) -> Result<(Vec<ContextHit>, Vec<LlmDebugResponse>), String> {
    let config = config.ok_or_else(|| "web search config unavailable".to_string())?;
    let messages = vec![
        json!({
            "role": "system",
            "content": "You are DeepSeekX web retrieval. Search the live web and return only JSON. Preferred shape: {\"hits\":[{\"title\":\"...\",\"url\":\"https://...\",\"snippet\":\"...\"}]}. Keep snippets concise and factual."
        }),
        json!({
            "role": "user",
            "content": format!(
                "Search the web for: {}. Return at most {} hits as JSON only.",
                query, max_results
            )
        }),
    ];

    let response = api::chat_completion_with_debug(config, messages, true).await?;
    let mut hits = parse_web_search_blocks(&response.content, max_results);
    if hits.is_empty() {
        let raw_text = api::extract_text(&response.content);
        hits = parse_web_search_text(&raw_text, max_results)?;
    }

    Ok((hits, vec![response.debug]))
}

fn parse_web_search_blocks(content: &Value, max_results: usize) -> Vec<ContextHit> {
    let Some(blocks) = content.as_array() else {
        return Vec::new();
    };

    blocks
        .iter()
        .filter_map(|block| {
            if block.get("type").and_then(|value| value.as_str()) != Some("search_result") {
                return None;
            }

            let url = block.get("url").and_then(|value| value.as_str())?;
            let title = block
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or(url);
            let snippet = block
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap_or_default();

            Some(ContextHit {
                source_type: "web_search".into(),
                title: title.to_string(),
                location: url.to_string(),
                snippet: truncate(snippet, MAX_SNIPPET_CHARS),
                confidence: 3.0,
                freshness: "live_web".into(),
                next_hint: url.to_string(),
            })
        })
        .take(max_results)
        .collect()
}

fn parse_web_search_text(raw_text: &str, max_results: usize) -> Result<Vec<ContextHit>, String> {
    let candidate = strip_code_fence(raw_text).trim();
    let value: Value = serde_json::from_str(candidate)
        .map_err(|error| format!("web search json parse failed: {}", error))?;

    let items = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(object) => object
            .get("hits")
            .and_then(|hits| hits.as_array())
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    Ok(items
        .into_iter()
        .filter_map(|item| {
            let url = item.get("url").and_then(|value| value.as_str())?;
            let title = item
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or(url);
            let snippet = item
                .get("snippet")
                .or_else(|| item.get("content"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();

            Some(ContextHit {
                source_type: "web_search".into(),
                title: title.to_string(),
                location: url.to_string(),
                snippet: truncate(snippet, MAX_SNIPPET_CHARS),
                confidence: 2.5,
                freshness: "live_web".into(),
                next_hint: url.to_string(),
            })
        })
        .take(max_results)
        .collect())
}

fn expand_sources(preferred_sources: Option<&[ContextSource]>) -> Vec<ContextSource> {
    if let Some(preferred_sources) = preferred_sources {
        if !preferred_sources.is_empty() {
            return preferred_sources.to_vec();
        }
    }

    vec![ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs]
}

fn matches_source_kind(path: &str, kind: SourceKind) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match kind {
        SourceKind::Code => {
            let code_exts = [
                "rs", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml", "go", "py", "sh",
                "css", "html", "mdx",
            ];
            code_exts.contains(&extension.as_str())
                && !path.starts_with("docs/")
                && !path.ends_with(".md")
        }
        SourceKind::Docs => {
            path.starts_with("docs/")
                || path.eq_ignore_ascii_case("readme.md")
                || extension == "md"
                || extension == "txt"
        }
    }
}

fn matches_knowledge_base_kind(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    matches!(
        extension.as_str(),
        "md" | "markdown" | "txt" | "html" | "htm" | "json" | "mdx" | "csv" | "pdf"
    )
}

fn match_content(path: &str, content: &str, tokens: &[String]) -> Option<(String, f32)> {
    let path_lc = path.to_ascii_lowercase();
    let content_lc = content.to_ascii_lowercase();

    let mut score = 0f32;
    for token in tokens {
        if path_lc.contains(token) {
            score += 2.0;
        }
        if content_lc.contains(token) {
            score += 1.0;
        }
    }

    if score <= 0.0 {
        return None;
    }

    let snippet = if let Some(token) = tokens.first() {
        snippet_around(&content_lc, content, token)
    } else {
        truncate(content, MAX_SNIPPET_CHARS)
    };

    Some((snippet, score))
}

fn rerank_hits(hits: &mut [ContextHit], intent: &RetrievalIntent) {
    let docs_bias = matches!(
        intent,
        RetrievalIntent::FindDocumentation | RetrievalIntent::PrepareReport
    );
    let code_bias = matches!(
        intent,
        RetrievalIntent::UnderstandExistingSystem
            | RetrievalIntent::LocateImplementation
            | RetrievalIntent::GatherEvidence
    );

    hits.sort_by(|left, right| {
        let left_bonus = source_bonus(left, docs_bias, code_bias);
        let right_bonus = source_bonus(right, docs_bias, code_bias);
        (right.confidence + right_bonus)
            .partial_cmp(&(left.confidence + left_bonus))
            .unwrap_or(Ordering::Equal)
    });
}

fn source_bonus(hit: &ContextHit, docs_bias: bool, code_bias: bool) -> f32 {
    match hit.source_type.as_str() {
        "workspace_docs" if docs_bias => 1.5,
        "workspace_code" if code_bias => 1.5,
        _ => 0.0,
    }
}

fn dedupe_hits(hits: &mut Vec<ContextHit>) {
    let mut seen = HashSet::new();
    hits.retain(|hit| seen.insert(hit.location.clone()));
}

fn normalize_query_tokens(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|token| token.len() >= 2)
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn source_name(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Code => "workspace_code",
        SourceKind::Docs => "workspace_docs",
    }
}

fn snippet_around(content_lc: &str, content: &str, token: &str) -> String {
    if let Some(index) = content_lc.find(token) {
        let start = content[..index]
            .chars()
            .count()
            .saturating_sub(MAX_SNIPPET_CHARS / 3);
        let snippet: String = content
            .chars()
            .skip(start)
            .take(MAX_SNIPPET_CHARS)
            .collect();
        truncate(&snippet, MAX_SNIPPET_CHARS)
    } else {
        truncate(content, MAX_SNIPPET_CHARS)
    }
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist"
    })
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let truncated: String = value.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

fn strip_code_fence(raw_text: &str) -> &str {
    let trimmed = raw_text.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.strip_suffix("```").unwrap_or(stripped).trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.strip_suffix("```").unwrap_or(stripped).trim();
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_query_tokens, parse_web_search_blocks, parse_web_search_text, retrieve_context,
        ContextHit,
    };
    use crate::agent::actions::{ContextSource, RetrievalIntent};
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    fn make_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("deepseekx-retrieval-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tokenizes_query_into_search_terms() {
        let tokens = normalize_query_tokens("agent orchestrator planner permissions");
        assert_eq!(
            tokens,
            vec!["agent", "orchestrator", "planner", "permissions"]
        );
    }

    #[tokio::test]
    async fn retrieves_hits_from_code_and_docs() {
        let workspace = make_temp_dir();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::create_dir_all(workspace.join("docs")).unwrap();
        fs::write(
            workspace.join("src").join("agent.rs"),
            "pub fn orchestrator() { /* planner */ }",
        )
        .unwrap();
        fs::write(
            workspace.join("docs").join("agent.md"),
            "The planner coordinates the orchestrator.",
        )
        .unwrap();

        let response = retrieve_context(
            None,
            &workspace.to_string_lossy(),
            "planner orchestrator",
            &RetrievalIntent::UnderstandExistingSystem,
            Some(&[ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs]),
            8,
            &[],
        )
        .await
        .unwrap();

        assert!(!response.hits.is_empty());
        assert!(response
            .hits
            .iter()
            .any(|hit| hit.location == "src/agent.rs"));
        assert!(response
            .hits
            .iter()
            .any(|hit| hit.location == "docs/agent.md"));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn context_hit_is_serializable() {
        let hit = ContextHit {
            source_type: "workspace_code".into(),
            title: "agent.rs".into(),
            location: "src/agent.rs".into(),
            snippet: "planner".into(),
            confidence: 3.0,
            freshness: "local_workspace".into(),
            next_hint: "read_files:src/agent.rs".into(),
        };
        let raw = serde_json::to_string(&hit).unwrap();
        assert!(raw.contains("workspace_code"));
    }

    #[test]
    fn parses_web_search_json_text() {
        let hits = parse_web_search_text(
            r#"{"hits":[{"title":"DeepSeekX","url":"https://example.com/deepseekx","snippet":"Latest project update"}]}"#,
            4,
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].location, "https://example.com/deepseekx");
        assert_eq!(hits[0].source_type, "web_search");
    }

    #[test]
    fn parses_web_search_blocks() {
        let hits = parse_web_search_blocks(
            &json!([
                {
                    "type": "search_result",
                    "title": "DeepSeekX docs",
                    "url": "https://example.com/docs",
                    "content": "Docs snippet"
                }
            ]),
            4,
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "DeepSeekX docs");
        assert_eq!(hits[0].next_hint, "https://example.com/docs");
    }

    #[tokio::test]
    async fn retrieves_hits_from_user_knowledge_base() {
        let workspace = make_temp_dir();
        let knowledge_base = make_temp_dir();
        fs::write(
            knowledge_base.join("reference.md"),
            "DeepSeekX backlog notes about retrieval sources and planner behavior.",
        )
        .unwrap();

        let response = retrieve_context(
            None,
            &workspace.to_string_lossy(),
            "retrieval planner",
            &RetrievalIntent::FindDocumentation,
            Some(&[ContextSource::UserKnowledgeBase]),
            8,
            &[knowledge_base.to_string_lossy().to_string()],
        )
        .await
        .unwrap();

        assert_eq!(response.unavailable_sources.len(), 0);
        assert!(response
            .hits
            .iter()
            .any(|hit| hit.source_type == "user_knowledge_base"));

        fs::remove_dir_all(workspace).unwrap();
        fs::remove_dir_all(knowledge_base).unwrap();
    }
}
