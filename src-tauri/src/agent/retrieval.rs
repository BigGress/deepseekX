use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;
use walkdir::WalkDir;

use crate::agent::actions::{ContextSource, RetrievalIntent};
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
}

pub async fn retrieve_context(
    workspace_root: &str,
    query: &str,
    intent: &RetrievalIntent,
    preferred_sources: Option<&[ContextSource]>,
    max_results: usize,
) -> Result<RetrievalResponse, String> {
    let sources = expand_sources(preferred_sources);
    let mut hits: Vec<ContextHit> = Vec::new();
    let mut unavailable_sources: Vec<String> = Vec::new();

    for source in sources {
        match source {
            ContextSource::WorkspaceCode => {
                hits.extend(scan_source(workspace_root, query, SourceKind::Code, max_results).await?);
            }
            ContextSource::WorkspaceDocs => {
                hits.extend(scan_source(workspace_root, query, SourceKind::Docs, max_results).await?);
            }
            ContextSource::UserKnowledgeBase => {
                unavailable_sources.push("user_knowledge_base".into());
            }
            ContextSource::WebSearch => {
                unavailable_sources.push("web_search".into());
            }
        }
    }

    rerank_hits(&mut hits, intent);
    dedupe_hits(&mut hits);
    hits.truncate(max_results);

    Ok(RetrievalResponse {
        hits,
        unavailable_sources,
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
                "rs", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml", "go", "py",
                "sh", "css", "html", "mdx",
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
        let snippet: String = content.chars().skip(start).take(MAX_SNIPPET_CHARS).collect();
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

#[cfg(test)]
mod tests {
    use super::{normalize_query_tokens, retrieve_context, ContextHit};
    use crate::agent::actions::{ContextSource, RetrievalIntent};
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
        assert_eq!(tokens, vec!["agent", "orchestrator", "planner", "permissions"]);
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
            &workspace.to_string_lossy(),
            "planner orchestrator",
            &RetrievalIntent::UnderstandExistingSystem,
            Some(&[ContextSource::WorkspaceCode, ContextSource::WorkspaceDocs]),
            8,
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
}
