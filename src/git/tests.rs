use super::{CheckoutPlan, GitRepository, HeadSelection, OriginalCheckout, ResolvedRef};

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
    let repository = GitRepository::discover(repo_dir.as_path()).expect("discover bare reposiotry");

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
    let repository =
        GitRepository::discover(shallow_repo_dir.as_path()).expect("discover shallow repository");

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

#[test]
fn clean_working_tree_is_not_dirty() {
    let repo_dir = new_temp_dir("clean-working-tree-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

    let is_dirty = repository
        .is_working_tree_dirty()
        .expect("check working tree status");

    assert!(!is_dirty);

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn modified_tracked_file_makes_working_tree_dirty() {
    let repo_dir = new_temp_dir("modified-tracked-file-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n\nmodified\n")
        .expect("modify tracked file");
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

    let is_dirty = repository
        .is_working_tree_dirty()
        .expect("check working tree status");

    assert!(is_dirty);

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn untracked_file_makes_working_tree_dirty() {
    let repo_dir = new_temp_dir("untracked-file-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    std::fs::write(repo_dir.join("untracked.txt"), "untracked\n").expect("write untracked file");
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

    let is_dirty = repository
        .is_working_tree_dirty()
        .expect("check working tree status");

    assert!(is_dirty);

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn ignored_file_does_not_make_working_tree_dirty() {
    let repo_dir = new_temp_dir("ignored-file-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    std::fs::write(repo_dir.join(".gitignore"), "ignored.txt\n").expect("write gitignore");
    run_git_for_test(&repo_dir, &["add", "README.md", ".gitignore"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    std::fs::write(repo_dir.join("ignored.txt"), "ignored\n").expect("write ignored file");
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

    let is_dirty = repository
        .is_working_tree_dirty()
        .expect("check working tree status");

    assert!(!is_dirty);

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn plan_checkouts_uses_current_checkout_for_dirty_head() {
    let repo_dir = new_temp_dir("plan-checkouts-dirty-head-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");
    let base = ResolvedRef {
        reference: "origin/main".to_owned(),
        sha: "base-sha".to_owned(),
    };

    let checkouts = repository.plan_checkouts(&base, HeadSelection::Dirty);

    assert!(
        matches!(&checkouts.head, CheckoutPlan::Current { git_root } if git_root == repository.root()),
        "expected head checkout to use current checkout at {}, got {:?}",
        repository.root().display(),
        checkouts.head
    );

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn plan_checkouts_uses_current_checkout_for_resolved_head_at_current_sha() {
    let repo_dir = new_temp_dir("plan-checkouts-current-head-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");
    let current_sha = match repository.original_checkout() {
        OriginalCheckout::Branch { sha, .. } | OriginalCheckout::Detached { sha } => sha.clone(),
    };
    let base = ResolvedRef {
        reference: "origin/main".to_owned(),
        sha: "base-sha".to_owned(),
    };
    let head = ResolvedRef {
        reference: "HEAD".to_owned(),
        sha: current_sha,
    };

    let checkouts = repository.plan_checkouts(&base, HeadSelection::Resolved(&head));

    assert!(
        matches!(&checkouts.head, CheckoutPlan::Current { git_root } if git_root == repository.root()),
        "expected head checkout to use current checkout at {}, got {:?}",
        repository.root().display(),
        checkouts.head
    );

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn plan_checkouts_uses_worktree_for_resolved_head_at_other_sha() {
    let repo_dir = new_temp_dir("plan-checkouts-other-head-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");
    let base = ResolvedRef {
        reference: "origin/main".to_owned(),
        sha: "base-sha".to_owned(),
    };
    let head = ResolvedRef {
        reference: "HEAD~1".to_owned(),
        sha: "other-sha".to_owned(),
    };

    let checkouts = repository.plan_checkouts(&base, HeadSelection::Resolved(&head));

    assert_eq!(
        checkouts.head,
        CheckoutPlan::Worktree {
            reference: "HEAD~1".to_owned(),
            sha: "other-sha".to_owned(),
        },
        "expected head checkout to use a worktree, got {:?}",
        checkouts.head
    );

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn plan_checkouts_uses_worktree_for_base() {
    let repo_dir = new_temp_dir("plan-checkouts-base-worktree-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");
    let base = ResolvedRef {
        reference: "origin/main".to_owned(),
        sha: "base-sha".to_owned(),
    };

    let checkouts = repository.plan_checkouts(&base, HeadSelection::Dirty);

    assert_eq!(
        checkouts.base,
        CheckoutPlan::Worktree {
            reference: "origin/main".to_owned(),
            sha: "base-sha".to_owned(),
        },
        "expected base checkout to use a worktree, got {:?}",
        checkouts.base
    );

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
