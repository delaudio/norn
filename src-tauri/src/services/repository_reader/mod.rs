//! Native repository exploration. Source authority is provider/root qualified.
mod blame;
mod content;
pub(crate) mod context;
mod diff;
mod find;
mod git;
mod inventory;
pub(crate) mod legacy;
mod manager;
#[cfg(test)]
mod tests;
use crate::local_review::{GitRunControl, LocalReviewCancellation};
use crate::services::bitbucket::workspace_prs::RepositoryIdentity;
pub use blame::BlameParams;
pub use content::{ContentParams, ContentState};
pub use context::Context;
pub use diff::DiffParams;
pub use find::FindParams;
pub use inventory::FilesParams;
pub use legacy::{
    RepositoryBlameLine, RepositoryFileContent, RepositoryFileDiff, RepositoryFileEntry,
    RepositoryFileStatus,
};
pub use manager::InfoParams;
pub(crate) use manager::Manager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
pub(crate) const MAX_FILES: usize = 50_000;
pub(crate) const MAX_FILE_BYTES: usize = 512 * 1024;
pub(crate) const MAX_METADATA: usize = 8 * 1024 * 1024;
pub(crate) const MAX_DIFF: usize = 8 * 1024 * 1024;
pub(crate) const CHUNK: usize = 32 * 1024;
fn invalid() -> String {
    "The repository preview could not be verified. Refresh the selected repository.".into()
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn control() -> GitRunControl {
    GitRunControl::new(
        Duration::from_secs(20),
        LocalReviewCancellation::new(),
        "Repository exploration",
    )
}
pub(crate) fn list_at(root: &Path) -> Result<Vec<RepositoryFileEntry>, String> {
    inventory::list_at(root, &control())
}
pub(crate) fn content_at(root: &Path, path: &str) -> Result<RepositoryFileContent, String> {
    content::legacy_read(root, path, &control())
}
pub(crate) fn diff_at(root: &Path, path: &str) -> Result<RepositoryFileDiff, String> {
    diff::legacy_diff(root, path, &control())
}
pub(crate) fn blame_at(root: &Path, path: &str) -> Result<Vec<RepositoryBlameLine>, String> {
    blame::legacy_blame(root, path, &control())
}
