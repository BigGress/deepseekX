use std::path::{Component, Path, PathBuf};

use crate::agent::actions::{AgentAction, FetchMethod};

#[derive(Debug, Clone, PartialEq)]
pub struct CommandRiskVerdict {
    pub allowed_without_confirmation: bool,
    pub requires_confirmation: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allowed,
    RequiresConfirmation {
        reason: String,
        risk_level: &'static str,
    },
}

pub fn ensure_workspace_relative(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("path resolves outside workspace".into());
    }

    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err("path resolves outside workspace".into());
            }
        }
    }

    Ok(())
}

pub fn resolve_workspace_path(workspace_root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    ensure_workspace_relative(rel_path)?;

    let workspace_root = std::fs::canonicalize(workspace_root).map_err(|e| {
        format!(
            "workspace root invalid ({}): {}",
            workspace_root.display(),
            e
        )
    })?;

    let mut current = workspace_root.clone();
    for component in Path::new(rel_path).components() {
        let Component::Normal(part) = component else {
            return Err("path resolves outside workspace".into());
        };

        current.push(part);

        if current.exists() {
            let canonical = std::fs::canonicalize(&current)
                .map_err(|e| format!("path resolution failed ({}): {}", current.display(), e))?;
            if !canonical.starts_with(&workspace_root) {
                return Err("path resolves outside workspace".into());
            }
        }
    }

    Ok(workspace_root.join(rel_path))
}

pub fn resolve_workspace_dir(
    workspace_root: &Path,
    raw_path: Option<&str>,
) -> Result<PathBuf, String> {
    let workspace_root = std::fs::canonicalize(workspace_root).map_err(|e| {
        format!(
            "workspace root invalid ({}): {}",
            workspace_root.display(),
            e
        )
    })?;

    let Some(path) = raw_path.map(str::trim) else {
        return Ok(workspace_root);
    };

    if path.is_empty() || path == "." {
        return Ok(workspace_root);
    }

    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        resolve_workspace_path(&workspace_root, path)?
    };

    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|e| format!("cwd invalid ({}): {}", candidate.display(), e))?;

    if !canonical.starts_with(&workspace_root) {
        return Err("cwd resolves outside workspace".into());
    }

    if !canonical.is_dir() {
        return Err("cwd must be a directory inside workspace".into());
    }

    Ok(canonical)
}

pub fn evaluate_action_permission(
    workspace_root: &Path,
    action: &AgentAction,
    command_allowlist: &[String],
) -> Result<PermissionOutcome, String> {
    match action {
        AgentAction::RetrieveContext { .. }
        | AgentAction::SummarizeFindings { .. }
        | AgentAction::CallMcpTool { .. } => Ok(PermissionOutcome::Allowed),
        AgentAction::ReadFiles { files, .. } => {
            for file in files {
                let _ = resolve_workspace_path(workspace_root, file)?;
            }
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::CreateFiles { files, .. } => {
            for file in files {
                let _ = resolve_workspace_path(workspace_root, &file.path)?;
            }
            let total_bytes: usize = files.iter().map(|file| file.content.len()).sum();
            if files.len() > 50 || total_bytes > 500_000 {
                return Ok(PermissionOutcome::RequiresConfirmation {
                    reason: "创建文件数量或内容过大".into(),
                    risk_level: "high",
                });
            }
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::RenameFiles { renames, .. } => {
            for rename in renames {
                let _ = resolve_workspace_path(workspace_root, &rename.from_path)?;
                let _ = resolve_workspace_path(workspace_root, &rename.to_path)?;
            }
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::DeleteFiles { paths, .. } => {
            for path in paths {
                let _ = resolve_workspace_path(workspace_root, path)?;
            }
            Ok(PermissionOutcome::RequiresConfirmation {
                reason: "删除文件属于高风险不可逆操作".into(),
                risk_level: "high",
            })
        }
        AgentAction::ApplyPatch { patches, .. } => {
            let mut total_bytes = 0usize;
            let mut total_hunks = 0usize;
            for patch in patches {
                let _ = resolve_workspace_path(workspace_root, &patch.path)?;
                total_hunks += patch.hunks.len();
                total_bytes += patch
                    .hunks
                    .iter()
                    .map(|hunk| hunk.old_text.len() + hunk.new_text.len())
                    .sum::<usize>();
            }

            if patches.len() > 50 || total_hunks > 150 || total_bytes > 250_000 {
                return Ok(PermissionOutcome::RequiresConfirmation {
                    reason: "补丁涉及文件或 hunk 过多".into(),
                    risk_level: "high",
                });
            }

            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::WriteFiles { files, .. } => {
            for file in files {
                let _ = resolve_workspace_path(workspace_root, &file.path)?;
            }

            let total_bytes: usize = files.iter().map(|file| file.content.len()).sum();
            if files.len() > 30 || total_bytes > 500_000 {
                return Ok(PermissionOutcome::RequiresConfirmation {
                    reason: "覆盖大量文件或写入内容过大".into(),
                    risk_level: "high",
                });
            }

            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::RunCommand { command, cwd, .. } => {
            let verdict = classify_command_risk(command, command_allowlist);
            if verdict.requires_confirmation || !verdict.allowed_without_confirmation {
                return Ok(PermissionOutcome::RequiresConfirmation {
                    reason: verdict.reason,
                    risk_level: "high",
                });
            }

            let _ = resolve_workspace_dir(workspace_root, cwd.as_deref())?;
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::FetchUrl {
            url,
            method,
            max_chars,
            ..
        } => {
            validate_fetch_url(url)?;
            if matches!(method, FetchMethod::Head | FetchMethod::Get) {
                let _ = max_chars.unwrap_or(4000);
                Ok(PermissionOutcome::Allowed)
            } else {
                Err("unsupported fetch method".into())
            }
        }
        AgentAction::VerifyChecks { checks, .. } => {
            if checks.is_empty() {
                return Err("verify_checks requires at least one check".into());
            }
            for check in checks {
                let verdict = classify_command_risk(&check.command, command_allowlist);
                if verdict.requires_confirmation || !verdict.allowed_without_confirmation {
                    return Ok(PermissionOutcome::RequiresConfirmation {
                        reason: format!("验证命令 `{}` 需要确认：{}", check.label, verdict.reason),
                        risk_level: "high",
                    });
                }
                let _ = resolve_workspace_dir(workspace_root, check.cwd.as_deref())?;
            }
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::AskUser { .. } | AgentAction::Finish { .. } => Ok(PermissionOutcome::Allowed),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectCommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub fn classify_command_risk(command: &str, command_allowlist: &[String]) -> CommandRiskVerdict {
    let trimmed = command.trim();
    let normalized = trimmed.to_lowercase();

    let dangerous_markers = ["rm ", "sudo ", "git push", "brew install"];

    if has_disallowed_shell_syntax(command) {
        return CommandRiskVerdict {
            allowed_without_confirmation: false,
            requires_confirmation: true,
            reason: "compound shell syntax is not allowlisted".into(),
        };
    }

    if let Ok(spec) = parse_configured_allowlisted_command(trimmed, command_allowlist) {
        return CommandRiskVerdict {
            allowed_without_confirmation: true,
            requires_confirmation: false,
            reason: format!("configured command allowlist: {}", spec.program),
        };
    }

    if dangerous_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return CommandRiskVerdict {
            allowed_without_confirmation: false,
            requires_confirmation: true,
            reason: "dangerous command".into(),
        };
    }

    match parse_builtin_allowlisted_command(trimmed) {
        Ok(_) => CommandRiskVerdict {
            allowed_without_confirmation: true,
            requires_confirmation: false,
            reason: "safe command allowlist".into(),
        },
        Err(reason) => CommandRiskVerdict {
            allowed_without_confirmation: false,
            requires_confirmation: true,
            reason,
        },
    }
}

pub(crate) fn parse_allowlisted_command(
    command: &str,
    command_allowlist: &[String],
) -> Result<DirectCommandSpec, String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("command is outside allowlist".into());
    }

    if has_disallowed_shell_syntax(trimmed) {
        return Err("compound shell syntax is not allowlisted".into());
    }

    if let Ok(spec) = parse_configured_allowlisted_command(trimmed, command_allowlist) {
        return Ok(spec);
    }

    parse_builtin_allowlisted_command(trimmed)
}

fn parse_configured_allowlisted_command(
    command: &str,
    command_allowlist: &[String],
) -> Result<DirectCommandSpec, String> {
    let raw_tokens = tokenize_command(command)?;
    let lowered_tokens: Vec<String> = raw_tokens
        .iter()
        .map(|token| token.to_lowercase())
        .collect();

    for entry in command_allowlist {
        let trimmed_entry = entry.trim();
        if trimmed_entry.is_empty() {
            continue;
        }
        let Ok(allowlisted_tokens) = tokenize_command(trimmed_entry) else {
            continue;
        };
        let lowered_allowlisted_tokens: Vec<String> = allowlisted_tokens
            .iter()
            .map(|token| token.to_lowercase())
            .collect();
        if lowered_allowlisted_tokens.len() > lowered_tokens.len() {
            continue;
        }
        if lowered_tokens.starts_with(&lowered_allowlisted_tokens) {
            return Ok(DirectCommandSpec {
                program: raw_tokens[0].clone(),
                args: raw_tokens[1..].to_vec(),
            });
        }
    }

    Err("command is outside allowlist".into())
}

fn parse_builtin_allowlisted_command(command: &str) -> Result<DirectCommandSpec, String> {
    let trimmed = command.trim();
    let normalized = trimmed.to_lowercase();
    if normalized.is_empty() {
        return Err("command is outside allowlist".into());
    }

    let dangerous_markers = ["rm ", "sudo ", "git push", "brew install"];

    if dangerous_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return Err("dangerous command".into());
    }

    let raw_tokens = tokenize_command(trimmed)?;
    let lowered_tokens: Vec<String> = raw_tokens
        .iter()
        .map(|token| token.to_lowercase())
        .collect();

    match lowered_tokens.as_slice() {
        [cmd] if cmd == "pwd" && raw_tokens.len() == 1 => Ok(DirectCommandSpec {
            program: "pwd".into(),
            args: vec![],
        }),
        [cmd, ..] if cmd == "pwd" => Err("pwd only supports zero arguments".into()),
        [cmd] if cmd == "ls" && raw_tokens.len() == 1 => Ok(DirectCommandSpec {
            program: "ls".into(),
            args: vec![],
        }),
        [cmd, ..] if cmd == "ls" => Ok(DirectCommandSpec {
            program: "ls".into(),
            args: validate_relative_path_args(&raw_tokens[1..])?,
        }),
        [cmd, ..] if cmd == "cat" && raw_tokens.len() > 1 => Ok(DirectCommandSpec {
            program: "cat".into(),
            args: validate_relative_path_args(&raw_tokens[1..])?,
        }),
        [cmd] if cmd == "cat" => Err("cat requires at least one relative path argument".into()),
        [cmd, pattern] if cmd == "rg" => {
            validate_search_pattern(pattern)?;
            Ok(DirectCommandSpec {
                program: "rg".into(),
                args: vec![raw_tokens[1].clone()],
            })
        }
        [cmd, pattern, _paths @ ..] if cmd == "rg" => {
            validate_search_pattern(pattern)?;
            let mut args = vec![raw_tokens[1].clone()];
            args.extend(validate_relative_path_args(&raw_tokens[2..])?);
            Ok(DirectCommandSpec {
                program: "rg".into(),
                args,
            })
        }
        [cmd, flag, range, file] if cmd == "sed" && *flag == "-n" => {
            validate_sed_range(range)?;
            Ok(DirectCommandSpec {
                program: "sed".into(),
                args: vec![
                    "-n".into(),
                    raw_tokens[2].clone(),
                    validate_single_relative_path(file)?,
                ],
            })
        }
        [cmd, flag, count, file] if (cmd == "head" || cmd == "tail") && *flag == "-n" => {
            validate_positive_number(count)?;
            Ok(DirectCommandSpec {
                program: if *cmd == "head" {
                    "head".into()
                } else {
                    "tail".into()
                },
                args: vec![
                    "-n".into(),
                    raw_tokens[2].clone(),
                    validate_single_relative_path(file)?,
                ],
            })
        }
        [cmd, ..] if cmd == "curl" => Ok(DirectCommandSpec {
            program: "curl".into(),
            args: parse_curl_args(&raw_tokens[1..])?,
        }),
        [cmd, ..] if cmd == "wget" => Ok(DirectCommandSpec {
            program: "wget".into(),
            args: parse_wget_args(&raw_tokens[1..])?,
        }),
        [cmd1, cmd2] if cmd1 == "git" && cmd2 == "status" => Ok(DirectCommandSpec {
            program: "git".into(),
            args: vec!["status".into()],
        }),
        [cmd1, cmd2, cmd3]
            if cmd1 == "git" && cmd2 == "status" && cmd3 == "--short" =>
        {
            Ok(DirectCommandSpec {
                program: "git".into(),
                args: vec!["status".into(), "--short".into()],
            })
        }
        [cmd1, cmd2] if cmd1 == "cargo" && cmd2 == "test" => Ok(DirectCommandSpec {
            program: "cargo".into(),
            args: vec!["test".into()],
        }),
        [cmd1, cmd2] if cmd1 == "cargo" && cmd2 == "build" => Ok(DirectCommandSpec {
            program: "cargo".into(),
            args: vec!["build".into()],
        }),
        [cmd1, cmd2] if cmd1 == "cargo" && cmd2 == "check" => Ok(DirectCommandSpec {
            program: "cargo".into(),
            args: vec!["check".into()],
        }),
        [cmd1, cmd2, cmd3]
            if cmd1 == "cargo" && cmd2 == "fmt" && cmd3 == "--check" =>
        {
            Ok(DirectCommandSpec {
                program: "cargo".into(),
                args: vec!["fmt".into(), "--check".into()],
            })
        }
        [cmd1, cmd2, ..]
            if cmd1 == "cargo"
                && (cmd2 == "test" || cmd2 == "build" || cmd2 == "check" || cmd2 == "fmt") =>
        {
            Err("cargo allowlist only supports plain test/build/check or fmt --check commands".into())
        }
        [cmd] if cmd == "pytest" => Ok(DirectCommandSpec {
            program: "pytest".into(),
            args: vec![],
        }),
        [cmd, _args @ ..] if cmd == "pytest" => Ok(DirectCommandSpec {
            program: "pytest".into(),
            args: parse_pytest_args(&raw_tokens[1..])?,
        }),
        [cmd1, cmd2] if cmd1 == "uv" && cmd2 == "run" => {
            Err("uv allowlist requires an explicit subcommand after `uv run`".into())
        }
        [cmd1, cmd2, ..] if cmd1 == "uv" && cmd2 == "run" => Ok(DirectCommandSpec {
            program: "uv".into(),
            args: parse_uv_run_args(&raw_tokens[2..])?,
        }),
        [cmd1, cmd2] if cmd1 == "node" && (cmd2 == "-v" || cmd2 == "--version") => {
            Ok(DirectCommandSpec {
                program: "node".into(),
                args: vec![raw_tokens[1].clone()],
            })
        }
        [cmd1, cmd2, cmd3]
            if cmd1 == "make" && (cmd2 == "-n" || cmd2 == "--just-print") =>
        {
            validate_make_target(cmd3)?;
            Ok(DirectCommandSpec {
                program: "make".into(),
                args: vec![raw_tokens[1].clone(), raw_tokens[2].clone()],
            })
        }
        [cmd1, cmd2] if cmd1 == "make" => {
            validate_make_target(cmd2)?;
            Ok(DirectCommandSpec {
                program: "make".into(),
                args: vec![raw_tokens[1].clone()],
            })
        }
        [cmd1, cmd2] if cmd1 == "npm" && cmd2 == "test" => Ok(DirectCommandSpec {
            program: "npm".into(),
            args: vec!["test".into()],
        }),
        [cmd1, cmd2, cmd3]
            if cmd1 == "npm"
                && cmd2 == "run"
                && (cmd3 == "build" || cmd3 == "test" || cmd3 == "lint") =>
        {
            Ok(DirectCommandSpec {
                program: "npm".into(),
                args: vec!["run".into(), raw_tokens[2].clone()],
            })
        }
        [cmd1, cmd2, ..] if cmd1 == "npm" && (cmd2 == "test" || cmd2 == "run") => {
            Err("npm allowlist only supports plain test or run build/test/lint commands".into())
        }
        [cmd1, cmd2] if cmd1 == "pnpm" && cmd2 == "test" => Ok(DirectCommandSpec {
            program: "pnpm".into(),
            args: vec!["test".into()],
        }),
        [cmd1, cmd2] if cmd1 == "pnpm" && cmd2 == "build" => Ok(DirectCommandSpec {
            program: "pnpm".into(),
            args: vec!["build".into()],
        }),
        [cmd1, cmd2] if cmd1 == "pnpm" && cmd2 == "check" => Ok(DirectCommandSpec {
            program: "pnpm".into(),
            args: vec!["check".into()],
        }),
        [cmd1, cmd2, cmd3]
            if cmd1 == "pnpm"
                && cmd2 == "run"
                && (cmd3 == "build" || cmd3 == "test" || cmd3 == "lint") =>
        {
            Ok(DirectCommandSpec {
                program: "pnpm".into(),
                args: vec!["run".into(), raw_tokens[2].clone()],
            })
        }
        [cmd1, cmd2, ..]
            if cmd1 == "pnpm"
                && (cmd2 == "test" || cmd2 == "build" || cmd2 == "check" || cmd2 == "run") =>
        {
            Err(
                "pnpm allowlist only supports plain test/build/check or run build/test/lint commands"
                    .into(),
            )
        }
        [cmd1, cmd2, ..] if cmd1 == "git" && cmd2 == "diff" => parse_git_diff_args(&raw_tokens[2..]),
        [cmd1, cmd2, ..] if cmd1 == "git" && cmd2 == "show" => parse_git_show_args(&raw_tokens[2..]),
        [cmd1, cmd2, ..] if cmd1 == "git" && cmd2 == "log" => parse_git_log_args(&raw_tokens[2..]),
        _ => Err("command is outside allowlist".into()),
    }
}

fn tokenize_command(command: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in command.chars() {
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err("unterminated quoted argument".into());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.is_empty() {
        Err("command is outside allowlist".into())
    } else {
        Ok(tokens)
    }
}

fn has_disallowed_shell_syntax(command: &str) -> bool {
    let mut quote: Option<char> = None;
    let chars: Vec<char> = command.chars().collect();
    let len = chars.len();
    let mut i = 0usize;

    while i < len {
        let ch = chars[i];
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => {}
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None => {
                let next = chars.get(i + 1).copied();
                if ch == '\n'
                    || ch == '\r'
                    || ch == '$'
                    || ch == '`'
                    || ch == ';'
                    || ch == '>'
                    || ch == '<'
                {
                    return true;
                }
                if ch == '&' {
                    if next == Some('&')
                        || chars
                            .get(i.wrapping_sub(1))
                            .copied()
                            .map(|prev| prev.is_whitespace())
                            .unwrap_or(true)
                    {
                        return true;
                    }
                }
                if ch == '|' {
                    return true;
                }
            }
        }
        i += 1;
    }

    quote.is_some()
}

fn validate_single_relative_path(value: &str) -> Result<String, String> {
    validate_workspace_argument(value)?;
    Ok(value.to_string())
}

fn validate_search_pattern(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("search pattern is empty".into());
    }

    let disallowed = [
        '*', '?', '[', ']', ';', '|', '&', '>', '<', '$', '`', '\n', '\r', '\'', '"',
    ];
    if value.chars().any(|ch| disallowed.contains(&ch)) {
        return Err("search pattern is not allowlisted".into());
    }

    Ok(())
}

fn validate_sed_range(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("sed range is empty".into());
    }

    let is_valid = value
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == 'p');
    if !is_valid || !value.ends_with('p') {
        return Err("sed range is not allowlisted".into());
    }

    Ok(())
}

fn validate_positive_number(value: &str) -> Result<(), String> {
    if value
        .parse::<usize>()
        .ok()
        .filter(|count| *count > 0)
        .is_some()
    {
        Ok(())
    } else {
        Err("numeric argument is not allowlisted".into())
    }
}

fn parse_curl_args(values: &[String]) -> Result<Vec<String>, String> {
    if values.is_empty() {
        return Err("curl requires a URL".into());
    }

    let mut args = Vec::new();
    let mut url: Option<String> = None;

    for value in values {
        if value.starts_with('-') {
            validate_curl_flag(value)?;
            args.push(value.to_string());
            continue;
        }

        if url.is_some() {
            return Err("curl only supports a single URL".into());
        }

        validate_local_http_url(value)?;
        url = Some(value.to_string());
    }

    let Some(url) = url else {
        return Err("curl requires a URL".into());
    };
    args.push(url);
    Ok(args)
}

fn parse_wget_args(values: &[String]) -> Result<Vec<String>, String> {
    if values.is_empty() {
        return Err("wget requires a URL".into());
    }

    let mut args = Vec::new();
    let mut url: Option<String> = None;

    for value in values {
        if value.starts_with('-') {
            validate_wget_flag(value)?;
            args.push(value.to_string());
            continue;
        }

        if url.is_some() {
            return Err("wget only supports a single URL".into());
        }

        validate_local_http_url(value)?;
        url = Some(value.to_string());
    }

    let Some(url) = url else {
        return Err("wget requires a URL".into());
    };
    args.push(url);
    Ok(args)
}

fn validate_curl_flag(value: &str) -> Result<(), String> {
    let allowed_exact = [
        "-L",
        "-I",
        "-i",
        "-s",
        "-S",
        "-f",
        "--location",
        "--head",
        "--silent",
        "--show-error",
        "--fail",
    ];
    if allowed_exact.contains(&value) {
        return Ok(());
    }

    if let Some(shorts) = value.strip_prefix('-') {
        if !shorts.is_empty()
            && !value.starts_with("--")
            && shorts
                .chars()
                .all(|ch| matches!(ch, 'L' | 'I' | 'i' | 's' | 'S' | 'f'))
        {
            return Ok(());
        }
    }

    Err("curl flag is not allowlisted".into())
}

fn validate_wget_flag(value: &str) -> Result<(), String> {
    let allowed_exact = ["-q", "-O-", "--quiet"];
    if allowed_exact.contains(&value) {
        Ok(())
    } else {
        Err("wget flag is not allowlisted".into())
    }
}

pub(crate) fn validate_fetch_url(value: &str) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("only http(s) URLs are allowlisted".into());
    }

    if value.contains('@') {
        return Err("URLs with embedded credentials are not allowlisted".into());
    }

    Ok(())
}

fn validate_local_http_url(value: &str) -> Result<(), String> {
    validate_fetch_url(value)?;
    if is_local_http_url(value) {
        Ok(())
    } else {
        Err("external HTTP fetches should use fetch_url".into())
    }
}

fn is_local_http_url(value: &str) -> bool {
    reqwest::Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"))
        .unwrap_or(false)
}

fn parse_pytest_args(values: &[String]) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    for value in values {
        if value == "-q" || value == "-x" || value == "-s" {
            args.push(value.to_string());
        } else if let Some(number) = value.strip_prefix("--maxfail=") {
            validate_positive_number(number)?;
            args.push(value.to_string());
        } else {
            args.push(validate_single_relative_path(value)?);
        }
    }
    Ok(args)
}

fn parse_uv_run_args(values: &[String]) -> Result<Vec<String>, String> {
    let lowered: Vec<String> = values.iter().map(|value| value.to_lowercase()).collect();
    match lowered.as_slice() {
        [cmd] if cmd == "pytest" => Ok(vec!["run".into(), values[0].clone()]),
        [cmd, _args @ ..] if cmd == "pytest" => {
            let mut parsed = vec!["run".into(), values[0].clone()];
            parsed.extend(parse_pytest_args(&values[1..])?);
            Ok(parsed)
        }
        [cmd1, cmd2, cmd3] if cmd1 == "python" && cmd2 == "-m" && cmd3 == "pytest" => Ok(vec![
            "run".into(),
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
        ]),
        [cmd1, cmd2, cmd3, rest @ ..] if cmd1 == "python" && cmd2 == "-m" && cmd3 == "pytest" => {
            let mut parsed = vec![
                "run".into(),
                values[0].clone(),
                values[1].clone(),
                values[2].clone(),
            ];
            parsed.extend(parse_pytest_args(rest)?);
            Ok(parsed)
        }
        _ => Err("uv allowlist only supports `uv run pytest ...` or `uv run python -m pytest ...`".into()),
    }
}

fn validate_make_target(value: &str) -> Result<(), String> {
    let allowed = ["test", "build", "check", "lint"];
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err("make target is not allowlisted".into())
    }
}

fn parse_git_diff_args(values: &[String]) -> Result<DirectCommandSpec, String> {
    match values {
        [flag] if flag == "--stat" || flag == "--name-only" => Ok(DirectCommandSpec {
            program: "git".into(),
            args: vec!["diff".into(), flag.to_string()],
        }),
        [sep, paths @ ..] if sep == "--" && !paths.is_empty() => {
            let mut args = vec!["diff".into(), "--".into()];
            args.extend(validate_relative_path_args(paths)?);
            Ok(DirectCommandSpec {
                program: "git".into(),
                args,
            })
        }
        [flag, sep, paths @ ..] if (flag == "--stat" || flag == "--name-only") && sep == "--" && !paths.is_empty() => {
            let mut args = vec!["diff".into(), flag.to_string(), "--".into()];
            args.extend(validate_relative_path_args(paths)?);
            Ok(DirectCommandSpec {
                program: "git".into(),
                args,
            })
        }
        _ => Err("git diff allowlist only supports --stat, --name-only, or path-scoped diff".into()),
    }
}

fn parse_git_show_args(values: &[String]) -> Result<DirectCommandSpec, String> {
    match values {
        [flag1, flag2, rev] if flag1 == "--stat" && flag2 == "--oneline" && is_safe_git_revision(rev) => {
            Ok(DirectCommandSpec {
                program: "git".into(),
                args: vec!["show".into(), "--stat".into(), "--oneline".into(), rev.to_string()],
            })
        }
        _ => Err("git show allowlist only supports `git show --stat --oneline <rev>`".into()),
    }
}

fn parse_git_log_args(values: &[String]) -> Result<DirectCommandSpec, String> {
    match values {
        [flag1, flag2, count] if flag1 == "--oneline" && flag2 == "-n" => {
            validate_positive_number(count)?;
            Ok(DirectCommandSpec {
                program: "git".into(),
                args: vec!["log".into(), "--oneline".into(), "-n".into(), count.to_string()],
            })
        }
        _ => Err("git log allowlist only supports `git log --oneline -n <count>`".into()),
    }
}

fn is_safe_git_revision(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
}

fn validate_relative_path_args(values: &[String]) -> Result<Vec<String>, String> {
    values
        .iter()
        .map(|value| {
            validate_workspace_argument(value)?;
            Ok(value.to_string())
        })
        .collect()
}

fn validate_workspace_argument(value: &str) -> Result<(), String> {
    if value.starts_with('~') || Path::new(value).is_absolute() {
        return Err("command argument resolves outside workspace".into());
    }

    if value.starts_with('-') {
        return Err("command argument is not allowlisted".into());
    }

    if value.contains('*') || value.contains('?') || value.contains('[') || value.contains(']') {
        return Err("command argument is not allowlisted".into());
    }

    ensure_workspace_relative(value)
        .map_err(|_| "command argument resolves outside workspace".into())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_command_risk, ensure_workspace_relative, evaluate_action_permission,
        resolve_workspace_dir, resolve_workspace_path, PermissionOutcome,
    };
    use crate::agent::actions::{AgentAction, FetchMethod, VerificationCheck, WriteFileRequest};
    use std::fs;
    use std::os::unix::fs::symlink;
    use uuid::Uuid;

    fn make_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("deepseekx-permissions-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_parent_traversal() {
        let err = ensure_workspace_relative("../secrets.txt").unwrap_err();
        assert!(err.contains("outside workspace"));
    }

    #[test]
    fn blocks_dangerous_shell_commands() {
        let verdict = classify_command_risk("rm -rf src", &[]);
        assert_eq!(verdict.requires_confirmation, true);
        assert_eq!(verdict.allowed_without_confirmation, false);
    }

    #[test]
    fn allows_known_safe_build_commands() {
        let verdict = classify_command_risk("cargo test", &[]);
        assert_eq!(verdict.allowed_without_confirmation, true);
    }

    #[test]
    fn configured_allowlist_bypasses_default_command_gate() {
        let verdict = classify_command_risk(
            "python -m pytest tests/unit -q",
            &["python -m pytest".into()],
        );
        assert_eq!(verdict.allowed_without_confirmation, true);
        assert_eq!(verdict.requires_confirmation, false);
    }

    #[test]
    fn configured_allowlist_can_explicitly_allow_git_push() {
        let verdict =
            classify_command_risk("git push origin main", &["git push".into()]);
        assert_eq!(verdict.allowed_without_confirmation, true);
        assert_eq!(verdict.requires_confirmation, false);
    }

    #[test]
    fn blocks_compound_shell_commands_even_with_safe_prefix() {
        let verdict = classify_command_risk("pwd; rm -rf src", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_absolute_path_arguments_for_allowlisted_commands() {
        let verdict = classify_command_risk("cat /etc/hosts", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_parent_traversal_arguments_for_allowlisted_commands() {
        let verdict = classify_command_risk("ls ../../", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_single_quoted_absolute_path_arguments() {
        let verdict = classify_command_risk("cat '/etc/hosts'", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_double_quoted_parent_traversal_arguments() {
        let verdict = classify_command_risk("ls \"../../\"", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_home_variable_expansion_arguments() {
        let verdict = classify_command_risk("cat $HOME/.ssh/config", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_braced_home_variable_expansion_arguments() {
        let verdict = classify_command_risk("ls ${HOME}", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_glob_arguments_for_allowlisted_commands() {
        let verdict = classify_command_risk("ls *", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_unexpected_pwd_arguments() {
        let verdict = classify_command_risk("pwd extra", &[]);
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn allows_relative_cat_arguments() {
        let verdict = classify_command_risk("cat src/main.rs", &[]);
        assert_eq!(verdict.allowed_without_confirmation, true);
        assert_eq!(verdict.requires_confirmation, false);
    }

    #[test]
    fn allows_read_only_workspace_search_commands() {
        for command in [
            "rg planner src",
            "sed -n 1,120p src/main.rs",
            "head -n 20 src/main.rs",
            "tail -n 20 src/main.rs",
            "git status",
            "git status --short",
            "git diff --stat",
            "curl -I http://127.0.0.1:8000",
            "wget http://127.0.0.1:3000",
            "cargo check",
            "cargo fmt --check",
            "pytest -q",
            "uv run pytest -q",
            "uv run python -m pytest -q tests",
            "node --version",
            "make test",
            "npm run build",
            "npm run lint",
            "pnpm run test",
            "git diff --name-only -- src-tauri/src/lib.rs",
            "git show --stat --oneline HEAD",
            "git log --oneline -n 3",
        ] {
            let verdict = classify_command_risk(command, &[]);
            assert_eq!(verdict.allowed_without_confirmation, true, "{command}");
            assert_eq!(verdict.requires_confirmation, false, "{command}");
        }
    }

    #[test]
    fn blocks_disallowed_search_pattern_and_flags() {
        let pattern_verdict = classify_command_risk("rg $(whoami) src", &[]);
        assert_eq!(pattern_verdict.allowed_without_confirmation, false);
        assert_eq!(pattern_verdict.requires_confirmation, true);

        let git_verdict = classify_command_risk("git diff", &[]);
        assert_eq!(git_verdict.allowed_without_confirmation, false);
        assert_eq!(git_verdict.requires_confirmation, true);

        let curl_output_verdict = classify_command_risk("curl -o out.txt https://example.com", &[]);
        assert_eq!(curl_output_verdict.allowed_without_confirmation, false);
        assert_eq!(curl_output_verdict.requires_confirmation, true);

        let curl_credentials_verdict = classify_command_risk("curl https://user:pass@example.com", &[]);
        assert_eq!(curl_credentials_verdict.allowed_without_confirmation, false);
        assert_eq!(curl_credentials_verdict.requires_confirmation, true);

        let external_curl_verdict = classify_command_risk("curl -fsSL https://example.com", &[]);
        assert_eq!(external_curl_verdict.allowed_without_confirmation, false);
        assert_eq!(external_curl_verdict.requires_confirmation, true);

        let quoted_absolute_path_verdict = classify_command_risk("cat '/etc/hosts'", &[]);
        assert_eq!(
            quoted_absolute_path_verdict.allowed_without_confirmation,
            false
        );
        assert_eq!(quoted_absolute_path_verdict.requires_confirmation, true);
    }

    #[test]
    fn resolves_existing_paths_within_workspace() {
        let workspace = make_temp_dir();
        let file_path = workspace.join("src").join("main.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "fn main() {}").unwrap();

        let resolved = resolve_workspace_path(&workspace, "src/main.rs").unwrap();
        assert_eq!(
            std::fs::canonicalize(resolved).unwrap(),
            std::fs::canonicalize(file_path).unwrap()
        );

        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn rejects_symlink_escape_outside_workspace() {
        let workspace = make_temp_dir();
        let outside_dir = make_temp_dir();
        let escape_link = workspace.join("escape");
        symlink(&outside_dir, &escape_link).unwrap();

        let err = resolve_workspace_path(&workspace, "escape/secret.txt").unwrap_err();
        assert!(err.contains("outside workspace"));

        fs::remove_dir_all(workspace).unwrap();
        fs::remove_dir_all(outside_dir).unwrap();
    }

    #[test]
    fn resolves_workspace_root_for_dot_cwd() {
        let workspace = make_temp_dir();
        let resolved = resolve_workspace_dir(&workspace, Some(".")).unwrap();
        assert_eq!(resolved, std::fs::canonicalize(&workspace).unwrap());
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn large_write_operations_require_confirmation() {
        let workspace = make_temp_dir();
        let action = AgentAction::WriteFiles {
            reason: "overwrite many files".into(),
            files: (0..31)
                .map(|idx| WriteFileRequest {
                    path: format!("src/file-{idx}.txt"),
                    content: "ok".into(),
                })
                .collect(),
        };

        let outcome = evaluate_action_permission(&workspace, &action, &[]).unwrap();
        assert!(matches!(
            outcome,
            PermissionOutcome::RequiresConfirmation { .. }
        ));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn allows_fetch_url_and_verify_checks_actions() {
        let workspace = make_temp_dir();

        let fetch = AgentAction::FetchUrl {
            reason: "read external docs".into(),
            url: "https://example.com/docs".into(),
            method: FetchMethod::Get,
            max_chars: Some(2000),
        };
        let verify = AgentAction::VerifyChecks {
            reason: "confirm build and tests".into(),
            checks: vec![
                VerificationCheck {
                    label: "web build".into(),
                    command: "npm run build".into(),
                    cwd: Some(".".into()),
                },
                VerificationCheck {
                    label: "rust tests".into(),
                    command: "cargo test".into(),
                    cwd: Some(".".into()),
                },
            ],
        };

        assert!(matches!(
            evaluate_action_permission(&workspace, &fetch, &[]).unwrap(),
            PermissionOutcome::Allowed
        ));
        assert!(matches!(
            evaluate_action_permission(&workspace, &verify, &[]).unwrap(),
            PermissionOutcome::Allowed
        ));

        fs::remove_dir_all(workspace).unwrap();
    }
}
