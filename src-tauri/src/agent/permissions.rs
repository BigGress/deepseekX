use std::path::{Component, Path, PathBuf};

use crate::agent::actions::AgentAction;

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
            Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("path resolves outside workspace".into());
            }
        }
    }

    Ok(())
}

pub fn resolve_workspace_path(workspace_root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    ensure_workspace_relative(rel_path)?;

    let workspace_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("workspace root invalid ({}): {}", workspace_root.display(), e))?;

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

pub fn resolve_workspace_dir(workspace_root: &Path, raw_path: Option<&str>) -> Result<PathBuf, String> {
    let workspace_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("workspace root invalid ({}): {}", workspace_root.display(), e))?;

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
) -> Result<PermissionOutcome, String> {
    match action {
        AgentAction::RetrieveContext { .. } | AgentAction::SummarizeFindings { .. } => {
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::ReadFiles { files, .. } => {
            for file in files {
                let _ = resolve_workspace_path(workspace_root, file)?;
            }
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::WriteFiles { files, .. } => {
            for file in files {
                let _ = resolve_workspace_path(workspace_root, &file.path)?;
            }

            let total_bytes: usize = files.iter().map(|file| file.content.len()).sum();
            if files.len() > 10 || total_bytes > 200_000 {
                return Ok(PermissionOutcome::RequiresConfirmation {
                    reason: "覆盖大量文件或写入内容过大".into(),
                    risk_level: "high",
                });
            }

            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::RunCommand { command, cwd, .. } => {
            let verdict = classify_command_risk(command);
            if verdict.requires_confirmation || !verdict.allowed_without_confirmation {
                return Ok(PermissionOutcome::RequiresConfirmation {
                    reason: verdict.reason,
                    risk_level: "high",
                });
            }

            let _ = resolve_workspace_dir(workspace_root, cwd.as_deref())?;
            Ok(PermissionOutcome::Allowed)
        }
        AgentAction::AskUser { .. } | AgentAction::Finish { .. } => Ok(PermissionOutcome::Allowed),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectCommandSpec {
    pub program: &'static str,
    pub args: Vec<String>,
}

pub fn classify_command_risk(command: &str) -> CommandRiskVerdict {
    let normalized = command.trim().to_lowercase();

    let dangerous_markers = ["rm ", "sudo ", "git push", "curl ", "wget ", "brew install"];
    let compound_shell_markers = ["&&", "||", ";", "|", "&", ">", "<", "$", "`", "\n", "'", "\""];

    if compound_shell_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return CommandRiskVerdict {
            allowed_without_confirmation: false,
            requires_confirmation: true,
            reason: "compound shell syntax is not allowlisted".into(),
        };
    }

    if dangerous_markers.iter().any(|marker| normalized.contains(marker)) {
        return CommandRiskVerdict {
            allowed_without_confirmation: false,
            requires_confirmation: true,
            reason: "dangerous command".into(),
        };
    }

    match parse_allowlisted_command(command) {
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

pub(crate) fn parse_allowlisted_command(command: &str) -> Result<DirectCommandSpec, String> {
    let trimmed = command.trim();
    let normalized = trimmed.to_lowercase();
    if normalized.is_empty() {
        return Err("command is outside allowlist".into());
    }

    let dangerous_markers = ["rm ", "sudo ", "git push", "curl ", "wget ", "brew install"];
    let compound_shell_markers = ["&&", "||", ";", "|", "&", ">", "<", "$", "`", "\n", "'", "\""];

    if compound_shell_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return Err("compound shell syntax is not allowlisted".into());
    }

    if dangerous_markers.iter().any(|marker| normalized.contains(marker)) {
        return Err("dangerous command".into());
    }

    let raw_tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let lowered_tokens: Vec<String> = raw_tokens.iter().map(|token| token.to_lowercase()).collect();

    match lowered_tokens.as_slice() {
        [cmd] if cmd == "pwd" && raw_tokens.len() == 1 => Ok(DirectCommandSpec {
            program: "pwd",
            args: vec![],
        }),
        [cmd, ..] if cmd == "pwd" => Err("pwd only supports zero arguments".into()),
        [cmd] if cmd == "ls" && raw_tokens.len() == 1 => Ok(DirectCommandSpec {
            program: "ls",
            args: vec![],
        }),
        [cmd, ..] if cmd == "ls" => Ok(DirectCommandSpec {
            program: "ls",
            args: validate_relative_path_args(&raw_tokens[1..])?,
        }),
        [cmd, ..] if cmd == "cat" && raw_tokens.len() > 1 => Ok(DirectCommandSpec {
            program: "cat",
            args: validate_relative_path_args(&raw_tokens[1..])?,
        }),
        [cmd] if cmd == "cat" => Err("cat requires at least one relative path argument".into()),
        [cmd, pattern] if cmd == "rg" => {
            validate_search_pattern(pattern)?;
            Ok(DirectCommandSpec {
                program: "rg",
                args: vec![raw_tokens[1].to_string()],
            })
        }
        [cmd, pattern, _paths @ ..] if cmd == "rg" => {
            validate_search_pattern(pattern)?;
            let mut args = vec![raw_tokens[1].to_string()];
            args.extend(validate_relative_path_args(&raw_tokens[2..])?);
            Ok(DirectCommandSpec {
                program: "rg",
                args,
            })
        }
        [cmd, flag, range, file] if cmd == "sed" && *flag == "-n" => {
            validate_sed_range(range)?;
            Ok(DirectCommandSpec {
                program: "sed",
                args: vec!["-n".into(), raw_tokens[2].to_string(), validate_single_relative_path(file)?],
            })
        }
        [cmd, flag, count, file] if (cmd == "head" || cmd == "tail") && *flag == "-n" => {
            validate_positive_number(count)?;
            Ok(DirectCommandSpec {
                program: if *cmd == "head" { "head" } else { "tail" },
                args: vec!["-n".into(), raw_tokens[2].to_string(), validate_single_relative_path(file)?],
            })
        }
        [cmd1, cmd2] if cmd1 == "git" && cmd2 == "status" => Ok(DirectCommandSpec {
            program: "git",
            args: vec!["status".into()],
        }),
        [cmd1, cmd2, cmd3] if cmd1 == "git" && cmd2 == "diff" && cmd3 == "--stat" => Ok(DirectCommandSpec {
            program: "git",
            args: vec!["diff".into(), "--stat".into()],
        }),
        [cmd1, cmd2] if cmd1 == "cargo" && cmd2 == "test" => Ok(DirectCommandSpec {
            program: "cargo",
            args: vec!["test".into()],
        }),
        [cmd1, cmd2] if cmd1 == "cargo" && cmd2 == "build" => Ok(DirectCommandSpec {
            program: "cargo",
            args: vec!["build".into()],
        }),
        [cmd1, cmd2] if cmd1 == "cargo" && cmd2 == "check" => Ok(DirectCommandSpec {
            program: "cargo",
            args: vec!["check".into()],
        }),
        [cmd1, cmd2, ..] if cmd1 == "cargo" && (cmd2 == "test" || cmd2 == "build" || cmd2 == "check") => {
            Err("cargo allowlist only supports plain test/build/check commands".into())
        }
        [cmd1, cmd2] if cmd1 == "npm" && cmd2 == "test" => Ok(DirectCommandSpec {
            program: "npm",
            args: vec!["test".into()],
        }),
        [cmd1, cmd2, cmd3] if cmd1 == "npm" && cmd2 == "run" && (cmd3 == "build" || cmd3 == "test") => Ok(DirectCommandSpec {
            program: "npm",
            args: vec!["run".into(), raw_tokens[2].to_string()],
        }),
        [cmd1, cmd2, ..] if cmd1 == "npm" && (cmd2 == "test" || cmd2 == "run") => {
            Err("npm allowlist only supports plain test or run build/test commands".into())
        }
        [cmd1, cmd2] if cmd1 == "pnpm" && cmd2 == "test" => Ok(DirectCommandSpec {
            program: "pnpm",
            args: vec!["test".into()],
        }),
        [cmd1, cmd2] if cmd1 == "pnpm" && cmd2 == "build" => Ok(DirectCommandSpec {
            program: "pnpm",
            args: vec!["build".into()],
        }),
        [cmd1, cmd2] if cmd1 == "pnpm" && cmd2 == "check" => Ok(DirectCommandSpec {
            program: "pnpm",
            args: vec!["check".into()],
        }),
        [cmd1, cmd2, cmd3] if cmd1 == "pnpm" && cmd2 == "run" && (cmd3 == "build" || cmd3 == "test") => Ok(DirectCommandSpec {
            program: "pnpm",
            args: vec!["run".into(), raw_tokens[2].to_string()],
        }),
        [cmd1, cmd2, ..] if cmd1 == "pnpm" && (cmd2 == "test" || cmd2 == "build" || cmd2 == "check" || cmd2 == "run") => {
            Err("pnpm allowlist only supports plain test/build/check or run build/test commands".into())
        }
        _ => Err("command is outside allowlist".into()),
    }
}

fn validate_single_relative_path(value: &str) -> Result<String, String> {
    validate_workspace_argument(value)?;
    Ok(value.to_string())
}

fn validate_search_pattern(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("search pattern is empty".into());
    }

    let disallowed = ['*', '?', '[', ']', ';', '|', '&', '>', '<', '$', '`', '\n', '\r', '\'', '"'];
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
    if value.parse::<usize>().ok().filter(|count| *count > 0).is_some() {
        Ok(())
    } else {
        Err("numeric argument is not allowlisted".into())
    }
}

fn validate_relative_path_args(values: &[&str]) -> Result<Vec<String>, String> {
    values
        .iter()
        .map(|value| {
            validate_workspace_argument(value)?;
            Ok((*value).to_string())
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

    ensure_workspace_relative(value).map_err(|_| "command argument resolves outside workspace".into())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_command_risk, ensure_workspace_relative, evaluate_action_permission,
        resolve_workspace_dir, resolve_workspace_path, PermissionOutcome,
    };
    use crate::agent::actions::{AgentAction, WriteFileRequest};
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
        let verdict = classify_command_risk("rm -rf src");
        assert_eq!(verdict.requires_confirmation, true);
        assert_eq!(verdict.allowed_without_confirmation, false);
    }

    #[test]
    fn allows_known_safe_build_commands() {
        let verdict = classify_command_risk("cargo test");
        assert_eq!(verdict.allowed_without_confirmation, true);
    }

    #[test]
    fn blocks_compound_shell_commands_even_with_safe_prefix() {
        let verdict = classify_command_risk("pwd; rm -rf src");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_absolute_path_arguments_for_allowlisted_commands() {
        let verdict = classify_command_risk("cat /etc/hosts");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_parent_traversal_arguments_for_allowlisted_commands() {
        let verdict = classify_command_risk("ls ../../");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_single_quoted_absolute_path_arguments() {
        let verdict = classify_command_risk("cat '/etc/hosts'");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_double_quoted_parent_traversal_arguments() {
        let verdict = classify_command_risk("ls \"../../\"");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_home_variable_expansion_arguments() {
        let verdict = classify_command_risk("cat $HOME/.ssh/config");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_braced_home_variable_expansion_arguments() {
        let verdict = classify_command_risk("ls ${HOME}");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_glob_arguments_for_allowlisted_commands() {
        let verdict = classify_command_risk("ls *");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn blocks_unexpected_pwd_arguments() {
        let verdict = classify_command_risk("pwd extra");
        assert_eq!(verdict.allowed_without_confirmation, false);
        assert_eq!(verdict.requires_confirmation, true);
    }

    #[test]
    fn allows_relative_cat_arguments() {
        let verdict = classify_command_risk("cat src/main.rs");
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
            "git diff --stat",
            "cargo check",
            "npm run build",
            "pnpm run test",
        ] {
            let verdict = classify_command_risk(command);
            assert_eq!(verdict.allowed_without_confirmation, true, "{command}");
            assert_eq!(verdict.requires_confirmation, false, "{command}");
        }
    }

    #[test]
    fn blocks_disallowed_search_pattern_and_flags() {
        let pattern_verdict = classify_command_risk("rg \"planner loop\" src");
        assert_eq!(pattern_verdict.allowed_without_confirmation, false);
        assert_eq!(pattern_verdict.requires_confirmation, true);

        let git_verdict = classify_command_risk("git diff");
        assert_eq!(git_verdict.allowed_without_confirmation, false);
        assert_eq!(git_verdict.requires_confirmation, true);
    }

    #[test]
    fn resolves_existing_paths_within_workspace() {
        let workspace = make_temp_dir();
        let file_path = workspace.join("src").join("main.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "fn main() {}").unwrap();

        let resolved = resolve_workspace_path(&workspace, "src/main.rs").unwrap();
        assert_eq!(std::fs::canonicalize(resolved).unwrap(), std::fs::canonicalize(file_path).unwrap());

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
            files: (0..11)
                .map(|idx| WriteFileRequest {
                    path: format!("src/file-{idx}.txt"),
                    content: "ok".into(),
                })
                .collect(),
        };

        let outcome = evaluate_action_permission(&workspace, &action).unwrap();
        assert!(matches!(outcome, PermissionOutcome::RequiresConfirmation { .. }));
        fs::remove_dir_all(workspace).unwrap();
    }
}
