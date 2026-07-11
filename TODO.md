# bazel-diff-targets Implementation TODO

This document breaks the `bazel-diff-targets` design into composable chunks of
work. The CLI is intended to be built in a new repository named
`bazel-diff-targets`.

## Phase 0: Repository Bootstrap

- [x] Create new `bazel-diff-targets` repository.
- [x] Add MIT `LICENSE`.
- [x] Add initial `README.md` with project purpose and MVP scope.
- [x] Add `DESIGN.md` copied from the agreed design.
- [x] Add `THIRD_PARTY_NOTICES.md` placeholder.
- [x] Add `.gitignore` for Rust, Bazel, and editor artifacts.
- [x] Add conventional commit guidance.
- [x] Add commitlint configuration.
- [x] Add release-please configuration.
- [x] Add Renovate configuration.

## Phase 1: Bazel And Rust Skeleton

- [x] Add `MODULE.bazel` using bzlmod.
- [x] Add `rules_rust` dependency.
- [x] Add initial `BUILD.bazel` for the Rust binary.
- [x] Add `Cargo.toml` for Rust tooling/editor support.
- [x] Create `src/main.rs` with `--help` / `--version` stub behavior.
- [x] Add CI job that builds the skeleton on Linux.
- [x] Add CI job that builds or checks the skeleton on macOS.

## Phase 2: Upstream bazel-diff Artifact Pinning

- [ ] Select initial upstream `Tinder/bazel-diff` release version.
- [ ] Add Bazel external artifact fetch with exact URL and SHA256.
- [ ] Add generated/constant bundled bazel-diff version metadata.
- [ ] Embed `bazel-diff_deploy.jar` bytes into the Rust binary at compile time.
- [ ] Add `--version` output including CLI version and bundled bazel-diff
      version.
- [ ] Update `THIRD_PARTY_NOTICES.md` with upstream source URL, bundled version,
      and full upstream license text.

## Phase 3: Argument Parsing

- [x] Add `clap` dependency.
- [x] Implement `src/args.rs`.
- [x] Add top-level flags:
  - [x] `--base-ref`
  - [x] `--head-ref`
  - [x] `--workspace-path`
  - [x] `--dirty`
  - [x] `--java-path`
  - [x] `--bazel-path`
  - [x] `--bazel-diff-jar`
  - [x] repeated `--target-type`
  - [x] `--use-cquery`
  - [x] `--exclude-external-targets`
  - [x] `--include-external-targets`
  - [x] `--include-distance`
  - [x] `--bazel-startup-options`
  - [x] `--bazel-command-options`
  - [x] `--json`
  - [x] `--quiet`
  - [x] `--verbose`
- [x] Enforce conflicts:
  - [x] `--exclude-external-targets` conflicts with
        `--include-external-targets`.
  - [x] `--include-distance` requires `--json`.
  - [x] `--dirty` conflicts with `--head-ref`.
- [x] Validate repeated `--target-type` values are non-empty after trimming.
- [x] Add unit tests for argument parsing and validation.

## Phase 4: Error Model

- [x] Add `thiserror` dependency.
- [x] Implement `src/error.rs` with typed application errors.
- [x] Define stable error kinds for expected failures:
  - [x] missing Java
  - [x] missing Bazel
  - [x] invalid ref
  - [x] base ref inference failure
  - [x] dirty working tree
  - [x] shallow repository
  - [x] bare repository
  - [x] workspace validation failure
  - [x] checkout failure
  - [x] bazel-diff execution failure
  - [x] output parsing failure
- [x] Implement human-readable stderr formatting.
- [x] Implement JSON error formatting for `--json` mode.
- [x] Add unit tests for text and JSON error output.

## Phase 5: Git Repository Logic

- [x] Implement `src/git.rs` Git backend helpers.
- [x] Discover Git root.
- [x] Reject bare repositories.
- [x] Reject shallow repositories before base inference.
- [x] Capture current branch when the run starts on a branch.
- [x] Capture current `HEAD` commit SHA.
- [x] Resolve refs to commit SHAs.
- [x] Infer local base ref from merge-base with:
  - [x] `origin/HEAD`
  - [x] `origin/main`
  - [x] `origin/master`
- [ ] Implement dirty working tree detection.
- [ ] Treat untracked files as dirty.
- [ ] Ignore ignored files.
- [ ] Include capped dirty file list in errors.
- [ ] Enforce clean working tree unless `--dirty` is passed.
- [ ] Reject `--dirty` with explicit `--head-ref`.
- [ ] Model prepared comparison workspaces for base and head.
- [ ] Use current workspace for dirty head mode.
- [ ] Use current workspace when it already represents the selected clean side.
- [ ] Create temporary detached worktree for base when needed.
- [ ] Create temporary detached worktree for explicit/non-current head when
      needed.
- [ ] Map workspace path into temporary worktrees by preserving the
      Git-root-relative path.
- [ ] Remove temporary worktrees and files created by the CLI during cleanup.
- [ ] Add unit tests with fake command runner where practical.
- [ ] Add integration tests for ref inference in a temp Git repo.
- [ ] Add integration tests for dirty mode using the current workspace.
- [ ] Add integration tests for temporary worktree creation and cleanup.

## Phase 6: Workspace Validation

- [ ] Resolve default workspace path to Git root when omitted.
- [ ] Resolve explicit `--workspace-path` relative to current working directory.
- [ ] Canonicalize workspace path.
- [ ] Verify workspace path is inside Git root.
- [ ] Verify workspace path is a directory.
- [ ] Verify workspace marker exists:
  - [ ] `WORKSPACE`
  - [ ] `WORKSPACE.bazel`
  - [ ] `MODULE.bazel`
- [ ] Add tests for nested workspace paths.
- [ ] Add tests for symlinked workspace paths resolving inside/outside repo.

## Phase 7: Toolchain Resolution

- [ ] Implement `src/toolchain.rs`.
- [ ] Resolve Java from:
  - [ ] `--java-path`
  - [ ] `$JAVA_HOME/bin/java`
  - [ ] `java`
- [ ] Validate Java with `<java> -version`.
- [ ] Resolve Bazel from:
  - [ ] `--bazel-path`
  - [ ] `$BAZEL`
  - [ ] `bazel`
- [ ] Validate Bazel with `<bazel> --version`.
- [ ] Ensure explicit `--java-path` failures do not silently fall back.
- [ ] Ensure invalid `JAVA_HOME` failures are surfaced clearly.
- [ ] Add unit tests for resolution order.

## Phase 8: Embedded JAR Cache And Custom JAR

- [ ] Implement `src/bazel_diff.rs` embedded JAR materialization.
- [ ] Add `directories` dependency for user cache paths.
- [ ] Extract embedded JAR to per-version cache directory.
- [ ] Verify checksum before cache reuse.
- [ ] Rewrite missing or corrupt cached JAR.
- [ ] Fall back to temp file if cache is unavailable.
- [ ] Validate `--bazel-diff-jar` exists and is a file.
- [ ] Use custom JAR path directly when provided.
- [ ] Add tests for cache path selection and custom JAR validation.

## Phase 9: bazel-diff Invocation

- [ ] Build `generate-hashes` arguments:
  - [ ] `-w <workspace-path>`
  - [ ] `-b <bazel-path>`
  - [ ] `--useCquery` when requested
  - [ ] `--excludeExternalTargets` unless external targets are included
  - [ ] `-tt <comma-joined target types>` when target types exist
  - [ ] `-so <bazel-startup-options>` when provided
  - [ ] `-co <bazel-command-options>` when provided
  - [ ] `--depEdgesFile <path>` when distance output is enabled
- [ ] Build `get-impacted-targets` arguments:
  - [ ] `-w <workspace-path>`
  - [ ] `-o <output-path>`
  - [ ] `-fh <head-hashes>`
  - [ ] `-sh <base-hashes>`
  - [ ] `--excludeExternalTargets` unless external targets are included
  - [ ] `-tt <comma-joined target types>` when target types exist
  - [ ] `--depEdgesFile <path>` when distance output is enabled
- [ ] Ensure subprocess stdout never pollutes CLI stdout.
- [ ] Stream useful subprocess stderr by default.
- [ ] Suppress non-error subprocess output in `--quiet` mode.
- [ ] Print exact commands in `--verbose` mode.
- [ ] Add tests for command construction.

## Phase 10: Main Orchestration

- [ ] Implement validation order:
  - [ ] parse args
  - [ ] discover repo and workspace
  - [ ] reject bare/shallow repo
  - [ ] resolve/infer refs
  - [ ] validate dirty/clean working tree mode
  - [ ] validate Java/Bazel/JAR
  - [ ] prepare comparison workspaces
  - [ ] perform generate/compare work
- [ ] Generate head hashes first.
- [ ] Use current workspace for dirty head mode.
- [ ] Avoid unnecessary worktree creation when a selected side is already
      current and clean.
- [ ] Generate base hashes from prepared base workspace.
- [ ] Run impacted target comparison.
- [ ] Ensure cleanup removes only temp worktrees/directories/files created by
      the CLI.
- [ ] Add failure tests for temporary worktree cleanup errors where practical.

## Phase 11: Output Formatting

- [ ] Implement `src/output.rs`.
- [ ] Text mode:
  - [ ] emit one target per line;
  - [ ] emit trailing newline when non-empty;
  - [ ] emit no stdout when empty.
- [ ] JSON mode success shape:
  - [ ] `ok`
  - [ ] `baseRef`
  - [ ] `headRef`
  - [ ] `hasChanges`
  - [ ] `targetCount`
  - [ ] `impactedTargets`
- [ ] Distance JSON shape:
  - [ ] keep `impactedTargets` as `string[]`;
  - [ ] add `impactedTargetDetails`.
- [ ] Confirm upstream distance JSON shape before locking schema.
- [ ] Add unit tests for empty, text, JSON, and distance outputs.

## Phase 12: End-To-End Tests

- [ ] Create small Bazel workspace fixture.
- [ ] Create temp Git repo in test.
- [ ] Commit base state.
- [ ] Commit head state with impacted target change.
- [ ] Run real built binary using embedded JAR.
- [ ] Assert text output target list.
- [ ] Assert JSON output summary.
- [ ] Add no-impacted-targets test.
- [ ] Add explicit `--base-ref HEAD~3 --head-ref HEAD~1` style test.
- [ ] Add `--dirty` test that includes uncommitted workspace changes.
- [ ] Ensure user's original checkout is not mutated after run.

## Phase 13: Documentation

- [ ] Document install from GitHub Releases.
- [ ] Document supported platforms:
  - [ ] macOS x86_64
  - [ ] macOS arm64
  - [ ] Linux x86_64
  - [ ] Linux arm64 if supported
- [ ] Document Windows as out of scope.
- [ ] Document Java requirement and setup:
  - [ ] macOS
  - [ ] Linux
- [ ] Document Bazel/Bazelisk requirement and `--bazel-path` / `$BAZEL`.
- [ ] Document no runtime network initiated by the CLI.
- [ ] Document temporary worktree behavior and cleanup.
- [ ] Document clean working tree requirement unless `--dirty` is passed.
- [ ] Document `--dirty` current-workspace semantics.
- [ ] Document stdout/stderr consumer contract.
- [ ] Document JSON schema.
- [ ] Document exit code semantics.
- [ ] Document embedded bazel-diff version and custom `--bazel-diff-jar`
      override.
- [ ] Document that no GitHub Actions or Buildkite behavior exists in the CLI.

## Phase 14: CI And Release Automation

- [ ] Add Linux CI for build, unit tests, and e2e tests.
- [ ] Add macOS CI for build/checks and tests where feasible.
- [ ] Add release-please workflow.
- [ ] Add release artifact workflow triggered by release/tag.
- [ ] Build macOS x86_64 artifact.
- [ ] Build macOS arm64 artifact.
- [ ] Build Linux x86_64 artifact.
- [ ] Attempt Linux arm64 artifact.
- [ ] Prefer musl Linux targets; fall back to GNU if necessary.
- [ ] Package archives with binary, `LICENSE`, `THIRD_PARTY_NOTICES.md`, and
      docs.
- [ ] Generate `SHA256SUMS`.
- [ ] Upload artifacts to GitHub Release.
- [ ] Smoke-test release archives by extracting and running:
  - [ ] `bazel-diff-targets --help`
  - [ ] `bazel-diff-targets --version`

## Phase 15: Future GitHub Action Adapter

- [ ] Update this GitHub Action to download a pinned CLI release artifact.
- [ ] Keep existing action inputs stable.
- [ ] Keep existing action outputs stable.
- [ ] Preserve `workspace-path: "."` action behavior by passing explicit path.
- [ ] Keep GitHub-specific ref inference in the action wrapper.
- [ ] Pass explicit `--base-ref` and `--head-ref` to the CLI.
- [ ] Start honoring existing `head-ref` action input.
- [ ] Translate comma-separated action `target-type` input into repeated
      `--target-type` flags.
- [ ] Invoke CLI with `--json`.
- [ ] Map JSON output to GitHub outputs.
- [ ] Pin CLI version internally to the action release.

## Phase 16: Future Buildkite Plugin Adapter

- [ ] Create separate Buildkite plugin repository.
- [ ] Download pinned CLI release artifact.
- [ ] Map plugin config to CLI flags.
- [ ] Pass explicit `--base-ref` and `--head-ref`.
- [ ] Use `--json` if plugin needs structured metadata.
- [ ] Keep Buildkite metadata/artifact behavior inside the plugin.

## Deferred CLI Features

- [ ] Parallel base/head hash generation.
- [ ] Output file flags.
- [ ] Shell completions.
- [ ] Homebrew distribution.
- [ ] Cargo install distribution.
- [ ] Predicate-style `--exit-code`.
- [ ] Custom JAR checksum flag.
- [ ] Config file support.
- [ ] Runtime download support, if ever needed.
- [ ] Windows support.
