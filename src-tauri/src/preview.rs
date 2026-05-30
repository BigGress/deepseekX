use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewCategory {
    Text, Markdown, Code, Image, Audio, Video,
    Pdf, Html, Csv, Spreadsheet, Document, Presentation,
    Archive, Binary, Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMode {
    Structured, Rendered, Raw, Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewCapabilities {
    pub can_render_inline: bool,
    pub can_open_focused: bool,
    pub can_download: bool,
    pub can_show_text_extract: bool,
    pub can_show_original_appearance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreviewContent {
    Text    { text: String, language: Option<String> },
    Markdown { markdown: String },
    Html    { html: String, sandboxed: bool },
    Table   { columns: Vec<String>, rows: Vec<Vec<String>> },
    Media   { url: String, media_type: String },
    Pages   { pages: Vec<PreviewPage> },
    Fallback { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewPage {
    pub page: u32,
    pub image_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreviewMetadata {
    pub detected_encoding: Option<String>,
    pub line_count: Option<u64>,
    pub page_count: Option<u32>,
    pub sheet_names: Option<Vec<String>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
    pub generated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewDescriptor {
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub category: PreviewCategory,
    pub default_mode: PreviewMode,
    pub available_modes: Vec<PreviewMode>,
    pub capabilities: PreviewCapabilities,
    pub content: Option<PreviewContent>,
    pub metadata: Option<PreviewMetadata>,
    pub warnings: Vec<String>,
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

fn resolve_abs(path: &str, workspace_root: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    let abs = if p.is_absolute() { p.to_path_buf() } else { Path::new(workspace_root).join(p) };
    std::fs::canonicalize(&abs).map_err(|e| format!("路径不存在: {e}"))
}

fn validate_path(path: &str, workspace_root: &str) -> Result<PathBuf, String> {
    let abs = resolve_abs(path, workspace_root)?;
    let root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("工作区根路径无效: {e}"))?;
    if !abs.starts_with(&root) {
        return Err("路径不在工作区范围内".to_string());
    }
    Ok(abs)
}

// ─── MIME detection ───────────────────────────────────────────────────────────

fn detect_mime(path: &Path, extension: Option<&str>) -> Option<String> {
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut buf = [0u8; 8192];
        if let Ok(n) = f.read(&mut buf) {
            if let Some(kind) = infer::get(&buf[..n]) {
                return Some(kind.mime_type().to_string());
            }
        }
    }
    extension
        .and_then(|ext| mime_guess::from_ext(ext).first())
        .map(|m| m.to_string())
}

// ─── Category classification ──────────────────────────────────────────────────

pub fn classify_category(ext: Option<&str>, mime: &Option<String>) -> PreviewCategory {
    let ext = ext.unwrap_or("");
    match ext {
        "txt" | "log" => PreviewCategory::Text,
        "md" | "mdx" => PreviewCategory::Markdown,
        "htm" | "html" => PreviewCategory::Html,
        "csv" => PreviewCategory::Csv,
        "pdf" => PreviewCategory::Pdf,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp" => PreviewCategory::Image,
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => PreviewCategory::Audio,
        "mp4" | "webm" | "mov" | "avi" | "mkv" => PreviewCategory::Video,
        "xlsx" | "xls" => PreviewCategory::Spreadsheet,
        "docx" | "doc" | "rtf" => PreviewCategory::Document,
        "pptx" | "ppt" => PreviewCategory::Presentation,
        "zip" | "tar" | "gz" | "bz2" | "7z" | "rar" | "tgz" => PreviewCategory::Archive,
        "ts" | "tsx" | "js" | "jsx" | "rs" | "py" | "go" | "java" | "c" | "cpp" | "h"
        | "css" | "scss" | "json" | "yaml" | "yml" | "toml" | "xml" | "sh" | "sql"
        | "swift" | "kt" | "rb" | "php" | "lua" | "r" | "jl" => PreviewCategory::Code,
        _ => match mime.as_deref().unwrap_or("") {
            m if m.starts_with("text/") => PreviewCategory::Text,
            m if m.starts_with("image/") => PreviewCategory::Image,
            m if m.starts_with("audio/") => PreviewCategory::Audio,
            m if m.starts_with("video/") => PreviewCategory::Video,
            "application/pdf" => PreviewCategory::Pdf,
            _ => PreviewCategory::Unknown,
        },
    }
}

// ─── Mode defaults ────────────────────────────────────────────────────────────

pub fn default_modes(cat: &PreviewCategory) -> (PreviewMode, Vec<PreviewMode>) {
    match cat {
        PreviewCategory::Text | PreviewCategory::Code =>
            (PreviewMode::Raw, vec![PreviewMode::Raw, PreviewMode::Metadata]),
        PreviewCategory::Markdown =>
            (PreviewMode::Structured, vec![PreviewMode::Structured, PreviewMode::Raw, PreviewMode::Metadata]),
        PreviewCategory::Html =>
            (PreviewMode::Rendered, vec![PreviewMode::Rendered, PreviewMode::Raw, PreviewMode::Metadata]),
        PreviewCategory::Csv =>
            (PreviewMode::Structured, vec![PreviewMode::Structured, PreviewMode::Raw, PreviewMode::Metadata]),
        PreviewCategory::Image | PreviewCategory::Audio | PreviewCategory::Video | PreviewCategory::Pdf =>
            (PreviewMode::Rendered, vec![PreviewMode::Rendered, PreviewMode::Metadata]),
        PreviewCategory::Spreadsheet | PreviewCategory::Document | PreviewCategory::Presentation =>
            (PreviewMode::Structured, vec![PreviewMode::Structured, PreviewMode::Metadata]),
        _ => (PreviewMode::Metadata, vec![PreviewMode::Metadata]),
    }
}

pub fn capabilities(cat: &PreviewCategory) -> PreviewCapabilities {
    match cat {
        PreviewCategory::Text | PreviewCategory::Code | PreviewCategory::Markdown
        | PreviewCategory::Html | PreviewCategory::Csv =>
            PreviewCapabilities { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: false, can_show_original_appearance: false },
        PreviewCategory::Image =>
            PreviewCapabilities { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: false, can_show_original_appearance: true },
        PreviewCategory::Audio | PreviewCategory::Video =>
            PreviewCapabilities { can_render_inline: true, can_open_focused: false, can_download: true, can_show_text_extract: false, can_show_original_appearance: true },
        PreviewCategory::Pdf =>
            PreviewCapabilities { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: true, can_show_original_appearance: true },
        PreviewCategory::Spreadsheet | PreviewCategory::Document | PreviewCategory::Presentation =>
            PreviewCapabilities { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: true, can_show_original_appearance: false },
        _ =>
            PreviewCapabilities { can_render_inline: false, can_open_focused: false, can_download: true, can_show_text_extract: false, can_show_original_appearance: false },
    }
}

// ─── describe_file_preview ────────────────────────────────────────────────────

#[tauri::command]
pub fn describe_file_preview(path: String, workspace_root: String) -> Result<PreviewDescriptor, String> {
    let abs = validate_path(&path, &workspace_root)?;
    let meta = std::fs::metadata(&abs).map_err(|e| format!("读取文件元数据失败: {e}"))?;

    let file_name = abs.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let extension = abs.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase());
    let size_bytes = Some(meta.len());
    let mime_type = detect_mime(&abs, extension.as_deref());
    let category = classify_category(extension.as_deref(), &mime_type);
    let (default_mode, available_modes) = default_modes(&category);
    let caps = capabilities(&category);

    Ok(PreviewDescriptor {
        path,
        file_name,
        extension,
        mime_type,
        size_bytes,
        category,
        default_mode,
        available_modes,
        capabilities: caps,
        content: None,
        metadata: None,
        warnings: vec![],
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("px-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn classify_code_extensions() {
        assert!(matches!(classify_category(Some("rs"), &None), PreviewCategory::Code));
        assert!(matches!(classify_category(Some("ts"), &None), PreviewCategory::Code));
        assert!(matches!(classify_category(Some("py"), &None), PreviewCategory::Code));
    }

    #[test]
    fn classify_special_types() {
        assert!(matches!(classify_category(Some("md"), &None), PreviewCategory::Markdown));
        assert!(matches!(classify_category(Some("html"), &None), PreviewCategory::Html));
        assert!(matches!(classify_category(Some("csv"), &None), PreviewCategory::Csv));
        assert!(matches!(classify_category(Some("pdf"), &None), PreviewCategory::Pdf));
        assert!(matches!(classify_category(Some("xlsx"), &None), PreviewCategory::Spreadsheet));
        assert!(matches!(classify_category(Some("docx"), &None), PreviewCategory::Document));
        assert!(matches!(classify_category(Some("pptx"), &None), PreviewCategory::Presentation));
    }

    #[test]
    fn describe_returns_descriptor_without_content() {
        let root = tmp_dir();
        let file = root.join("hello.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let result = describe_file_preview(
            "hello.rs".to_string(),
            root.to_string_lossy().to_string(),
        ).unwrap();

        assert_eq!(result.file_name, "hello.rs");
        assert!(matches!(result.category, PreviewCategory::Code));
        assert!(result.content.is_none());
        assert_eq!(result.size_bytes, Some(12));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn describe_rejects_path_outside_workspace() {
        let root = tmp_dir();
        let err = describe_file_preview("/etc/passwd".to_string(), root.to_string_lossy().to_string());
        assert!(err.is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
