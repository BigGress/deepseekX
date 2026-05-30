use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use tokio::process::Command;
use walkdir::WalkDir;

use crate::agent::actions::{FetchMethod, PatchHunkRequest};
use crate::agent::permissions::{
    ensure_workspace_relative, parse_allowlisted_command, resolve_workspace_dir,
    resolve_workspace_path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedCommandResult {
    Executed {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    RequiresConfirmation {
        reason: String,
    },
    SpawnFailed {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchUrlResult {
    pub url: String,
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body_excerpt: Option<String>,
}

pub async fn read_workspace_file(workspace_root: &str, rel_path: &str) -> Result<String, String> {
    let full_path = resolve_workspace_path(Path::new(workspace_root), rel_path)?;
    tokio::fs::read_to_string(&full_path)
        .await
        .map_err(|e| format!("read failed ({}): {}", full_path.display(), e))
}

pub async fn write_workspace_file(
    workspace_root: &str,
    rel_path: &str,
    content: &str,
) -> Result<(), String> {
    ensure_workspace_relative(rel_path)?;
    let full_path = resolve_workspace_path(Path::new(workspace_root), rel_path)?;
    if let Some(parent_rel) = Path::new(rel_path).parent() {
        if !parent_rel.as_os_str().is_empty() {
            resolve_workspace_path(Path::new(workspace_root), &parent_rel.to_string_lossy())?;
        }
    }
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir failed ({}): {}", parent.display(), e))?;
    }

    if full_path.exists() {
        resolve_workspace_path(Path::new(workspace_root), rel_path)?;
    }

    tokio::fs::write(&full_path, content)
        .await
        .map_err(|e| format!("write failed ({}): {}", full_path.display(), e))
}

pub async fn create_workspace_file(
    workspace_root: &str,
    rel_path: &str,
    content: &str,
) -> Result<(), String> {
    ensure_workspace_relative(rel_path)?;
    let full_path = resolve_workspace_path(Path::new(workspace_root), rel_path)?;
    if full_path.exists() {
        return Err(format!("create failed ({}): file already exists", full_path.display()));
    }
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir failed ({}): {}", parent.display(), e))?;
    }
    tokio::fs::write(&full_path, content)
        .await
        .map_err(|e| format!("write failed ({}): {}", full_path.display(), e))
}

pub async fn rename_workspace_entry(
    workspace_root: &str,
    from_path: &str,
    to_path: &str,
) -> Result<(), String> {
    ensure_workspace_relative(from_path)?;
    ensure_workspace_relative(to_path)?;
    let from_full = resolve_workspace_path(Path::new(workspace_root), from_path)?;
    let to_full = resolve_workspace_path(Path::new(workspace_root), to_path)?;
    if !from_full.exists() {
        return Err(format!("rename failed ({}): source does not exist", from_full.display()));
    }
    if to_full.exists() {
        return Err(format!("rename failed ({}): destination already exists", to_full.display()));
    }
    if let Some(parent) = to_full.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir failed ({}): {}", parent.display(), e))?;
    }
    tokio::fs::rename(&from_full, &to_full)
        .await
        .map_err(|e| format!("rename failed ({} -> {}): {}", from_full.display(), to_full.display(), e))
}

pub async fn delete_workspace_entry(workspace_root: &str, rel_path: &str) -> Result<(), String> {
    ensure_workspace_relative(rel_path)?;
    let full_path = resolve_workspace_path(Path::new(workspace_root), rel_path)?;
    let metadata = tokio::fs::symlink_metadata(&full_path)
        .await
        .map_err(|e| format!("delete failed ({}): {}", full_path.display(), e))?;

    if metadata.is_dir() {
        tokio::fs::remove_dir_all(&full_path)
            .await
            .map_err(|e| format!("remove dir failed ({}): {}", full_path.display(), e))
    } else {
        tokio::fs::remove_file(&full_path)
            .await
            .map_err(|e| format!("remove file failed ({}): {}", full_path.display(), e))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPatchResult {
    pub after: String,
    pub changed_ranges: Vec<String>,
    pub applied_hunks: usize,
}

pub async fn apply_workspace_patch(
    workspace_root: &str,
    rel_path: &str,
    hunks: &[PatchHunkRequest],
) -> Result<ApplyPatchResult, String> {
    ensure_workspace_relative(rel_path)?;
    if hunks.is_empty() {
        return Err("patch requires at least one hunk".into());
    }

    let full_path = resolve_workspace_path(Path::new(workspace_root), rel_path)?;
    let mut current = tokio::fs::read_to_string(&full_path)
        .await
        .map_err(|e| format!("read failed ({}): {}", full_path.display(), e))?;
    let mut changed_ranges = Vec::new();

    for (index, hunk) in hunks.iter().enumerate() {
        if hunk.old_text.is_empty() {
            return Err(format!("hunk {} has empty old_text", index + 1));
        }

        let occurrence = hunk.occurrence.unwrap_or(1);
        if occurrence == 0 {
            return Err(format!("hunk {} occurrence must be >= 1", index + 1));
        }

        let matches = current.match_indices(&hunk.old_text).collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(format!("hunk {} old_text not found in {}", index + 1, rel_path));
        }
        if occurrence > matches.len() {
            return Err(format!(
                "hunk {} requested occurrence {} but only found {} match(es) in {}",
                index + 1,
                occurrence,
                matches.len(),
                rel_path
            ));
        }

        let (start, matched) = matches[occurrence - 1];
        let start_line = line_number_at(&current, start);
        let end_line = start_line + matched.lines().count().max(1) - 1;
        current.replace_range(start..start + matched.len(), &hunk.new_text);
        changed_ranges.push(format!("{}:{}-{}", rel_path, start_line, end_line));
    }

    tokio::fs::write(&full_path, &current)
        .await
        .map_err(|e| format!("write failed ({}): {}", full_path.display(), e))?;

    Ok(ApplyPatchResult {
        after: current,
        changed_ranges,
        applied_hunks: hunks.len(),
    })
}

fn line_number_at(content: &str, byte_index: usize) -> usize {
    content[..byte_index].bytes().filter(|byte| *byte == b'\n').count() + 1
}

pub async fn run_guarded_command(
    workspace_root: &str,
    command: &str,
    cwd: Option<&str>,
    command_allowlist: &[String],
) -> GuardedCommandResult {
    let spec = match parse_allowlisted_command(command, command_allowlist) {
        Ok(spec) => spec,
        Err(reason) => {
            return GuardedCommandResult::RequiresConfirmation { reason };
        }
    };

    let resolved_args = match spec.program.as_str() {
        "ls" | "cat" => {
            let mut args = Vec::with_capacity(spec.args.len());
            for arg in &spec.args {
                match resolve_workspace_path(Path::new(workspace_root), arg) {
                    Ok(path) => args.push(path.to_string_lossy().to_string()),
                    Err(reason) => {
                        return GuardedCommandResult::RequiresConfirmation { reason };
                    }
                }
            }
            args
        }
        _ => spec.args.clone(),
    };

    let current_dir = match resolve_workspace_dir(Path::new(workspace_root), cwd) {
        Ok(dir) => dir,
        Err(reason) => {
            return GuardedCommandResult::RequiresConfirmation { reason };
        }
    };

    let output = Command::new(&spec.program)
        .args(&resolved_args)
        .current_dir(&current_dir)
        .output()
        .await
        .map_err(|e| format!("command spawn failed: {}", e));

    match output {
        Ok(output) => GuardedCommandResult::Executed {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        },
        Err(reason) => GuardedCommandResult::SpawnFailed { reason },
    }
}

pub async fn fetch_url(
    url: &str,
    method: &FetchMethod,
    max_chars: usize,
) -> Result<FetchUrlResult, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("http client init failed: {}", e))?;

    let request = match method {
        FetchMethod::Get => client.get(url),
        FetchMethod::Head => client.head(url),
    };

    let response = request
        .send()
        .await
        .map_err(|e| format!("http request failed: {}", e))?;
    let status_code = response.status().as_u16();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let body_excerpt = match method {
        FetchMethod::Head => None,
        FetchMethod::Get => {
            let text = response
                .text()
                .await
                .map_err(|e| format!("http body read failed: {}", e))?;
            Some(truncate_text(&text, max_chars.min(20_000)))
        }
    };

    Ok(FetchUrlResult {
        url: final_url,
        status_code,
        content_type,
        body_excerpt,
    })
}

pub async fn list_workspace_entries(
    workspace_root: &str,
    rel_path: Option<&str>,
    depth: usize,
) -> Result<Vec<String>, String> {
    let base_dir = resolve_workspace_dir(Path::new(workspace_root), rel_path)?;
    let workspace_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("workspace root invalid ({}): {}", workspace_root, e))?;

    let mut entries: Vec<String> = Vec::new();
    for entry in WalkDir::new(&base_dir)
        .max_depth(depth.max(1))
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.depth() == 0 {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(&workspace_root)
            .map(PathBuf::from)
            .map_err(|e| format!("list path resolution failed: {}", e))?;

        let mut display = rel.to_string_lossy().to_string();
        if entry.file_type().is_dir() {
            display.push('/');
        }
        entries.push(display);
    }

    entries.sort();
    Ok(entries)
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::{
        apply_workspace_patch, create_workspace_file, delete_workspace_entry,
        list_workspace_entries, read_workspace_file, rename_workspace_entry, run_guarded_command,
        write_workspace_file, GuardedCommandResult,
    };
    use crate::agent::actions::PatchHunkRequest;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tokio::process::Command;
    use uuid::Uuid;

    fn make_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("deepseekx-executor-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn blocks_commands_that_require_confirmation() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "rm -rf src", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_absolute_path_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result =
            run_guarded_command(&workspace.to_string_lossy(), "cat /etc/hosts", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_parent_traversal_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls ../../", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_single_quoted_absolute_path_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result =
            run_guarded_command(&workspace.to_string_lossy(), "cat '/etc/hosts'", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_double_quoted_parent_traversal_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls \"../../\"", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_home_variable_expansion_during_execution() {
        let workspace = make_temp_dir();

        let result =
            run_guarded_command(&workspace.to_string_lossy(), "cat $HOME/.ssh/config", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_braced_home_variable_expansion_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls ${HOME}", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_glob_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls *", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_unexpected_pwd_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "pwd extra", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn executes_relative_cat_arguments_without_shell() {
        let workspace = make_temp_dir();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(workspace.join("src").join("main.rs"), "fn main() {}\n").unwrap();

        let result =
            run_guarded_command(&workspace.to_string_lossy(), "cat src/main.rs", None, &[]).await;

        let GuardedCommandResult::Executed {
            exit_code,
            stdout,
            stderr,
        } = result
        else {
            panic!("expected direct command execution");
        };

        assert_eq!(exit_code, 0);
        assert_eq!(stdout, "fn main() {}\n");
        assert!(stderr.is_empty());

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn executes_allowlisted_commands() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "pwd", None, &[]).await;
        let GuardedCommandResult::Executed {
            exit_code,
            stdout,
            stderr,
        } = result
        else {
            panic!("expected command execution");
        };
        let reported_dir = std::fs::canonicalize(stdout.trim()).unwrap();
        let expected_dir = std::fs::canonicalize(&workspace).unwrap();

        assert_eq!(exit_code, 0);
        assert!(stderr.trim().is_empty());
        assert_eq!(reported_dir, expected_dir);

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn executes_configured_allowlisted_commands() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(
            &workspace.to_string_lossy(),
            "echo configured",
            None,
            &["echo".into()],
        )
        .await;

        let GuardedCommandResult::Executed { exit_code, .. } = result else {
            panic!("expected configured allowlisted command execution");
        };
        assert_eq!(exit_code, 0);

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn executes_read_only_search_and_preview_commands() {
        let workspace = make_temp_dir();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src").join("main.rs"),
            "fn main() {\n    println!(\"planner\");\n}\n",
        )
        .unwrap();

        let rg = run_guarded_command(&workspace.to_string_lossy(), "rg planner src", None, &[]).await;
        let sed = run_guarded_command(
            &workspace.to_string_lossy(),
            "sed -n 1,2p src/main.rs",
            None,
            &[],
        )
        .await;
        let head = run_guarded_command(&workspace.to_string_lossy(), "head -n 1 src/main.rs", None, &[]).await;

        for result in [rg, sed, head] {
            let GuardedCommandResult::Executed { exit_code, .. } = result else {
                panic!("expected command execution");
            };
            assert_eq!(exit_code, 0);
        }

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn executes_git_status_in_workspace() {
        let workspace = make_temp_dir();

        let init = Command::new("git")
            .arg("init")
            .current_dir(&workspace)
            .output()
            .await
            .unwrap();
        assert!(init.status.success());

        let result = run_guarded_command(&workspace.to_string_lossy(), "git status", None, &[]).await;

        let GuardedCommandResult::Executed {
            exit_code, stdout, ..
        } = result
        else {
            panic!("expected command execution");
        };
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("On branch") || stdout.contains("No commits yet"));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn rejects_read_through_symlink_outside_workspace() {
        let workspace = make_temp_dir();
        let outside_dir = make_temp_dir();
        let outside_file = outside_dir.join("secret.txt");
        fs::write(&outside_file, "top-secret").unwrap();
        symlink(&outside_file, workspace.join("linked-secret.txt")).unwrap();

        let err = read_workspace_file(&workspace.to_string_lossy(), "linked-secret.txt")
            .await
            .unwrap_err();

        assert!(err.contains("outside workspace"));

        fs::remove_dir_all(workspace).unwrap();
        fs::remove_dir_all(outside_dir).unwrap();
    }

    #[tokio::test]
    async fn rejects_write_through_symlinked_directory_outside_workspace() {
        let workspace = make_temp_dir();
        let outside_dir = make_temp_dir();
        symlink(&outside_dir, workspace.join("escape")).unwrap();

        let err = write_workspace_file(&workspace.to_string_lossy(), "escape/payload.txt", "owned")
            .await
            .unwrap_err();

        assert!(err.contains("outside workspace"));
        assert!(!outside_dir.join("payload.txt").exists());

        fs::remove_dir_all(workspace).unwrap();
        fs::remove_dir_all(outside_dir).unwrap();
    }

    #[tokio::test]
    async fn reports_spawn_failures_structurally() {
        let workspace = make_temp_dir();
        fs::remove_dir_all(&workspace).unwrap();

        let result = run_guarded_command(&workspace.to_string_lossy(), "pwd", None, &[]).await;

        assert!(matches!(
            result,
            GuardedCommandResult::SpawnFailed { .. }
                | GuardedCommandResult::RequiresConfirmation { .. }
        ));
    }

    #[tokio::test]
    async fn lists_workspace_entries_from_relative_directory() {
        let workspace = make_temp_dir();
        fs::create_dir_all(workspace.join("src").join("agent")).unwrap();
        fs::write(
            workspace.join("src").join("agent").join("planner.rs"),
            "mod x;",
        )
        .unwrap();

        let entries = list_workspace_entries(&workspace.to_string_lossy(), Some("src"), 2)
            .await
            .unwrap();

        assert!(entries.iter().any(|entry| entry == "src/agent/"));
        assert!(entries.iter().any(|entry| entry == "src/agent/planner.rs"));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn applies_exact_patch_to_existing_file() {
        let workspace = make_temp_dir();
        let path = workspace.join("src").join("demo.ts");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "const mode = 'chat';\nconsole.log(mode);\n").unwrap();

        let result = apply_workspace_patch(
            &workspace.to_string_lossy(),
            "src/demo.ts",
            &[PatchHunkRequest {
                old_text: "const mode = 'chat';".into(),
                new_text: "const mode = 'agent';".into(),
                occurrence: None,
            }],
        )
        .await
        .unwrap();

        assert!(result.after.contains("const mode = 'agent';"));
        assert_eq!(result.changed_ranges, vec!["src/demo.ts:1-1"]);
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn applies_patch_to_selected_occurrence() {
        let workspace = make_temp_dir();
        let path = workspace.join("src").join("demo.ts");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "alpha\nbeta\nalpha\n").unwrap();

        let result = apply_workspace_patch(
            &workspace.to_string_lossy(),
            "src/demo.ts",
            &[PatchHunkRequest {
                old_text: "alpha".into(),
                new_text: "gamma".into(),
                occurrence: Some(2),
            }],
        )
        .await
        .unwrap();

        assert_eq!(result.after, "alpha\nbeta\ngamma\n");
        assert_eq!(result.changed_ranges, vec!["src/demo.ts:3-3"]);
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn rejects_patch_when_old_text_missing() {
        let workspace = make_temp_dir();
        let path = workspace.join("src").join("demo.ts");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "hello\n").unwrap();

        let err = apply_workspace_patch(
            &workspace.to_string_lossy(),
            "src/demo.ts",
            &[PatchHunkRequest {
                old_text: "missing".into(),
                new_text: "updated".into(),
                occurrence: None,
            }],
        )
        .await
        .unwrap_err();

        assert!(err.contains("old_text not found"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn creates_new_workspace_file_without_overwrite() {
        let workspace = make_temp_dir();

        create_workspace_file(&workspace.to_string_lossy(), "src/new.ts", "export {};\n")
            .await
            .unwrap();
        let created = fs::read_to_string(workspace.join("src").join("new.ts")).unwrap();
        assert_eq!(created, "export {};\n");

        let err = create_workspace_file(&workspace.to_string_lossy(), "src/new.ts", "again")
            .await
            .unwrap_err();
        assert!(err.contains("already exists"));
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn renames_workspace_file() {
        let workspace = make_temp_dir();
        let src = workspace.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("before.ts"), "hello").unwrap();

        rename_workspace_entry(
            &workspace.to_string_lossy(),
            "src/before.ts",
            "src/after.ts",
        )
        .await
        .unwrap();

        assert!(!src.join("before.ts").exists());
        assert_eq!(fs::read_to_string(src.join("after.ts")).unwrap(), "hello");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn deletes_workspace_file() {
        let workspace = make_temp_dir();
        let src = workspace.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("temp.ts"), "bye").unwrap();

        delete_workspace_entry(&workspace.to_string_lossy(), "src/temp.ts")
            .await
            .unwrap();

        assert!(!src.join("temp.ts").exists());
        fs::remove_dir_all(workspace).unwrap();
    }
}
