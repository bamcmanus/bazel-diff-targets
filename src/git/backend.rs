use std::path::{Path, PathBuf};

use crate::error::AppError;

pub(super) trait GitBackend {
    fn discover_root(&self, current_dir: &Path) -> Result<PathBuf, AppError>;
    fn current_branch(&self, root: &Path) -> Result<Option<String>, AppError>;
    fn current_sha(&self, root: &Path) -> Result<String, AppError>;
    fn resolve_ref(&self, root: &Path, reference: &str) -> Result<String, AppError>;
    fn is_bare_repository(&self, root: &Path) -> Result<bool, AppError>;
    fn is_shallow_repository(&self, root: &Path) -> Result<bool, AppError>;
    fn merge_base(
        &self,
        root: &Path,
        left_ref: &str,
        right_ref: &str,
    ) -> Result<Option<String>, AppError>;
    fn is_working_tree_dirty(&self, root: &Path) -> Result<bool, AppError>;
}
