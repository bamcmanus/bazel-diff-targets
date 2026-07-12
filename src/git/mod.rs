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

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn original_checkout(&self) -> &OriginalCheckout {
        &self.original_checkout
    }
}
