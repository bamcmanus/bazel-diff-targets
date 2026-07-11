use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Java executable not found: {attempted}")]
    MissingJava { attempted: String },

    #[error("Bazel executable not found: {attempted}")]
    MissingBazel { attempted: String },

    #[error("Invalid Git ref: {reference}")]
    InvalidRef { reference: String },

    #[error("Could not infer base ref")]
    BaseRefInferenceFailure,

    #[error("Working tree is dirty")]
    DirtyWorkingTree {
        entries: Vec<String>,
        remaining: usize,
    },

    #[error("Shallow Git repositories are not supported")]
    ShallowRepository,

    #[error("Bare Git repositories are not supported")]
    BareRepository,

    #[error("Invalid workspace: {message}")]
    WorkspaceValidationFailure { message: String },

    #[error("Git checkout failed: {message}")]
    CheckoutFailure { message: String },

    #[error("bazel-diff failed: {message}")]
    BazelDiffExecutionFailure { message: String },

    #[error("Could not parse bazel-diff output: {message}")]
    OutputParsingFailure { message: String },
}

#[derive(Serialize)]
struct JsonErrorResponse<'a> {
    ok: bool,
    error: JsonErrorBody<'a>,
}

#[derive(Serialize)]
struct JsonErrorBody<'a> {
    kind: &'static str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MissingJava { .. } => "missing_java",
            Self::MissingBazel { .. } => "missing_bazel",
            Self::InvalidRef { .. } => "invalid_ref",
            Self::BaseRefInferenceFailure => "base_ref_inference_failure",
            Self::DirtyWorkingTree { .. } => "dirty_working_tree",
            Self::ShallowRepository => "shallow_repository",
            Self::BareRepository => "bare_repository",
            Self::WorkspaceValidationFailure { .. } => "workspace_validation_failure",
            Self::CheckoutFailure { .. } => "checkout_failure",
            Self::BazelDiffExecutionFailure { .. } => "bazel_diff_execution_failure",
            Self::OutputParsingFailure { .. } => "output_parsing_failure",
        }
    }

    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::MissingJava { .. } => {
                Some("Install Java, set JAVA_HOME, or pass --java-path /path/to/java.")
            }
            Self::MissingBazel { .. } => {
                Some("Install Bazel or Bazelisk, set BAZEL, or pass --bazel-path /path/to/bazel.")
            }
            Self::BaseRefInferenceFailure => Some("Pass --base-ref explicitly."),
            Self::DirtyWorkingTree { .. } => {
                Some("Commit, stash, or remove local changes before running bazel-diff-targets.")
            }
            Self::ShallowRepository => {
                Some("Use a non-shallow clone before running bazel-diff-targets.")
            }
            _ => None,
        }
    }

    pub fn format_human(&self) -> String {
        let mut output = format!("error: {self}");

        if let Some(hint) = self.hint() {
            output.push('\n');
            output.push_str("hint: ");
            output.push_str(hint);
        }

        if let Self::DirtyWorkingTree { entries, remaining } = self {
            for entry in entries {
                output.push('\n');
                output.push_str("  ");
                output.push_str(entry);
            }

            if *remaining > 0 {
                output.push('\n');
                output.push_str("  and ");
                output.push_str(&remaining.to_string());
                output.push_str(" more");
            }
        }

        output
    }

    pub fn format_json(&self) -> String {
        let message = self.to_string();
        let response = JsonErrorResponse {
            ok: false,
            error: JsonErrorBody {
                kind: self.kind(),
                message: &message,
                hint: self.hint(),
            },
        };

        serde_json::to_string(&response).expect("serializing AppError should not fail")
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn exposes_stable_error_kinds() {
        let cases = [
            (
                AppError::MissingJava {
                    attempted: "java".to_owned(),
                },
                "missing_java",
            ),
            (
                AppError::BaseRefInferenceFailure,
                "base_ref_inference_failure",
            ),
            (
                AppError::MissingBazel {
                    attempted: "bazel".to_owned(),
                },
                "missing_bazel",
            ),
            (
                AppError::InvalidRef {
                    reference: "not-a-ref".to_owned(),
                },
                "invalid_ref",
            ),
            (
                AppError::DirtyWorkingTree {
                    entries: vec!["M file.txt".to_owned()],
                    remaining: 0,
                },
                "dirty_working_tree",
            ),
            (AppError::ShallowRepository, "shallow_repository"),
            (AppError::BareRepository, "bare_repository"),
            (
                AppError::WorkspaceValidationFailure {
                    message: "missing Module.bazel".to_owned(),
                },
                "workspace_validation_failure",
            ),
            (
                AppError::CheckoutFailure {
                    message: "git checkout failed".to_owned(),
                },
                "checkout_failure",
            ),
            (
                AppError::BazelDiffExecutionFailure {
                    message: "java -jar failed".to_owned(),
                },
                "bazel_diff_execution_failure",
            ),
            (
                AppError::OutputParsingFailure {
                    message: "invalid JSON".to_owned(),
                },
                "output_parsing_failure",
            ),
        ];

        for (error, expected_kind) in cases {
            assert_eq!(error.kind(), expected_kind);
        }
    }

    #[test]
    fn provides_hints_for_actionable_errors() {
        let cases = [
            (
                AppError::MissingJava {
                    attempted: "java".to_owned(),
                },
                "Install Java, set JAVA_HOME, or pass --java-path /path/to/java.",
            ),
            (
                AppError::MissingBazel {
                    attempted: "bazel".to_owned(),
                },
                "Install Bazel or Bazelisk, set BAZEL, or pass --bazel-path /path/to/bazel.",
            ),
            (
                AppError::BaseRefInferenceFailure,
                "Pass --base-ref explicitly.",
            ),
            (
                AppError::DirtyWorkingTree {
                    entries: vec!["M file.txt".to_owned()],
                    remaining: 0,
                },
                "Commit, stash, or remove local changes before running bazel-diff-targets.",
            ),
            (
                AppError::ShallowRepository,
                "Use a non-shallow clone before running bazel-diff-targets.",
            ),
        ];

        for (error, hint) in cases {
            assert_eq!(error.hint(), Some(hint));
        }
    }

    #[test]
    fn omits_hints_when_no_specific_action_is_available() {
        let errors = [
            AppError::InvalidRef {
                reference: "not-a-ref".to_owned(),
            },
            AppError::BareRepository,
            AppError::WorkspaceValidationFailure {
                message: "missing Module.bazel".to_owned(),
            },
            AppError::CheckoutFailure {
                message: "git checkout failed".to_owned(),
            },
            AppError::BazelDiffExecutionFailure {
                message: "java -jar failed".to_owned(),
            },
            AppError::OutputParsingFailure {
                message: "invalid JSON".to_owned(),
            },
        ];

        for error in errors {
            assert_eq!(error.hint(), None);
        }
    }

    #[test]
    fn displays_human_readable_messages() {
        let cases = [
            (
                AppError::MissingJava {
                    attempted: "/missing/java".to_owned(),
                },
                "Java executable not found: /missing/java",
            ),
            (
                AppError::MissingBazel {
                    attempted: "bazel".to_owned(),
                },
                "Bazel executable not found: bazel",
            ),
            (
                AppError::InvalidRef {
                    reference: "not-a-ref".to_owned(),
                },
                "Invalid Git ref: not-a-ref",
            ),
            (
                AppError::DirtyWorkingTree {
                    entries: vec!["M file.txt".to_owned()],
                    remaining: 0,
                },
                "Working tree is dirty",
            ),
            (
                AppError::ShallowRepository,
                "Shallow Git repositories are not supported",
            ),
            (
                AppError::BareRepository,
                "Bare Git repositories are not supported",
            ),
            (
                AppError::WorkspaceValidationFailure {
                    message: "missing Module.bazel".to_owned(),
                },
                "Invalid workspace: missing Module.bazel",
            ),
            (
                AppError::CheckoutFailure {
                    message: "git checkout failed".to_owned(),
                },
                "Git checkout failed: git checkout failed",
            ),
            (
                AppError::BazelDiffExecutionFailure {
                    message: "java -jar failed".to_owned(),
                },
                "bazel-diff failed: java -jar failed",
            ),
            (
                AppError::OutputParsingFailure {
                    message: "invalid JSON".to_owned(),
                },
                "Could not parse bazel-diff output: invalid JSON",
            ),
        ];

        for (error, msg) in cases {
            assert_eq!(error.to_string(), msg);
        }
    }

    #[test]
    fn formats_human_error_with_hint() {
        let error = AppError::MissingJava {
            attempted: "java".to_owned(),
        };

        assert_eq!(error.format_human(), "error: Java executable not found: java\nhint: Install Java, set JAVA_HOME, or pass --java-path /path/to/java.");
    }

    #[test]
    fn formats_human_error_without_hint() {
        let error = AppError::InvalidRef {
            reference: "not-a-ref".to_owned(),
        };

        assert_eq!(error.format_human(), "error: Invalid Git ref: not-a-ref");
    }

    #[test]
    fn formats_dirty_working_tree_entries() {
        let error = AppError::DirtyWorkingTree {
            entries: vec!["M src/main.rs".to_owned(), "?? scratch.txt".to_owned()],
            remaining: 3,
        };

        assert_eq!(error.format_human(), "error: Working tree is dirty\nhint: Commit, stash, or remove local changes before running bazel-diff-targets.\n  M src/main.rs\n  ?? scratch.txt\n  and 3 more");
    }

    #[test]
    fn formats_json_error_with_hint() {
        let error = AppError::MissingJava {
            attempted: "java".to_owned(),
        };

        assert_eq!(
            error.format_json(),
            r#"{"ok":false,"error":{"kind":"missing_java","message":"Java executable not found: java","hint":"Install Java, set JAVA_HOME, or pass --java-path /path/to/java."}}"#
        );
    }

    #[test]
    fn formats_json_error_without_hint() {
        let error = AppError::InvalidRef {
            reference: "not-a-ref".to_owned(),
        };

        assert_eq!(
            error.format_json(),
            r#"{"ok":false,"error":{"kind":"invalid_ref","message":"Invalid Git ref: not-a-ref"}}"#
        );
    }
}
