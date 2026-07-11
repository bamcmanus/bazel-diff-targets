use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::AppError;

pub struct GitRepository {
    root: PathBuf,
    original_checkout: OriginalCheckout,
    backend: CliGitBackend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginalCheckout {
    Branch { name: String, sha: String },
    Detached { sha: String },
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedRef {
    pub reference: String,
    pub sha: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedRefs {
    pub base: ResolvedRef,
    pub head: ResolvedRef,
}

trait GitBackend {
    fn discover_root(&self, current_dir: &Path) -> Result<PathBuf, AppError>;
    fn current_branch(&self, root: &Path) -> Result<Option<String>, AppError>;
    fn current_sha(&self, root: &Path) -> Result<String, AppError>;
    fn resolve_ref(&self, root: &Path, reference: &str) -> Result<String, AppError>;
    fn is_bare_repository(&self, root: &Path) -> Result<bool, AppError>;
    fn is_shallow_repository(&self, root: &Path) -> Result<bool, AppError>;
    fn merge_base(
        &self,
        root: &Path,
        left_ref: &str,
        right_ref: &str,
    ) -> Result<Option<String>, AppError>;
}

struct CliGitBackend;

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
}

impl GitRepository {
    pub fn discover(current_dir: &Path) -> Result<Self, AppError> {
        let backend = CliGitBackend;
        let root = backend.discover_root(current_dir)?;
        let sha = backend.current_sha(&root)?;

        let original_checkout = match backend.current_branch(&root)? {
            Some(name) => OriginalCheckout::Branch { name, sha },
            None => OriginalCheckout::Detached { sha },
        };

        Ok(Self {
            root,
            original_checkout,
            backend,
        })
    }

    pub fn validate_state(&self) -> Result<(), AppError> {
        if self.backend.is_bare_repository(&self.root)? {
            return Err(AppError::BareRepository);
        }

        if self.backend.is_shallow_repository(&self.root)? {
            return Err(AppError::ShallowRepository);
        }

        Ok(())
    }

    pub fn resolve_ref(&self, reference: &str) -> Result<ResolvedRef, AppError> {
        let sha = self.backend.resolve_ref(&self.root, reference)?;

        Ok(ResolvedRef {
            reference: reference.to_owned(),
            sha,
        })
    }

    pub fn infer_base_ref(&self, head_ref: &str) -> Result<ResolvedRef, AppError> {
        let head = self.resolve_ref(head_ref)?;

        for default_branch_ref in ["origin/HEAD", "origin/main", "origin/master"] {
            match self.resolve_ref(default_branch_ref) {
                Ok(_) => {
                    if let Some(merge_base_sha) = self.backend.merge_base(
                        &self.root,
                        head.sha.as_str(),
                        default_branch_ref,
                    )? {
                        return Ok(ResolvedRef {
                            reference: default_branch_ref.to_owned(),
                            sha: merge_base_sha,
                        });
                    }
                }
                Err(AppError::InvalidRef { .. }) => continue,
                Err(error) => return Err(error),
            }
        }

        Err(AppError::BaseRefInferenceFailure)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn original_checkout(&self) -> &OriginalCheckout {
        &self.original_checkout
    }
}

#[cfg(test)]
mod tests {
    use super::{GitRepository, OriginalCheckout};

    #[test]
    fn discovers_repository_from_directory_inside_repo() {
        let repo_dir = new_temp_dir("discover-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let nested_dir = repo_dir.join("nested");
        std::fs::create_dir(&nested_dir).expect("create nested directory");

        let repository =
            GitRepository::discover(nested_dir.as_path()).expect("discover test Git repository");

        let expected_root = repo_dir.canonicalize().expect("canonicalize repo dir");
        assert_eq!(repository.root(), expected_root.as_path());
        let OriginalCheckout::Branch { name, sha } = repository.original_checkout() else {
            panic!(
                "expected branch checkout, got {:?}",
                repository.original_checkout()
            );
        };
        assert_eq!(name, "main");
        assert!([40, 64].contains(&sha.len()));
        assert!(sha.chars().all(|character| character.is_ascii_hexdigit()));
        repository
            .validate_state()
            .expect("normal repository should be supported");

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    #[test]
    fn rejects_bare_repository_state() {
        let repo_dir = new_temp_dir("bare-repo");
        run_git_for_test(&repo_dir, &["init", "--bare"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover bare reposiotry");

        let error = repository
            .validate_state()
            .expect_err("bare repository should be rejected");

        assert!(matches!(error, crate::error::AppError::BareRepository));

        std::fs::remove_dir_all(repo_dir).expect("remove test repository")
    }

    #[test]
    fn rejects_shallow_repository_state() {
        let source_repo_dir = new_temp_dir("shallow-source-repo");
        run_git_for_test(&source_repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&source_repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(
            &source_repo_dir,
            &["config", "user.email", "test@example.invalid"],
        );
        std::fs::write(source_repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&source_repo_dir, &["add", "README.md"]);
        run_git_for_test(&source_repo_dir, &["commit", "-m", "initial commit"]);
        let shallow_repo_dir = new_temp_dir("shallow-repo");
        std::fs::remove_dir_all(&shallow_repo_dir).expect("remove clone destination placeholder");
        let source_url = format!("file://{}", source_repo_dir.display());
        run_git_for_test(
            std::env::temp_dir().as_path(),
            &[
                "clone",
                "--depth",
                "1",
                source_url.as_str(),
                shallow_repo_dir
                    .to_str()
                    .expect("temp path should be valid UTF-8"),
            ],
        );
        let repository = GitRepository::discover(shallow_repo_dir.as_path())
            .expect("discover shallow repository");

        let error = repository
            .validate_state()
            .expect_err("shallow repository should be rejected");

        assert!(matches!(error, crate::error::AppError::ShallowRepository));

        std::fs::remove_dir_all(source_repo_dir).expect("remove source test repository");
        std::fs::remove_dir_all(shallow_repo_dir).expect("remove shallow test repository");
    }

    #[test]
    fn resolves_ref_to_commit_sha() {
        let repo_dir = new_temp_dir("resolve-ref-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

        let resolved_ref = repository
            .resolve_ref("main")
            .expect("resolve branch ref to commit SHA");

        let OriginalCheckout::Branch { sha, .. } = repository.original_checkout() else {
            panic!(
                "expected branch checkout, got {:?}",
                repository.original_checkout()
            );
        };

        assert_eq!(resolved_ref.reference, "main");
        assert_eq!(&resolved_ref.sha, sha);

        std::fs::remove_dir_all(repo_dir).expect("remove test repository")
    }

    #[test]
    fn rejects_invalid_ref() {
        let repo_dir = new_temp_dir("invalid-ref-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

        let error = repository
            .resolve_ref("does-not-exist")
            .expect_err("invalid ref should be rejected");

        assert!(
            matches!(error, crate::error::AppError::InvalidRef { reference } if reference == "does-not-exist")
        );

        std::fs::remove_dir_all(repo_dir).expect("remove test repository")
    }

    #[test]
    fn infers_base_ref_from_origin_head() {
        let repo_dir = new_temp_dir("infer-base-origin-head-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let expected_base_sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
        run_git_for_test(
            &repo_dir,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        run_git_for_test(
            &repo_dir,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        run_git_for_test(&repo_dir, &["checkout", "-b", "feature"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n\nfeature change\n")
            .expect("write feature change");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "feature commit"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

        let inferred_base = repository
            .infer_base_ref("HEAD")
            .expect("infer base ref from origin/HEAD");

        assert_eq!(inferred_base.reference, "origin/HEAD");
        assert_eq!(inferred_base.sha, expected_base_sha);

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    #[test]
    fn infers_base_ref_from_origin_main_when_origin_head_is_missing() {
        let repo_dir = new_temp_dir("infer-base-origin-main-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let expected_base_sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
        run_git_for_test(
            &repo_dir,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        run_git_for_test(&repo_dir, &["checkout", "-b", "feature"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n\nfeature change\n")
            .expect("write feature change");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "feature commit"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

        let inferred_base = repository
            .infer_base_ref("HEAD")
            .expect("infer base ref from origin/main");

        assert_eq!(inferred_base.reference, "origin/main");
        assert_eq!(inferred_base.sha, expected_base_sha);

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    #[test]
    fn infers_base_ref_from_origin_master_when_origin_head_and_main_are_missing() {
        let repo_dir = new_temp_dir("infer-base-origin-master-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "master"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let expected_base_sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
        run_git_for_test(
            &repo_dir,
            &["update-ref", "refs/remotes/origin/master", "HEAD"],
        );
        run_git_for_test(&repo_dir, &["checkout", "-b", "feature"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n\nfeature change\n")
            .expect("write feature change");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "feature commit"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

        let inferred_base = repository
            .infer_base_ref("HEAD")
            .expect("infer base ref from origin/master");

        assert_eq!(inferred_base.reference, "origin/master");
        assert_eq!(inferred_base.sha, expected_base_sha);

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    #[test]
    fn fails_to_infer_base_ref_when_default_branch_candidates_are_missing() {
        let repo_dir = new_temp_dir("infer-base-missing-candidates-repo");
        run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
        run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
        run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
        std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
        run_git_for_test(&repo_dir, &["add", "README.md"]);
        run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
        let repository =
            GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

        let error = repository
            .infer_base_ref("HEAD")
            .expect_err("missing default branch candidates should fail inference");

        assert!(matches!(
            error,
            crate::error::AppError::BaseRefInferenceFailure
        ));

        std::fs::remove_dir_all(repo_dir).expect("remove test repository");
    }

    fn run_git_output_for_test(root: &std::path::Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git");

        assert!(
            output.status.success(),
            "git {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout)
            .expect("git output should be valid UTF-8")
            .trim()
            .to_owned()
    }

    fn new_temp_dir(name: &str) -> std::path::PathBuf {
        let unique = format!(
            "bazel-diff-targets-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after UNIX epoch")
                .as_nanos()
        );

        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir(&path).expect("create temp directory");

        path
    }

    fn run_git_for_test(root: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git");

        assert!(
            output.status.success(),
            "git {} failed\nstdout:\n{}\nstderr\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
