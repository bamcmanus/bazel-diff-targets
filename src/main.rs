mod args;
mod error;
mod git;

use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let args = args::Args::parse();

    let current_dir = match std::env::current_dir() {
        Ok(current_dir) => current_dir,
        Err(error) => {
            let error = error::AppError::WorkspaceValidationFailure {
                message: format!("failed to determine current directory: {error}"),
            };
            print_error(&error, args.json);
            return ExitCode::FAILURE;
        }
    };

    match run(&args, current_dir.as_path()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            print_error(&error, args.json);
            ExitCode::FAILURE
        }
    }
}

fn run(args: &args::Args, current_dir: &Path) -> Result<(), error::AppError> {
    let repository = git::GitRepository::discover(current_dir)?;

    repository.validate_state()?;

    if !args.dirty && repository.is_working_tree_dirty()? {
        return Err(error::AppError::DirtyWorkingTree {
            entries: Vec::new(),
            remaining: 0,
        });
    }

    Ok(())
}

fn print_error(error: &error::AppError, json: bool) {
    if json {
        eprintln!("{}", error.format_json());
    } else {
        eprintln!("{}", error.format_human());
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{args, error, run};

    #[test]
    fn run_rejects_dirty_working_tree_without_dirty_flag() {
        let repo_dir = new_temp_dir("run-rejects-dirty-working-tree");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        std::fs::write(repo_dir.join("README.md"), "# changed\n").expect("modify test file");
        let args = test_args(false);

        let result = run(&args, repo_dir.as_path());

        let error = result.expect_err("dirty working tree should be rejected");
        assert!(
            matches!(&error, error::AppError::DirtyWorkingTree { entries, remaining: 0 } if entries.is_empty()),
            "expected dirty working tree error with no entries, got {error:?}"
        );

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    #[test]
    fn run_allows_dirty_working_tree_with_dirty_flag() {
        let repo_dir = new_temp_dir("run-allows-dirty-working-tree");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        std::fs::write(repo_dir.join("README.md"), "# changed\n").expect("modify test file");
        let args = test_args(true);

        let result = run(&args, repo_dir.as_path());

        assert!(
            result.is_ok(),
            "dirty working tree should be allowed with dirty flag, got {result:?}"
        );

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    fn test_args(dirty: bool) -> args::Args {
        args::Args {
            base_ref: None,
            head_ref: None,
            dirty,
            workspace_path: None,
            java_path: None,
            bazel_path: None,
            bazel_diff_jar: None,
            target_types: Vec::new(),
            use_cquery: false,
            exclude_external_targets: false,
            include_external_targets: false,
            include_distance: false,
            bazel_startup_options: None,
            bazel_command_options: None,
            json: false,
            quiet: false,
            verbose: false,
        }
    }

    fn new_temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "bazel-diff-targets-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after UNIX epoch")
                .as_nanos()
        );
        path.push(unique);
        std::fs::create_dir(&path).expect("create test directory");

        path
    }

    fn run_git_for_test(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git command");

        if !output.status.success() {
            panic!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }
}
