mod backend;
mod cli_backend;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use backend::GitBackend;
use cli_backend::CliGitBackend;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginalCheckout {
    Branch { name: String, sha: String },
    Detached { sha: String },
}

impl OriginalCheckout {
    fn sha(&self) -> &str {
        match self {
            Self::Branch { sha, .. } | Self::Detached { sha } => sha,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedRef {
    pub reference: String,
    pub sha: String,
}

impl ResolvedRef {
    fn worktree_plan(&self) -> CheckoutPlan {
        CheckoutPlan::Worktree {
            reference: self.reference.clone(),
            sha: self.sha.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedRefs {
    pub base: ResolvedRef,
    pub head: ResolvedRef,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CheckoutPlan {
    Current { git_root: PathBuf },
    Worktree { reference: String, sha: String },
}

#[derive(Debug, PartialEq, Eq)]
pub struct CheckoutPlans {
    pub base: CheckoutPlan,
    pub head: CheckoutPlan,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeadSelection<'a> {
    Dirty,
    Resolved(&'a ResolvedRef),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Checkout {
    Current {
        git_root: PathBuf,
    },
    Worktree {
        git_root: PathBuf,
        temporary_root: PathBuf,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Checkouts {
    pub base: Checkout,
    pub head: Checkout,
}

impl Checkout {
    pub fn git_root(&self) -> &Path {
        match self {
            Self::Current { git_root } | Self::Worktree { git_root, .. } => git_root,
        }
    }
}

pub struct GitRepository {
    root: PathBuf,
    original_checkout: OriginalCheckout,
    backend: CliGitBackend,
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

    pub fn is_working_tree_dirty(&self) -> Result<bool, AppError> {
        self.backend.is_working_tree_dirty(&self.root)
    }

    pub fn plan_checkouts(&self, base: &ResolvedRef, head: HeadSelection<'_>) -> CheckoutPlans {
        let current_sha = self.original_checkout.sha();
        let head_checkout_plan = match head {
            HeadSelection::Dirty => CheckoutPlan::Current {
                git_root: self.root.clone(),
            },
            HeadSelection::Resolved(head) if head.sha == current_sha => CheckoutPlan::Current {
                git_root: self.root.clone(),
            },
            HeadSelection::Resolved(head) => head.worktree_plan(),
        };

        CheckoutPlans {
            base: base.worktree_plan(),
            head: head_checkout_plan,
        }
    }

    pub(crate) fn remove_worktree(&self, worktree_root: &Path) -> Result<(), AppError> {
        self.backend.remove_worktree(&self.root, worktree_root)
    }

    pub(crate) fn cleanup_checkouts(&self, checkouts: Checkouts) -> Result<(), AppError> {
        let base_result = self.cleanup_checkout(checkouts.base);
        let head_result = self.cleanup_checkout(checkouts.head);

        match (base_result, head_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(AppError::CheckoutFailure { message: format!("failed to clean up base checkout: {error}") }),
            (Ok(()), Err(error)) => Err(AppError::CheckoutFailure { message: format!("failed to clean up head checkout: {error}") }),
            (Err(base_error), Err(head_error)) => Err(AppError::CheckoutFailure { message: format!("failed to clean up base checkout: {base_error}; failed to clean up head checkout: {head_error}") }),

        }
    }

    fn cleanup_checkout(&self, checkout: Checkout) -> Result<(), AppError> {
        match checkout {
            Checkout::Current { .. } => Ok(()),
            Checkout::Worktree {
                git_root,
                temporary_root,
            } => {
                self.remove_worktree(git_root.as_path())?;

                std::fs::remove_dir_all(temporary_root.as_path()).map_err(|error| {
                    AppError::CheckoutFailure {
                        message: format!(
                            "failed to remove temporary worktree directory {}: {error}",
                            temporary_root.display()
                        ),
                    }
                })
            }
        }
    }

    pub(crate) fn add_detached_worktree(
        &self,
        worktree_root: &Path,
        sha: &str,
    ) -> Result<(), AppError> {
        self.backend
            .add_detached_worktree(&self.root, worktree_root, sha)
    }

    pub(crate) fn prepare_checkouts(&self, plans: CheckoutPlans) -> Result<Checkouts, AppError> {
        let CheckoutPlans {
            base: base_plan,
            head: head_plan,
        } = plans;

        let base_checkout = self.prepare_checkout(base_plan)?;
        let head_checkout = match self.prepare_checkout(head_plan) {
            Ok(head_checkout) => head_checkout,
            Err(preparation_error) => match self.cleanup_checkout(base_checkout) {
                Ok(()) => return Err(preparation_error),
                Err(cleanup_error) => {
                    return Err(AppError::CheckoutFailure {
                        message: format!(
                            "failed to prepare head checkout: {preparation_error}; \
                            failed to clean up base checkout: {cleanup_error}"
                        ),
                    })
                }
            },
        };

        Ok(Checkouts {
            base: base_checkout,
            head: head_checkout,
        })
    }

    fn prepare_checkout(&self, plan: CheckoutPlan) -> Result<Checkout, AppError> {
        match plan {
            CheckoutPlan::Current { git_root } => Ok(Checkout::Current { git_root }),
            CheckoutPlan::Worktree { sha, .. } => {
                let temp_directory = tempfile::Builder::new()
                    .prefix("bazel-diff-targets-")
                    .tempdir()
                    .map_err(|error| AppError::CheckoutFailure {
                        message: format!("failed to create temporary worktree directory: {error}"),
                    })?;

                let worktree_root = temp_directory.path().join("worktree");

                self.add_detached_worktree(worktree_root.as_path(), sha.as_str())?;

                let temporary_root = temp_directory.keep();

                Ok(Checkout::Worktree {
                    git_root: worktree_root,
                    temporary_root,
                })
            }
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn original_checkout(&self) -> &OriginalCheckout {
        &self.original_checkout
    }
}
