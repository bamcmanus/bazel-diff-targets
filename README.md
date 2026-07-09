# bazel-diff-targets

`bazel-diff-targets` is a CI-neutral command-line tool that computes impacted
Bazel targets between two Git commits or refs using upstream
[`Tinder/bazel-diff`](https://github.com/Tinder/bazel-diff).

The CLI is intended to be the reusable engine for local development workflows,
GitHub Actions adapters, Buildkite plugin adapters, and other automation that
can invoke a CLI and parse stdout or JSON.

## MVP scope

The MVP is a standalone Rust CLI that:

- accepts explicit or locally inferred Git refs;
- validates the Git repository and Bazel workspace;
- invokes an embedded, pinned upstream `bazel-diff` JAR;
- prints impacted Bazel targets to stdout in text mode;
- optionally prints a JSON summary with `--json`.

CI-specific behavior is intentionally out of scope for the CLI. GitHub Actions,
Buildkite, and other CI integrations should live in thin adapters that call this
binary.
