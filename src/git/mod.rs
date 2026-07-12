mod backend;
mod cli_backend;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use backend::GitBackend;
use cli_backend::CliGitBackend;

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
    Current { git_root: PathBuf },
    Worktree { git_root: PathBuf },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Checkouts {
    pub base: Checkout,
    pub head: Checkout,
}

impl Checkout {
    pub fn git_root(&self) -> &Path {
        match self {
            Self::Current { git_root } | Self::Worktree { git_root } => git_root,
        }
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

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn original_checkout(&self) -> &OriginalCheckout {
        &self.original_checkout
    }
}
