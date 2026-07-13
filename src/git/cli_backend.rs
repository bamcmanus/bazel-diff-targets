use std::path::{Path, PathBuf};
use std::process::Command;

use super::backend::GitBackend;
use crate::error::AppError;

pub(super) struct CliGitBackend;

fn run_git(root: &Path, args: &[&str]) -> Result<String, AppError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| AppError::CheckoutFailure {
            message: format!("failed to run git: {error}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        let message = if stderr.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            format!("git {} failed: {stderr}", args.join(" "))
        };

        return Err(AppError::CheckoutFailure { message });
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|error| AppError::OutputParsingFailure {
            message: format!("git output was not valid UTF-8: {error}"),
        })?;

    Ok(stdout.trim().to_owned())
}

impl GitBackend for CliGitBackend {
    fn discover_root(&self, current_dir: &Path) -> Result<PathBuf, AppError> {
        match run_git(current_dir, &["rev-parse", "--show-toplevel"]) {
            Ok(root) => Ok(PathBuf::from(root)),
            Err(_) => {
                let git_dir = run_git(current_dir, &["rev-parse", "--git-dir"])?;
                let git_dir = PathBuf::from(git_dir);

                if git_dir.is_absolute() {
                    Ok(git_dir)
                } else {
                    Ok(current_dir.join(git_dir))
                }
            }
        }
    }

    fn current_branch(&self, root: &Path) -> Result<Option<String>, AppError> {
        let output = Command::new("git")
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .current_dir(root)
            .output()
            .map_err(|error| AppError::CheckoutFailure {
                message: format!("failed to run git: {error}"),
            })?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout).map_err(|error| {
                AppError::OutputParsingFailure {
                    message: format!("git output was not valid UTF-8: {error}"),
                }
            })?;

            Ok(Some(stdout.trim().to_owned()))
        } else {
            Ok(None)
        }
    }

    fn current_sha(&self, root: &Path) -> Result<String, AppError> {
        run_git(root, &["rev-parse", "HEAD"])
    }

    fn is_bare_repository(&self, root: &Path) -> Result<bool, AppError> {
        let output = run_git(root, &["rev-parse", "--is-bare-repository"])?;

        Ok(output == "true")
    }

    fn is_shallow_repository(&self, root: &Path) -> Result<bool, AppError> {
        let output = run_git(root, &["rev-parse", "--is-shallow-repository"])?;

        Ok(output == "true")
    }

    fn resolve_ref(&self, root: &Path, reference: &str) -> Result<String, AppError> {
        let commit_reference = format!("{reference}^{{commit}}");
        let output = Command::new("git")
            .args(["rev-parse", "--verify", commit_reference.as_str()])
            .current_dir(root)
            .output()
            .map_err(|error| AppError::CheckoutFailure {
                message: format!("failed to run git: {error}"),
            })?;

        if !output.status.success() {
            return Err(AppError::InvalidRef {
                reference: reference.to_owned(),
            });
        }

        let stdout =
            String::from_utf8(output.stdout).map_err(|error| AppError::OutputParsingFailure {
                message: format!("git output was not valid UTF-8: {error}"),
            })?;

        Ok(stdout.trim().to_owned())
    }

    fn merge_base(
        &self,
        root: &Path,
        left_ref: &str,
        right_ref: &str,
    ) -> Result<Option<String>, AppError> {
        let output = Command::new("git")
            .args(["merge-base", left_ref, right_ref])
            .current_dir(root)
            .output()
            .map_err(|error| AppError::CheckoutFailure {
                message: format!("failed to run git: {error}"),
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout =
            String::from_utf8(output.stdout).map_err(|error| AppError::OutputParsingFailure {
                message: format!("git output was not valid UTF-8: {error}"),
            })?;

        Ok(Some(stdout.trim().to_owned()))
    }

    fn is_working_tree_dirty(&self, root: &Path) -> Result<bool, AppError> {
        let output = run_git(root, &["status", "--porcelain=v1", "--untracked-files=all"])?;

        Ok(!output.is_empty())
    }

    fn remove_worktree(
        &self,
        repository_root: &Path,
        worktree_root: &Path,
    ) -> Result<(), AppError> {
        let worktree_root = worktree_root
            .to_str()
            .ok_or_else(|| AppError::CheckoutFailure {
                message: format!(
                    "worktree path is not valid UTF-8: {}",
                    worktree_root.display()
                ),
            })?;

        run_git(
            repository_root,
            &["worktree", "remove", "--force", worktree_root],
        )?;

        Ok(())
    }

    fn add_detached_worktree(
        &self,
        repository_root: &Path,
        worktree_root: &Path,
        sha: &str,
    ) -> Result<(), AppError> {
        let worktree_root = worktree_root
            .to_str()
            .ok_or_else(|| AppError::CheckoutFailure {
                message: format!(
                    "worktree path is not valid UTF-8: {}",
                    worktree_root.display()
                ),
            })?;

        run_git(
            repository_root,
            &["worktree", "add", "--detach", worktree_root, sha],
        )?;

        Ok(())
    }
}
