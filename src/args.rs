use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(name = "bazel-diff-targets")]
#[command(version)]
#[command(about = "Compute impacted Bazel targets between two Git refs.")]
pub struct Args {
    #[arg(long = "base-ref")]
    pub base_ref: Option<String>,

    #[arg(long = "head-ref")]
    pub head_ref: Option<String>,

    #[arg(long = "workspace-path")]
    pub workspace_path: Option<PathBuf>,

    #[arg(long = "java-path")]
    pub java_path: Option<PathBuf>,

    #[arg(long = "bazel-path")]
    pub bazel_path: Option<PathBuf>,

    #[arg(long = "bazel-diff-jar")]
    pub bazel_diff_jar: Option<PathBuf>,

    #[arg(long = "target-type", action = ArgAction::Append, value_parser = parse_non_empty_trimmed)]
    pub target_types: Vec<String>,

    #[arg(long = "use-cquery")]
    pub use_cquery: bool,

    #[arg(
        long = "exclude-external-targets",
        conflicts_with = "include_external_targets"
    )]
    pub exclude_external_targets: bool,

    #[arg(long = "include-external-targets")]
    pub include_external_targets: bool,

    #[arg(long = "include-distance", requires = "json")]
    pub include_distance: bool,

    #[arg(long = "bazel-startup-options")]
    pub bazel_startup_options: Option<String>,

    #[arg(long = "bazel-command-options")]
    pub bazel_command_options: Option<String>,

    #[arg(long = "json")]
    pub json: bool,

    #[arg(long = "quiet")]
    pub quiet: bool,

    #[arg(long = "verbose")]
    pub verbose: bool,
}

fn parse_non_empty_trimmed(value: &str) -> Result<String, String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Err("target type must not be empty".to_owned())
    } else {
        Ok(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Args;

    #[test]
    fn parses_repeated_target_types() {
        let args = Args::parse_from([
            "bazel-diff-targets",
            "--target-type",
            "java_library",
            "--target-type",
            "rust_binary",
        ]);

        assert_eq!(args.target_types, ["java_library", "rust_binary"]);
    }

    #[test]
    fn trims_target_type_values() {
        let args = Args::parse_from(["bazel-diff-targets", "--target-type", " java_library "]);

        assert_eq!(args.target_types, ["java_library"]);
    }

    #[test]
    fn rejects_empty_target_type_values() {
        let error =
            Args::try_parse_from(["bazel-diff-targets", "--target-type", "   "]).unwrap_err();

        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn rejects_conflicting_external_target_flags() {
        let error = Args::try_parse_from([
            "bazel-diff-targets",
            "--exclude-external-targets",
            "--include-external-targets",
        ])
        .unwrap_err();

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn include_distance_requires_json() {
        let error = Args::try_parse_from(["bazel-diff-targets", "--include-distance"]).unwrap_err();

        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }
}
