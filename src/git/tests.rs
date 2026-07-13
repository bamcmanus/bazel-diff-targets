use super::{
    Checkout, CheckoutPlan, CheckoutPlans, Checkouts, GitRepository, HeadSelection,
    OriginalCheckout, ResolvedRef,
};

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

#[test]
fn removes_temporary_worktree() {
    let repo_dir = new_temp_dir("remove-worktree-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
    let worktree_dir = new_temp_dir("remove-worktree-destination");
    std::fs::remove_dir_all(&worktree_dir).expect("remove worktree destination placeholder");
    run_git_for_test(
        &repo_dir,
        &[
            "worktree",
            "add",
            "--detach",
            worktree_dir
                .to_str()
                .expect("temp path should be valid UTF-8"),
            sha.as_str(),
        ],
    );
    let canonical_worktree_path = worktree_dir
        .canonicalize()
        .expect("canonicalize temporary worktree path");
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

    repository
        .remove_worktree(canonical_worktree_path.as_path())
        .expect("remove temporary worktree");

    assert!(
        !worktree_dir.exists(),
        "temporary worktree should be removed from {}",
        worktree_dir.display()
    );
    let worktree_list = run_git_output_for_test(&repo_dir, &["worktree", "list", "--porcelain"]);
    let worktree_entry = format!("worktree {}", canonical_worktree_path.display());
    assert!(
        !worktree_list.contains(worktree_entry.as_str()),
        "temporary worktree should not remain registered: {worktree_entry}\n{worktree_list}"
    );
    let current_branch = run_git_output_for_test(&repo_dir, &["symbolic-ref", "--short", "HEAD"]);
    assert_eq!(
        current_branch, "main",
        "original checkout should remain on main after removing a temporary worktree"
    );

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn adds_detached_temporary_worktree() {
    let repo_dir = new_temp_dir("add-detached-worktree-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
    let temporary_parent = tempfile::tempdir().expect("create temporary worktree parent");
    let worktree_root = temporary_parent.path().join("worktree");
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");

    repository
        .add_detached_worktree(worktree_root.as_path(), sha.as_str())
        .expect("add detached temporary worktree");

    let worktree_sha = run_git_output_for_test(&worktree_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        worktree_sha, sha,
        "temporary worktree should be checked out at the requested commit"
    );
    let detached_head = std::process::Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(&worktree_root)
        .output()
        .expect("check temporary worktree HEAD");
    assert!(
        !detached_head.status.success(),
        "temporary worktree HEAD should be detached\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&detached_head.stdout),
        String::from_utf8_lossy(&detached_head.stderr)
    );
    let original_branch = run_git_output_for_test(&repo_dir, &["symbolic-ref", "--short", "HEAD"]);
    assert_eq!(
        original_branch, "main",
        "original checkout should remain on main after adding a temporary worktree"
    );

    repository
        .remove_worktree(worktree_root.as_path())
        .expect("remove temporary worktree");
    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn cleanup_checkouts_removes_base_and_head_worktrees() {
    let repo_dir = new_temp_dir("cleanup-checkouts-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
    let base_temporary_root = tempfile::tempdir().expect("create base temporary root");
    let head_temporary_root = tempfile::tempdir().expect("create head temporary root");
    let base_worktree_root = base_temporary_root.path().join("worktree");
    let head_worktree_root = head_temporary_root.path().join("worktree");
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");
    repository
        .add_detached_worktree(base_worktree_root.as_path(), sha.as_str())
        .expect("add base detached temporary worktree");
    repository
        .add_detached_worktree(head_worktree_root.as_path(), sha.as_str())
        .expect("add head detached temporary worktree");
    let canonical_base_worktree_root = base_worktree_root
        .canonicalize()
        .expect("canonicalize base temporary worktree path");
    let canonical_head_worktree_root = head_worktree_root
        .canonicalize()
        .expect("canonicalize head temporary worktree path");
    let checkouts = Checkouts {
        base: Checkout::Worktree {
            git_root: canonical_base_worktree_root.clone(),
            temporary_root: base_temporary_root.path().to_path_buf(),
        },
        head: Checkout::Worktree {
            git_root: canonical_head_worktree_root.clone(),
            temporary_root: head_temporary_root.path().to_path_buf(),
        },
    };

    repository
        .cleanup_checkouts(checkouts)
        .expect("clean up base and head temporary worktrees");

    assert!(
        !base_worktree_root.exists(),
        "base temporary worktree should be removed from {}",
        base_worktree_root.display()
    );
    assert!(
        !head_worktree_root.exists(),
        "head temporary worktree should be removed from {}",
        head_worktree_root.display()
    );
    assert!(
        !base_temporary_root.path().exists(),
        "base temporary root should be removed from {}",
        base_temporary_root.path().display()
    );
    assert!(
        !head_temporary_root.path().exists(),
        "head temporary root should be removed from {}",
        head_temporary_root.path().display()
    );
    let worktree_list = run_git_output_for_test(&repo_dir, &["worktree", "list", "--porcelain"]);
    let base_worktree_entry = format!("worktree {}", canonical_base_worktree_root.display());
    let head_worktree_entry = format!("worktree {}", canonical_head_worktree_root.display());
    assert!(
        !worktree_list.contains(base_worktree_entry.as_str()),
        "base temporary worktree should not remain registered: {base_worktree_entry}\n{worktree_list}"
    );
    assert!(
        !worktree_list.contains(head_worktree_entry.as_str()),
        "head temporary worktree should not remain registered: {head_worktree_entry}\n{worktree_list}"
    );
    let original_branch = run_git_output_for_test(&repo_dir, &["symbolic-ref", "--short", "HEAD"]);
    assert_eq!(
        original_branch, "main",
        "original checkout should remain on main after cleaning up temporary worktrees"
    );

    std::fs::remove_dir_all(repo_dir).expect("remove test repository");
}

#[test]
fn prepare_checkouts_materializes_base_and_head_worktrees() {
    let repo_dir = new_temp_dir("prepare-checkouts-repo");
    run_git_for_test(&repo_dir, &["init", "--initial-branch", "main"]);
    run_git_for_test(&repo_dir, &["config", "user.name", "Test User"]);
    run_git_for_test(&repo_dir, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(repo_dir.join("README.md"), "# test\n").expect("write test file");
    run_git_for_test(&repo_dir, &["add", "README.md"]);
    run_git_for_test(&repo_dir, &["commit", "-m", "initial commit"]);
    let sha = run_git_output_for_test(&repo_dir, &["rev-parse", "HEAD"]);
    let repository =
        GitRepository::discover(repo_dir.as_path()).expect("discover test Git repository");
    let plans = CheckoutPlans {
        base: CheckoutPlan::Worktree {
            reference: "origin/main".to_owned(),
            sha: sha.clone(),
        },
        head: CheckoutPlan::Worktree {
            reference: "HEAD~1".to_owned(),
            sha: sha.clone(),
        },
    };

    let checkouts = repository
        .prepare_checkouts(plans)
        .expect("prepare comparison checkouts");

    assert!(
        matches!(&checkouts.base, Checkout::Worktree { .. }),
        "expected base checkout to be a worktree, got {:?}",
        checkouts.base
    );
    assert!(
        matches!(&checkouts.head, Checkout::Worktree { .. }),
        "expected head checkout to be a worktree, got {:?}",
        checkouts.head
    );
    let base_git_root = checkouts.base.git_root();
    let head_git_root = checkouts.head.git_root();
    assert_ne!(
        base_git_root,
        head_git_root,
        "base and head worktrees must have distinct roots: base={}, head={}",
        base_git_root.display(),
        head_git_root.display()
    );
    let base_sha = run_git_output_for_test(base_git_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        base_sha, sha,
        "base worktree should be checked out at the requested commit"
    );
    let head_sha = run_git_output_for_test(head_git_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        head_sha, sha,
        "head worktree should be checked out at the requested commit"
    );
    let base_head = std::process::Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(base_git_root)
        .output()
        .expect("check base worktree HEAD");
    assert!(
        !base_head.status.success(),
        "base worktree HEAD should be detached\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&base_head.stdout),
        String::from_utf8_lossy(&base_head.stderr)
    );
    let head_head = std::process::Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(head_git_root)
        .output()
        .expect("check head worktree HEAD");
    assert!(
        !head_head.status.success(),
        "head worktree HEAD should be detached\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&head_head.stdout),
        String::from_utf8_lossy(&head_head.stderr)
    );
    let original_branch = run_git_output_for_test(&repo_dir, &["symbolic-ref", "--short", "HEAD"]);
    assert_eq!(
        original_branch, "main",
        "original checkout should remain on main after preparing comparison checkouts"
    );

    repository
        .cleanup_checkouts(checkouts)
        .expect("clean up comparison checkouts");
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
