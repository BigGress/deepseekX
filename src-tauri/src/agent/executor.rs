use std::path::{Path, PathBuf};

use tokio::process::Command;
use walkdir::WalkDir;

use crate::agent::permissions::{
    ensure_workspace_relative, parse_allowlisted_command, resolve_workspace_dir, resolve_workspace_path,
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

pub async fn run_guarded_command(
    workspace_root: &str,
    command: &str,
    cwd: Option<&str>,
) -> GuardedCommandResult {
    let spec = match parse_allowlisted_command(command) {
        Ok(spec) => spec,
        Err(reason) => {
            return GuardedCommandResult::RequiresConfirmation { reason };
        }
    };

    let resolved_args = match spec.program {
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

    let output = Command::new(spec.program)
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

#[cfg(test)]
mod tests {
    use super::{
        list_workspace_entries, read_workspace_file, run_guarded_command, write_workspace_file,
        GuardedCommandResult,
    };
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

        let result = run_guarded_command(&workspace.to_string_lossy(), "rm -rf src", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_absolute_path_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "cat /etc/hosts", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_parent_traversal_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls ../../", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_single_quoted_absolute_path_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "cat '/etc/hosts'", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_double_quoted_parent_traversal_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls \"../../\"", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_home_variable_expansion_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "cat $HOME/.ssh/config", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_braced_home_variable_expansion_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls ${HOME}", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_glob_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "ls *", None).await;

        assert!(matches!(
            result,
            GuardedCommandResult::RequiresConfirmation { .. }
        ));

        fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn blocks_unexpected_pwd_arguments_during_execution() {
        let workspace = make_temp_dir();

        let result = run_guarded_command(&workspace.to_string_lossy(), "pwd extra", None).await;

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

        let result = run_guarded_command(&workspace.to_string_lossy(), "cat src/main.rs", None).await;

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

        let result = run_guarded_command(&workspace.to_string_lossy(), "pwd", None).await;
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
    async fn executes_read_only_search_and_preview_commands() {
        let workspace = make_temp_dir();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src").join("main.rs"),
            "fn main() {\n    println!(\"planner\");\n}\n",
        )
        .unwrap();

        let rg = run_guarded_command(&workspace.to_string_lossy(), "rg planner src", None).await;
        let sed = run_guarded_command(&workspace.to_string_lossy(), "sed -n 1,2p src/main.rs", None).await;
        let head = run_guarded_command(&workspace.to_string_lossy(), "head -n 1 src/main.rs", None).await;

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

        let result = run_guarded_command(&workspace.to_string_lossy(), "git status", None).await;

        let GuardedCommandResult::Executed { exit_code, stdout, .. } = result else {
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

        let err = write_workspace_file(
            &workspace.to_string_lossy(),
            "escape/payload.txt",
            "owned",
        )
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

        let result = run_guarded_command(&workspace.to_string_lossy(), "pwd", None).await;

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
        fs::write(workspace.join("src").join("agent").join("planner.rs"), "mod x;").unwrap();

        let entries = list_workspace_entries(&workspace.to_string_lossy(), Some("src"), 2)
            .await
            .unwrap();

        assert!(entries.iter().any(|entry| entry == "src/agent/"));
        assert!(entries.iter().any(|entry| entry == "src/agent/planner.rs"));

        fs::remove_dir_all(workspace).unwrap();
    }
}
