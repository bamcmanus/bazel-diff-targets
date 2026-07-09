# bazel-diff-targets Design

This document captures the agreed design for a future standalone Rust CLI named
`bazel-diff-targets`. The CLI is intended to be developed in a new repository,
not in this GitHub Action repository.

## Purpose

`bazel-diff-targets` is a CI-neutral command-line tool that computes impacted
Bazel targets between two Git commits or refs using upstream
`Tinder/bazel-diff`.

The CLI is the reusable engine for:

- local developer usage;
- a future GitHub Action adapter;
- a future Buildkite plugin adapter;
- any other consumer that can invoke a CLI and parse stdout or JSON.

The CLI must not contain GitHub Actions or Buildkite-specific behavior. CI
adapters are responsible for their own platform-specific ref inference, metadata,
artifacts, and output mechanisms.

## Repository And Binary

- Repository name: `bazel-diff-targets`
- Binary name: `bazel-diff-targets`
- Language: Rust
- Build system: Bazel-first
- Bazel dependency mode: bzlmod / `MODULE.bazel`
- Versioning: SemVer, starting at `v0.1.0`
- Release management: release-please
- Public Rust API: none in MVP; binary crate only

Bazel is the authoritative build and release path. Cargo files may exist for
editor support or Rust-only checks, but the MVP does not promise that
`cargo build` produces the official functional release artifact.

## Command Shape

The CLI has a single top-level command. There is no required `run` subcommand.

Example:

```bash
bazel-diff-targets --base-ref origin/main --head-ref HEAD
```

The MVP uses long kebab-case flags only. There are no short aliases.

## Flags

### Ref And Workspace Flags

```text
--base-ref <ref>
--head-ref <ref>
--workspace-path <path>
```

Behavior:

- `--head-ref` defaults to the current `HEAD` commit.
- `--base-ref`, if omitted, is inferred for local usage.
- CI adapters should pass both `--base-ref` and `--head-ref` explicitly.
- If `--workspace-path` is omitted, it defaults to the Git repository root.
- If `--workspace-path` is explicitly provided, it is resolved relative to the
  current working directory.

### Toolchain Flags

```text
--java-path <path>
--bazel-path <path>
--bazel-diff-jar <path>
```

Java resolution order:

1. `--java-path`, if provided. It must work.
2. `$JAVA_HOME/bin/java`, if `JAVA_HOME` is set. It must work.
3. `java` on `PATH`.

Bazel resolution order:

1. `--bazel-path`, if provided.
2. `$BAZEL`, if set.
3. `bazel` on `PATH`.

`--bazel-diff-jar` overrides the embedded upstream `bazel-diff` JAR. The path
must exist and be a file. The MVP does not verify checksums for user-provided
JARs; callers are responsible for custom artifacts.

### Bazel-Diff Behavior Flags

```text
--target-type <type>
--use-cquery
--exclude-external-targets
--include-external-targets
--include-distance
--bazel-startup-options <string>
--bazel-command-options <string>
```

Rules:

- `--target-type` is repeatable.
- The CLI does not parse comma-separated target types.
- The GitHub Action adapter can translate its existing comma-separated input
  into repeated `--target-type` flags later.
- Target type values are trimmed and rejected if empty after trimming.
- External targets are excluded by default.
- `--exclude-external-targets` explicitly keeps the default.
- `--include-external-targets` disables external target exclusion.
- Passing both external target flags is an argument error.
- `--bazel-startup-options` and `--bazel-command-options` are single string
  pass-through flags matching upstream `bazel-diff` behavior.
- `--include-distance` is only valid with `--json`.

### Output And Logging Flags

```text
--json
--quiet
--verbose
--version
--help
```

## Ref Semantics

Refs are resolved before computation with:

```bash
git rev-parse --verify <ref>^{commit}
```

Allowed inputs:

- branch names;
- commit SHAs;
- tags that resolve to commits.

Rejected inputs:

- invalid refs;
- non-commit Git objects.

Resolved SHAs are used for checkout, logging, and JSON output.

### Local Base Ref Inference

If `--base-ref` is omitted, the CLI infers it from the selected head ref and the
origin default branch:

1. determine selected head ref;
2. resolve default branch from `origin/HEAD`, then `origin/main`, then
   `origin/master`;
3. compute `git merge-base <head-ref> <default-branch-ref>`.

If inference fails, the CLI must fail clearly and ask the user to pass
`--base-ref` explicitly.

The CLI does not run `git fetch` automatically.

## Git Repository Requirements

The MVP requires:

- current directory is inside a non-bare Git repository;
- repository is non-shallow;
- working tree is clean before any checkout mutation;
- clean means `git status --porcelain` is empty;
- untracked files count as dirty;
- ignored files do not count as dirty.

Dirty tree errors should show a capped list of dirty entries, such as the first
20 entries plus `and N more`.

There is no shallow repository override in the MVP.

There is no submodule management in the MVP. Users or CI must initialize
submodules before invoking the CLI if their Bazel workspace requires them.

## Checkout Strategy

The MVP mutates the current checkout, protected by the clean working tree
requirement.

Flow:

1. record the original branch if the run starts on a branch;
2. record the original commit SHA;
3. resolve head and base refs to SHAs;
4. checkout the head SHA if needed;
5. generate head hashes first;
6. checkout the base SHA;
7. generate base hashes;
8. compare hashes;
9. restore the original branch if the run started on a branch;
10. otherwise restore the original commit SHA.

The CLI uses `git checkout` for compatibility. It does not implement branch
movement detection. If restoration fails, it emits a loud error or warning.

Temporary worktree support is deferred.

Dirty working tree and uncommitted change support is deferred. A future design
should prefer a safe temporary-worktree strategy over automatic stash/pop.

## Workspace Behavior

- Git root is discovered from the current working directory.
- Git commands run from the Git root.
- Workspace path must canonicalize inside the Git root.
- Nested Bazel workspaces are allowed.
- Symlinked workspace paths are allowed if the canonicalized target remains
  inside the canonical Git root.
- Workspace path must contain one of:
  - `WORKSPACE`
  - `WORKSPACE.bazel`
  - `MODULE.bazel`

Bare repositories are unsupported.

Non-UTF-8 paths are not a first-class MVP concern. The implementation should use
`PathBuf` internally where practical and error clearly if a path must be rendered
as UTF-8 but cannot be.

## Embedded Upstream bazel-diff

The CLI bundles a pinned upstream `bazel-diff_deploy.jar` from
`Tinder/bazel-diff`.

Decisions:

- no runtime download support;
- no `--bazel-diff-version` flag in MVP;
- callers who want a custom upstream JAR must provide `--bazel-diff-jar`;
- a custom JAR is caller-trusted and not checksum-verified in MVP.

### Build-Time Pinning

Bazel fetches the upstream JAR with an exact URL and SHA256. The Rust binary
embeds the JAR bytes at compile time, likely via `include_bytes!`.

The bundled upstream `bazel-diff` version should be stored in one obvious place
and reported by `--version`.

### Runtime Extraction And Cache

Java requires a filesystem path for `java -jar`, so the CLI materializes the
embedded JAR at runtime.

Behavior:

- extract embedded JAR to a per-version user cache;
- verify checksum before reuse;
- rewrite if missing or corrupt;
- if cache is unavailable, fall back to a temp file.

## Java And Bazel

The CLI does not bundle a JRE. Java is required at runtime and must be documented
for macOS and Linux setup.

Bazel installation and version management are out of scope. Users are expected to
already have Bazel or Bazelisk available.

The CLI only validates Bazel with:

```bash
<bazel> --version
```

All query/cquery work is delegated to upstream `bazel-diff`.

The CLI inherits the parent environment and does not set special environment
variables for Bazel or bazel-diff in the MVP.

## Output Contract

### Text Mode

Default stdout contains impacted Bazel labels only:

```text
//foo:bar
//baz:qux
```

Rules:

- one target per line;
- trailing newline when non-empty;
- no stdout at all when there are no impacted targets;
- logs and errors go to stderr.

There are no explicit output file flags in the MVP. Users can redirect stdout.

### JSON Mode

`--json` prints a JSON summary to stdout on success.

Success shape:

```json
{
  "ok": true,
  "baseRef": "abc123",
  "headRef": "def456",
  "hasChanges": true,
  "targetCount": 2,
  "impactedTargets": [
    "//foo:bar",
    "//baz:qux"
  ]
}
```

No-target success shape:

```json
{
  "ok": true,
  "baseRef": "abc123",
  "headRef": "def456",
  "hasChanges": false,
  "targetCount": 0,
  "impactedTargets": []
}
```

### Distance Output

`--include-distance` requires `--json`. If `--include-distance` is passed without
`--json`, the CLI errors clearly.

When enabled:

- `impactedTargets` remains `string[]`;
- distance data appears in `impactedTargetDetails`.

Example:

```json
{
  "ok": true,
  "baseRef": "abc123",
  "headRef": "def456",
  "hasChanges": true,
  "targetCount": 1,
  "impactedTargets": ["//foo:bar"],
  "impactedTargetDetails": [
    {
      "target": "//foo:bar",
      "distance": 1
    }
  ]
}
```

The exact detail fields should be confirmed against upstream `bazel-diff` JSON
before implementation locks the schema.

## Exit Codes

- `0`: computation succeeded, including zero impacted targets.
- non-zero: actual failure.

No `grep`-style no-match non-zero behavior in the MVP. Predicate-style behavior,
such as `--exit-code`, is deferred.

## Errors

Default errors are human-readable text on stderr.

With `--json`, failures emit structured JSON on stderr and exit non-zero:

```json
{
  "ok": false,
  "error": {
    "kind": "missing_java",
    "message": "Java executable not found: java",
    "hint": "Install Java, or pass --java-path /path/to/java."
  }
}
```

The Rust implementation should use typed application errors with `thiserror` so
JSON error kinds are stable.

## Logging

- stdout is strictly reserved for final results;
- default stderr contains concise progress logs;
- `--quiet` suppresses non-error logs;
- `--quiet` does not suppress useful error details;
- `--verbose` prints exact commands and relevant paths;
- subprocess stdout must not pollute CLI stdout;
- subprocess stderr may stream by default unless quiet;
- no intentional colored output in MVP.

Documentation should warn users not to pass secrets in visible command options,
because `--verbose` prints arguments.

## Runtime Network

The CLI itself performs no intentional runtime network access.

No runtime:

- Git fetch;
- bazel-diff download;
- telemetry;
- update check.

Bazel itself may access the network depending on the workspace configuration.
Docs should call out that distinction.

## Temporary Files

The CLI creates a unique temp directory for intermediate artifacts:

- head hashes;
- base hashes;
- dependency edges file when distance output is enabled;
- impacted-target intermediate output.

The CLI reads final output into memory, emits the final result to stdout, and
cleans up only files and directories it created.

No `--keep-temp` in MVP.

## Release And Distribution

MVP distribution is GitHub Releases with prebuilt binaries.

Release archives include:

- `bazel-diff-targets` binary;
- `LICENSE`;
- `THIRD_PARTY_NOTICES.md`;
- README or install documentation.

Publish `SHA256SUMS` with release artifacts.

Supported platforms:

- macOS x86_64;
- macOS arm64;
- Linux x86_64;
- attempt Linux arm64 if feasible;
- prefer Linux musl binaries for portability, but fall back to GNU if musl
  complicates MVP release automation.

Windows is explicitly out of scope for MVP.

Follow-on distribution features:

- Homebrew;
- Cargo install support.

`--version` prints both CLI and bundled upstream versions:

```text
bazel-diff-targets 0.1.0
bundled bazel-diff <version>
```

## Licensing

The CLI repo uses MIT.

Because the binary bundles `Tinder/bazel-diff`, the repo includes
`THIRD_PARTY_NOTICES.md` with:

- bundled bazel-diff version;
- upstream source URL;
- full upstream license text.

Release archives include both `LICENSE` and `THIRD_PARTY_NOTICES.md`.

Follow-up cleanup for this GitHub Action repo:

- `LICENSE` is MIT;
- `package.json` currently says ISC;
- update package metadata later so it matches the actual license.

## CI And Testing

CI runs on Linux and macOS.

Test layers:

1. Rust unit tests for:
   - argument parsing;
   - ref inference;
   - output parsing;
   - error formatting;
   - command construction.
2. Integration/e2e tests with:
   - a small real Git repo;
   - a small real Bazel workspace;
   - the real embedded bazel-diff JAR;
   - assertions on impacted target output.
3. Release artifact smoke tests that:
   - build archives;
   - extract archives;
   - run `--help`;
   - run `--version`;
   - verify docs and checksums.

Release process:

- release-please creates release PRs, tags, changelog, and GitHub Releases;
- a separate workflow builds binaries and uploads release artifacts.

Commit policy:

- conventional commits enforced with commitlint.

Dependency updates:

- Renovate is enabled for GitHub Actions, Cargo, Bazel modules, and eventually
  upstream bazel-diff artifact updates if feasible.

## Suggested Rust Module Structure

```text
src/
  main.rs          # top-level orchestration and process exit behavior
  args.rs          # clap definitions
  error.rs         # typed errors plus human/JSON formatting
  git.rs           # git root/ref/clean/checkout logic
  toolchain.rs     # Java/Bazel resolution and validation
  bazel_diff.rs    # embedded JAR cache/extraction plus java -jar calls
  output.rs        # text/JSON parsing and formatting
```

Tests:

```text
tests/
  e2e.rs
```

Bazel files:

```text
MODULE.bazel
BUILD.bazel
third_party/bazel_diff/...
```

Likely Rust crates:

- `clap` for argument parsing;
- `serde` and `serde_json` for JSON;
- `thiserror` for typed errors;
- `tempfile` for temp directories;
- `directories` for cache directories.

## Future GitHub Action Adapter

Future phase, not part of CLI MVP.

The existing GitHub Action should become a thin adapter that:

- downloads a pinned prebuilt `bazel-diff-targets` release artifact;
- keeps existing action inputs and outputs stable;
- handles GitHub-specific base/head ref inference;
- passes explicit `--base-ref` and `--head-ref` to the CLI;
- preserves existing `workspace-path: "."` behavior by passing explicit
  `--workspace-path`;
- calls the CLI with `--json`;
- maps JSON fields to GitHub outputs;
- translates the existing comma-separated `target-type` input into repeated
  `--target-type` flags;
- starts honoring the existing `head-ref` action input.

The action release should pin the CLI version internally. A user override can be
a later advanced feature.

## Future Buildkite Plugin Adapter

Future phase, not part of CLI MVP.

The Buildkite plugin should:

- download a pinned prebuilt CLI release artifact;
- map plugin configuration to CLI flags;
- pass explicit `--base-ref` and `--head-ref`;
- call the CLI with `--json` if it wants structured metadata;
- own all Buildkite-specific metadata and artifact behavior.

## Deferred Features

- dirty working tree / uncommitted changes support;
- temporary worktree strategy;
- output file flags;
- shell completions;
- Homebrew distribution;
- Cargo install distribution;
- predicate-style `--exit-code`;
- checksum flag for custom JARs;
- config file support;
- runtime download support, if ever needed;
- Windows support.
