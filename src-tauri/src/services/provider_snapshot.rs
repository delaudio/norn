//! Immutable transient provider evidence, independent of saved review run IDs.
pub(crate) mod durable;
mod images;
use super::bitbucket::workspace_prs::{DetailParams, RepositoryIdentity};
pub use images::{BrowserSnapshotSource, ImageParams, ImageSide};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) const PATCH_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const FILE_LIMIT: usize = 10_000;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
const CACHE_COUNT: usize = 16;
const TTL: Duration = Duration::from_secs(600);
const CHUNK_BYTES: usize = 64 * 1024;
pub(crate) fn invalid() -> String {
    "The provider snapshot could not be verified. Refresh explicitly; no working-tree fallback was used.".into()
}
pub(crate) fn full_sha(value: &str) -> Result<String, String> {
    if !matches!(value.len(), 40 | 64) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(value.to_ascii_lowercase())
}
pub(crate) fn safe_path(path: &str) -> bool {
    if path.is_empty()
        || path.len() > 4096
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || (path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && path.as_bytes().get(1) == Some(&b':'))
    {
        return false;
    }
    !path
        .split('/')
        .any(|part| part.is_empty() || matches!(part, "." | ".."))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotRef {
    pub repository: RepositoryIdentity,
    pub pr_id: u32,
    pub snapshot_id: String,
}
impl SnapshotRef {
    pub fn validate(&self) -> Result<(), String> {
        DetailParams {
            repository: self.repository.clone(),
            pr_id: self.pr_id,
        }
        .validate()?;
        if self.snapshot_id.len() != 64
            || !self
                .snapshot_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserParams {
    pub snapshot: SnapshotRef,
}
impl BrowserParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilesParams {
    pub snapshot: SnapshotRef,
    pub offset: usize,
    pub limit: usize,
}
impl FilesParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if self.offset > FILE_LIMIT || !(1..=100).contains(&self.limit) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffParams {
    pub snapshot: SnapshotRef,
    pub path: String,
    pub offset: usize,
}
impl DiffParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if !safe_path(&self.path) || self.offset > PATCH_BYTES {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SnapshotFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Capture {
    pub request: DetailParams,
    pub title: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub head_sha: String,
    pub destination_sha: String,
    pub base_sha: String,
    #[serde(skip)]
    pub patch: String,
    pub expected_count: usize,
    pub expected_files: Option<Vec<SnapshotFile>>,
    pub source_repository: Option<RepositoryIdentity>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotInfo {
    pub request: DetailParams,
    pub snapshot: SnapshotRef,
    pub title: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub head_sha: String,
    pub destination_sha: String,
    pub base_sha: String,
    pub patch_sha256: String,
    pub file_count: usize,
    pub patch_bytes: usize,
    pub expires_at: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilesPage {
    pub request: FilesParams,
    pub files: Vec<SnapshotFile>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffPage {
    pub request: DiffParams,
    pub text: String,
    pub total_bytes: usize,
    pub content_sha256: String,
    pub next_offset: Option<usize>,
}
pub(crate) struct SnapshotData {
    pub info: SnapshotInfo,
    pub patch: String,
    source_repository: Option<RepositoryIdentity>,
    files: Vec<Section>,
    bytes: usize,
    created: Instant,
}
impl SnapshotData {
    pub(crate) fn source_repository(&self) -> Option<&RepositoryIdentity> {
        self.source_repository.as_ref()
    }
}

pub(crate) struct Section {
    file: SnapshotFile,
    start: usize,
    end: usize,
    content_sha256: String,
}
impl Section {
    pub(crate) fn parts(&self) -> (&SnapshotFile, std::ops::Range<usize>) {
        (&self.file, self.start..self.end)
    }
    pub(crate) fn path(&self) -> &str {
        &self.file.path
    }
}
pub(crate) struct Manager {
    entries: Mutex<VecDeque<Arc<SnapshotData>>>,
    saved: Mutex<VecDeque<(String, SnapshotRef)>>,
    maximum: usize,
    count: usize,
    ttl: Duration,
    images: images::RasterCache,
    remote_gate: Arc<std::sync::atomic::AtomicUsize>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            saved: Mutex::new(VecDeque::new()),
            maximum: CACHE_BYTES,
            count: CACHE_COUNT,
            ttl: TTL,
            images: images::RasterCache::default(),
            remote_gate: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Manager {
    pub(crate) fn stop(&self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    fn prune(&self, entries: &mut VecDeque<Arc<SnapshotData>>) {
        entries.retain(|entry| entry.created.elapsed() < self.ttl);
    }
    pub fn insert(&self, capture: Capture) -> Result<SnapshotInfo, String> {
        if self.shutdown.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Provider snapshot canceled during shutdown.".into());
        }
        capture.request.validate()?;
        if let Some(source) = &capture.source_repository {
            source.validate()?;
            if source.provider != capture.request.repository.provider {
                return Err(invalid());
            }
        }
        let head_sha = full_sha(&capture.head_sha)?;
        let base_sha = full_sha(&capture.base_sha)?;
        let destination_sha = full_sha(&capture.destination_sha)?;
        if head_sha.len() != base_sha.len()
            || head_sha.len() != destination_sha.len()
            || capture.patch.len() > PATCH_BYTES
            || capture.expected_count > FILE_LIMIT
        {
            return Err(invalid());
        }
        if self.count == 0
            || capture.title.len() > 4096
            || capture.title.chars().any(char::is_control)
            || [&capture.source_branch, &capture.destination_branch]
                .iter()
                .any(|branch| {
                    branch.is_empty() || branch.len() > 256 || branch.chars().any(char::is_control)
                })
        {
            return Err(invalid());
        }
        let mut files = super::bitbucket::workspace_prs::snapshot_sections(&capture.patch)?;
        for section in &mut files {
            section.content_sha256 = hash(&capture.patch.as_bytes()[section.start..section.end]);
        }
        if files.len() != capture.expected_count {
            return Err(format!("Provider diff contains {} file sections, but its inventory reports {}. Refresh the PR; no review was started.", files.len(), capture.expected_count));
        }
        if let Some(expected) = capture.expected_files {
            if expected.len() != files.len() {
                return Err(invalid());
            }
            let mut expected: std::collections::HashMap<_, _> = expected
                .into_iter()
                .map(|file| (file.path.clone(), file))
                .collect();
            if expected.len() != files.len() {
                return Err(invalid());
            }
            for section in &files {
                let file = expected.remove(&section.file.path).ok_or_else(invalid)?;
                if file.old_path != section.file.old_path
                    || file.status != section.file.status
                    || (!section.file.binary
                        && (file.additions != section.file.additions
                            || file.deletions != section.file.deletions))
                {
                    return Err("Provider diff disagrees with its file inventory (paths, status or line counts). Refresh the PR; no review was started.".into());
                }
            }
        }
        let inventory_bytes =
            serde_json::to_vec(&files.iter().map(|file| &file.file).collect::<Vec<_>>())
                .map_err(|_| invalid())?
                .len();
        if inventory_bytes > PATCH_BYTES {
            return Err(invalid());
        }
        let source_bytes = capture
            .source_repository
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| invalid())?
            .map_or(0, |bytes| bytes.len());
        let bytes = inventory_bytes + capture.patch.len() + source_bytes;
        if bytes > self.maximum {
            return Err("Provider snapshot exceeds the evidence cache limit.".into());
        }
        let mut nonce = [0; 32];
        getrandom::getrandom(&mut nonce).map_err(|_| invalid())?;
        let snapshot = SnapshotRef {
            repository: capture.request.repository.clone(),
            pr_id: capture.request.pr_id,
            snapshot_id: hash(&nonce),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid())?
            .as_millis();
        let info = SnapshotInfo {
            request: capture.request,
            snapshot,
            title: capture.title,
            source_branch: capture.source_branch,
            destination_branch: capture.destination_branch,
            head_sha,
            destination_sha,
            base_sha,
            patch_sha256: hash(capture.patch.as_bytes()),
            file_count: files.len(),
            patch_bytes: capture.patch.len(),
            expires_at: u64::try_from(now + self.ttl.as_millis()).map_err(|_| invalid())?,
        };
        let data = Arc::new(SnapshotData {
            info: info.clone(),
            patch: capture.patch,
            source_repository: capture.source_repository,
            files,
            bytes,
            created: Instant::now(),
        });
        let mut entries = self.entries.lock().map_err(|_| invalid())?;
        self.prune(&mut entries);
        while entries.len() >= self.count
            || entries.iter().map(|entry| entry.bytes).sum::<usize>() + bytes > self.maximum
        {
            entries.pop_front();
        }
        entries.push_back(data);
        Ok(info)
    }
    pub fn get(&self, snapshot: &SnapshotRef) -> Result<Arc<SnapshotData>, String> {
        snapshot.validate()?;
        if self.shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Provider snapshot canceled during shutdown.".into());
        }
        let mut entries = self.entries.lock().map_err(|_| invalid())?;
        self.prune(&mut entries);
        let index = entries.iter().position(|entry| entry.info.snapshot == *snapshot).ok_or_else(|| "This provider snapshot is expired or unavailable. Refresh the PR preview explicitly.".to_string())?;
        let data = entries.remove(index).ok_or_else(invalid)?;
        entries.push_back(data.clone());
        Ok(data)
    }
    pub(crate) fn inventory(&self, snapshot: &SnapshotRef) -> Result<Vec<SnapshotFile>, String> {
        let data = self.get(snapshot)?;
        Ok(data
            .files
            .iter()
            .map(|section| section.file.clone())
            .collect())
    }
    pub fn files(&self, request: FilesParams) -> Result<FilesPage, String> {
        request.validate()?;
        let data = self.get(&request.snapshot)?;
        if request.offset > data.files.len() {
            return Err(invalid());
        }
        let maximum_end = (request.offset + request.limit).min(data.files.len());
        let mut files = Vec::new();
        let mut bytes = 0;
        for section in &data.files[request.offset..maximum_end] {
            let size = serde_json::to_vec(&section.file)
                .map_err(|_| invalid())?
                .len();
            if bytes + size > 384 * 1024 {
                break;
            }
            bytes += size;
            files.push(section.file.clone());
        }
        let end = request.offset + files.len();
        if end == request.offset && end < data.files.len() {
            return Err(invalid());
        }
        Ok(FilesPage {
            files,
            total: data.files.len(),
            next_offset: (end < data.files.len()).then_some(end),
            request,
        })
    }

    pub fn diff(&self, request: DiffParams) -> Result<DiffPage, String> {
        request.validate()?;
        let data = self.get(&request.snapshot)?;
        let section = data
            .files
            .iter()
            .find(|section| section.file.path == request.path)
            .ok_or_else(invalid)?;
        let patch = &data.patch[section.start..section.end];
        if request.offset > patch.len() || !patch.is_char_boundary(request.offset) {
            return Err(invalid());
        }
        let mut end = (request.offset + CHUNK_BYTES).min(patch.len());
        while !patch.is_char_boundary(end) {
            end -= 1;
        }
        Ok(DiffPage {
            text: patch[request.offset..end].into(),
            total_bytes: patch.len(),
            content_sha256: section.content_sha256.clone(),
            next_offset: (end < patch.len()).then_some(end),
            request,
        })
    }
}
pub(crate) fn section(file: SnapshotFile, start: usize, end: usize) -> Section {
    Section {
        file,
        start,
        end,
        content_sha256: String::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::ReviewProvider;
    const PATCH: &str = include_str!("../../tests/fixtures/provider_snapshot.patch");
    pub(crate) fn capture(provider: ReviewProvider) -> Capture {
        Capture {
            request: DetailParams {
                repository: RepositoryIdentity {
                    provider,
                    workspace: "Owner".into(),
                    repo: "Repo".into(),
                },
                pr_id: 7,
            },
            title: "Owned PR".into(),
            source_branch: "feature".into(),
            destination_branch: "main".into(),
            head_sha: "a".repeat(40),
            destination_sha: "b".repeat(40),
            base_sha: "c".repeat(40),
            patch: PATCH.into(),
            expected_count: 5,
            source_repository: None,
            expected_files: None,
        }
    }
    fn page(snapshot: SnapshotRef, offset: usize, limit: usize) -> FilesParams {
        FilesParams {
            snapshot,
            offset,
            limit,
        }
    }
    #[test]
    fn complete_owned_git_inventory_preserves_paths_modes_binary_and_distinct_base() {
        let manager = Manager::default();
        let info = manager.insert(capture(ReviewProvider::Github)).unwrap();
        assert_ne!(info.base_sha, info.destination_sha);
        assert_eq!(info.patch_sha256, hash(PATCH.as_bytes()));
        assert_eq!(info.file_count, 5);
        let first = manager.files(page(info.snapshot.clone(), 0, 2)).unwrap();
        assert_eq!(first.next_offset, Some(2));
        assert_eq!(first.files[0].path, "added 世界.txt");
        assert_eq!(first.files[0].additions, 2);
        assert!(first.files[1].binary);
        let second = manager.files(page(info.snapshot.clone(), 2, 100)).unwrap();
        assert_eq!(
            second
                .files
                .iter()
                .map(|f| f.status.as_str())
                .collect::<Vec<_>>(),
            ["deleted", "modified", "renamed"]
        );
        assert_eq!(second.files[2].old_path.as_deref(), Some("old name è.txt"));
        assert_eq!(second.next_offset, None);
        for file in first.files.iter().chain(second.files.iter()) {
            let diff = manager
                .diff(DiffParams {
                    snapshot: info.snapshot.clone(),
                    path: file.path.clone(),
                    offset: 0,
                })
                .unwrap();
            assert!(PATCH.contains(&diff.text));
            assert_eq!(diff.content_sha256, hash(diff.text.as_bytes()));
            assert_eq!(diff.total_bytes, diff.text.len());
        }
        assert!(!PATCH.contains("destination-only"));
    }
    #[test]
    fn scoped_handles_fail_on_wrong_provider_pr_path_offset_restart_and_eviction() {
        let manager = Manager {
            count: 2,
            ..Manager::default()
        };
        let first = manager.insert(capture(ReviewProvider::Github)).unwrap();
        let other = manager.insert(capture(ReviewProvider::Bitbucket)).unwrap();
        let mut wrong = first.snapshot.clone();
        wrong.repository.provider = ReviewProvider::Bitbucket;
        assert!(manager.get(&wrong).is_err());
        wrong = first.snapshot.clone();
        wrong.pr_id += 1;
        assert!(manager.get(&wrong).is_err());
        assert!(Manager::default().get(&first.snapshot).is_err());
        assert!(manager
            .diff(DiffParams {
                snapshot: first.snapshot.clone(),
                path: "../file".into(),
                offset: 0
            })
            .is_err());
        assert!(manager
            .diff(DiffParams {
                snapshot: first.snapshot.clone(),
                path: "missing".into(),
                offset: 0
            })
            .is_err());
        assert!(manager.files(page(first.snapshot.clone(), 6, 1)).is_err());
        manager.get(&first.snapshot).unwrap();
        manager.insert(capture(ReviewProvider::Github)).unwrap();
        assert!(manager.get(&other.snapshot).is_err());
        assert!(manager.get(&first.snapshot).is_ok());
    }
    #[test]
    fn cache_lifetime_bytes_and_inventory_completeness_are_enforced() {
        let expired = Manager {
            ttl: Duration::ZERO,
            ..Manager::default()
        };
        let info = expired.insert(capture(ReviewProvider::Github)).unwrap();
        assert!(expired.get(&info.snapshot).is_err());
        assert!(Manager {
            maximum: 10,
            ..Manager::default()
        }
        .insert(capture(ReviewProvider::Github))
        .is_err());
        let mut c = capture(ReviewProvider::Github);
        c.expected_count = 4;
        assert!(Manager::default().insert(c).is_err());
        let mut c = capture(ReviewProvider::Github);
        c.expected_files = Some(vec![]);
        assert!(Manager::default().insert(c).is_err());
    }
    #[test]
    fn utf8_chunks_reassemble_exact_bytes_and_reject_mid_character_offsets() {
        let manager = Manager::default();
        let mut c = capture(ReviewProvider::Github);
        let content = "世".repeat(24_000);
        c.patch = format!(
            "diff --git a/large b/large\n--- a/large\n+++ b/large\n@@ -0,0 +1 @@\n+{content}\n"
        );
        c.expected_count = 1;
        let expected = c.patch.clone();
        let info = manager.insert(c).unwrap();
        let mut offset = 0;
        let mut assembled = String::new();
        loop {
            let chunk = manager
                .diff(DiffParams {
                    snapshot: info.snapshot.clone(),
                    path: "large".into(),
                    offset,
                })
                .unwrap();
            assert!(chunk.text.len() <= CHUNK_BYTES);
            assert_eq!(chunk.content_sha256, hash(expected.as_bytes()));
            assembled.push_str(&chunk.text);
            if let Some(next) = chunk.next_offset {
                assert!(next > offset);
                offset = next;
            } else {
                break;
            }
        }
        assert_eq!(assembled, expected);
        let invalid_offset = expected.find('世').unwrap() + 1;
        assert!(manager
            .diff(DiffParams {
                snapshot: info.snapshot,
                path: "large".into(),
                offset: invalid_offset
            })
            .is_err());
    }
    #[test]
    fn parses_actual_git_suppressed_empty_context() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("old")).unwrap();
        std::fs::create_dir(dir.path().join("new")).unwrap();
        std::fs::write(dir.path().join("old/x"), "before\n\ntail\n").unwrap();
        std::fs::write(dir.path().join("new/x"), "after\n\ntail\n").unwrap();
        let output = std::process::Command::new("git")
            .args([
                "-c",
                "diff.suppressBlankEmpty=true",
                "diff",
                "--no-index",
                "--no-ext-diff",
                "--",
                "old/x",
                "new/x",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let patch = String::from_utf8(output.stdout).unwrap();
        assert!(patch.contains("\n\n tail\n"));
        let sections = super::super::bitbucket::workspace_prs::snapshot_sections(&patch).unwrap();
        assert_eq!(sections.len(), 1);
        let (file, range) = sections[0].parts();
        assert_eq!((file.additions, file.deletions), (1, 1));
        assert_eq!(file.path, "new/x");
        assert_eq!(file.old_path.as_deref(), Some("old/x"));
        assert_eq!(&patch[range], patch);
    }
    #[test]
    fn rejects_truncated_contradictory_duplicate_and_hostile_patch_metadata() {
        let malformed = [
            "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n",
            "diff --git a/x b/y\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/../x b/../x\nold mode 100644\nnew mode 100755\n",
            "diff --git a/x b/x\nold mode 100644\n",
            "diff --git a/x b/x\nBinary files a/evil and b/evil differ\n",
            "diff --git a/x b/x\nnew file mode 100644\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n",
            "diff --git a/x b/x\nindex bogus..ffff\nold mode 100644\nnew mode 100755\n",
        ];
        for patch in malformed {
            assert!(
                super::super::bitbucket::workspace_prs::snapshot_sections(patch).is_err(),
                "accepted: {patch}"
            );
        }
        assert!(
            super::super::bitbucket::workspace_prs::snapshot_sections(&format!("{PATCH}{PATCH}"))
                .is_err()
        );
    }
    #[test]
    fn inventory_pages_stay_bounded_after_json_escape_expansion() {
        let manager = Manager::default();
        let mut c = capture(ReviewProvider::Github);
        let mut patch = String::new();
        for index in 0..100 {
            let path = format!("{index}{}", "\"".repeat(4000));
            let old = serde_json::to_string(&format!("a/{path}")).unwrap();
            let new = serde_json::to_string(&format!("b/{path}")).unwrap();
            patch.push_str(&format!(
                "diff --git {old} {new}\nold mode 100644\nnew mode 100755\n"
            ));
        }
        c.patch = patch;
        c.expected_count = 100;
        let info = manager.insert(c).unwrap();
        let first = manager.files(page(info.snapshot.clone(), 0, 100)).unwrap();
        assert!(!first.files.is_empty());
        assert!(first.files.len() < 100);
        assert!(serde_json::to_vec(&first).unwrap().len() < 512 * 1024);
        assert_eq!(first.next_offset, Some(first.files.len()));
        let mut offset = first.files.len();
        while offset < 100 {
            let next = manager
                .files(page(info.snapshot.clone(), offset, 100))
                .unwrap();
            assert!(serde_json::to_vec(&next).unwrap().len() < 512 * 1024);
            offset += next.files.len();
            assert_eq!(next.next_offset, (offset < 100).then_some(offset));
        }
    }
    #[test]
    fn copy_empty_add_delete_and_binary_add_have_verified_metadata() {
        let cases = [
            ("diff --git a/old b/new\nsimilarity index 100%\ncopy from old\ncopy to new\n", "copied", Some("old"), false),
            ("diff --git a/empty b/empty\nnew file mode 100644\nindex 0000000..e69de29\n", "added", None, false),
            ("diff --git a/empty b/empty\ndeleted file mode 100644\nindex e69de29..0000000\n", "deleted", None, false),
            ("diff --git a/image b/image\nnew file mode 100644\nindex 0000000..1234567\nBinary files /dev/null and b/image differ\n", "added", None, true),
        ];
        for (patch, status, old, binary) in cases {
            let mut c = capture(ReviewProvider::Github);
            c.patch = patch.into();
            c.expected_count = 1;
            let manager = Manager::default();
            let info = manager.insert(c).unwrap();
            let files = manager.files(page(info.snapshot, 0, 100)).unwrap();
            assert_eq!(files.files[0].status, status);
            assert_eq!(files.files[0].old_path.as_deref(), old);
            assert_eq!(files.files[0].binary, binary);
        }
    }
}
