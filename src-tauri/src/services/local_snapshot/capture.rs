//! Native immutable captures. A saved read never consults the current checkout.
use super::*;
use crate::local_review::{
    local_branch_review_snapshot_for_verified_path, local_review_snapshot_for_verified_path,
    LocalReviewSnapshot,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredLayer {
    kind: Layer,
    patch: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    format_version: u32,
    root: PathBuf,
    info: Info,
    layers: Vec<StoredLayer>,
}
struct Section {
    file: File,
    layer_index: usize,
    range: std::ops::Range<usize>,
}
pub(crate) struct Data {
    pub info: Info,
    root: PathBuf,
    layers: Vec<StoredLayer>,
    files: Vec<Section>,
    bytes: usize,
}
pub(crate) struct Manager {
    startup_root: PathBuf,
    shutdown: AtomicBool,
    options_cancel: LocalReviewCancellation,
    active: Mutex<HashMap<String, (Source, LocalReviewCancellation)>>,
    cache: Mutex<VecDeque<Arc<Data>>>,
}
#[cfg(test)]
struct CaptureGuard<'a> {
    manager: &'a Manager,
    id: String,
}
#[cfg(test)]
impl Drop for CaptureGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.manager.active.lock() {
            active.remove(&self.id);
        }
    }
}
pub(crate) struct Ticket {
    manager: Arc<Manager>,
    request: CancelParams,
    cancellation: LocalReviewCancellation,
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if let Ok(mut active) = self.manager.active.lock() {
            active.remove(&self.request.capture_id);
        }
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn identity(stored: &Stored) -> Result<String, String> {
    let mut content = stored.clone();
    content.info.snapshot.snapshot_id.clear();
    content.info.local_review_id = 0;
    let bytes = serde_json::to_vec(&("norn-local-snapshot-v1", content)).map_err(|_| invalid())?;
    Ok(hash(&bytes))
}
fn review_id(id: &str) -> Result<u32, String> {
    Ok(u32::from_str_radix(&id[..8], 16).map_err(|_| invalid())? | (1 << 31))
}
impl Stored {
    fn validate(self, expected: &SnapshotRef) -> Result<Data, String> {
        expected.validate()?;
        self.info.comparison.validate()?;
        let info = &self.info;
        if self.format_version != 1
            || info.snapshot != *expected
            || info.comparison.source != expected.source
            || !self.root.is_absolute()
            || root_id(&self.root)? != expected.root_id
            || !valid_id(&info.native_snapshot_sha256)
            || identity(&self)? != expected.snapshot_id
            || review_id(&expected.snapshot_id)? != info.local_review_id
            || info.branch.is_empty()
            || info.branch.len() > 256
            || info.branch.chars().any(char::is_control)
            || info.warnings.len() > 128
            || info
                .warnings
                .iter()
                .any(|w| w.len() > 512 || w.chars().any(char::is_control))
            || self.layers.len() != info.comparison.layers.len()
            || info.layers.len() != self.layers.len()
        {
            return Err(invalid());
        }
        super::super::provider_snapshot::full_sha(&info.base_sha)?;
        if let Some(head) = &info.head_sha {
            super::super::provider_snapshot::full_sha(head)?;
        }
        let mut files = Vec::new();
        let mut patch_bytes = 0;
        for (index, layer) in self.layers.iter().enumerate() {
            if layer.kind != info.comparison.layers[index] || info.layers[index].kind != layer.kind
            {
                return Err(invalid());
            }
            patch_bytes += layer.patch.len();
            if patch_bytes > PATCH_BYTES {
                return Err(invalid());
            }
            let sections = super::super::bitbucket::workspace_prs::snapshot_sections_with_limit(
                &layer.patch,
                PATCH_BYTES,
            )?;
            if sections.len() != info.layers[index].file_count
                || layer.patch.len() != info.layers[index].patch_bytes
            {
                return Err(invalid());
            }
            for section in sections {
                let (file, range) = section.parts();
                files.push(Section {
                    file: File {
                        layer: layer.kind,
                        path: file.path.clone(),
                        old_path: file.old_path.clone(),
                        status: file.status.clone(),
                        additions: file.additions,
                        deletions: file.deletions,
                        binary: file.binary,
                    },
                    layer_index: index,
                    range,
                });
            }
        }
        if files.len() > FILE_LIMIT
            || files.len() != info.file_count
            || patch_bytes != info.patch_bytes
        {
            return Err(invalid());
        }
        let bytes = serde_json::to_vec(&self).map_err(|_| invalid())?.len();
        if bytes > METADATA_BYTES {
            return Err(invalid());
        }
        Ok(Data {
            info: self.info,
            root: self.root,
            layers: self.layers,
            files,
            bytes,
        })
    }
}
impl Data {
    fn stored(&self) -> Stored {
        Stored {
            format_version: 1,
            root: self.root.clone(),
            info: self.info.clone(),
            layers: self.layers.clone(),
        }
    }
}
impl Manager {
    pub(crate) fn new(startup_root: PathBuf) -> Self {
        Self {
            startup_root,
            shutdown: AtomicBool::new(false),
            options_cancel: LocalReviewCancellation::new(),
            active: Mutex::new(HashMap::new()),
            cache: Mutex::new(VecDeque::new()),
        }
    }
    fn check(&self) -> Result<(), String> {
        if self.shutdown.load(Ordering::Acquire) {
            Err("Local snapshot operation canceled during shutdown.".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn stopped(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
    pub(crate) fn stop(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.options_cancel.cancel();
        if let Ok(active) = self.active.lock() {
            for (_, cancellation) in active.values() {
                cancellation.cancel();
            }
        }
    }
    pub(crate) fn options(&self, request: OptionsParams) -> Result<OptionsResult, String> {
        self.check()?;
        let result = super::options(request, &self.startup_root, self.options_cancel.clone())?;
        self.check()?;
        Ok(result)
    }
    pub(crate) fn cancel(&self, request: CancelParams) -> Result<bool, String> {
        request.validate()?;
        self.check()?;
        let active = self.active.lock().map_err(|_| invalid())?;
        if let Some((source, cancellation)) = active.get(&request.capture_id) {
            if *source != request.source {
                return Err(invalid());
            }
            cancellation.cancel();
            Ok(true)
        } else {
            Ok(false)
        }
    }
    #[cfg(test)]
    fn begin(
        &self,
        request: &CaptureParams,
    ) -> Result<(CaptureGuard<'_>, LocalReviewCancellation), String> {
        request.validate()?;
        self.check()?;
        let mut active = self.active.lock().map_err(|_| invalid())?;
        if active.len() >= 2 || active.contains_key(&request.capture_id) {
            return Err("A local capture is already running. Wait or cancel it explicitly.".into());
        }
        let cancel = LocalReviewCancellation::new();
        active.insert(
            request.capture_id.clone(),
            (request.comparison.source.clone(), cancel.clone()),
        );
        Ok((
            CaptureGuard {
                manager: self,
                id: request.capture_id.clone(),
            },
            cancel,
        ))
    }
    pub(crate) fn reserve(self: &Arc<Self>, request: CancelParams) -> Result<Ticket, String> {
        request.validate()?;
        self.check()?;
        let mut active = self.active.lock().map_err(|_| invalid())?;
        if active.len() >= 2 || active.contains_key(&request.capture_id) {
            return Err("A local capture is already running. Wait or cancel it explicitly.".into());
        }
        let cancellation = LocalReviewCancellation::new();
        active.insert(
            request.capture_id.clone(),
            (request.source.clone(), cancellation.clone()),
        );
        Ok(Ticket {
            manager: self.clone(),
            request,
            cancellation,
        })
    }
    pub(crate) fn capture_reserved(
        &self,
        request: CaptureParams,
        ticket: Ticket,
    ) -> Result<CaptureResult, String> {
        request.validate()?;
        if ticket.request.capture_id != request.capture_id
            || ticket.request.source != request.comparison.source
        {
            return Err(invalid());
        }
        self.capture_inner(request, ticket.cancellation.clone())
    }
    pub(crate) fn recheck_reserved(
        &self,
        request: RecheckParams,
        ticket: Ticket,
    ) -> Result<RecheckResult, String> {
        request.validate()?;
        if ticket.request.capture_id != request.capture_id
            || ticket.request.source != request.snapshot.source
        {
            return Err(invalid());
        }
        self.recheck_inner(request, ticket.cancellation.clone())
    }
    fn collect(
        &self,
        comparison: &Comparison,
        cancel: LocalReviewCancellation,
    ) -> Result<Data, String> {
        comparison.validate()?;
        self.check()?;
        let started = Instant::now();
        let control = GitRunControl::new(TIMEOUT, cancel.clone(), "Local snapshot capture");
        let resolved = resolve_source(&comparison.source, &self.startup_root, &control)?;
        if let Some(base) = &comparison.base_ref {
            let candidates = git_text(
                &resolved.root,
                &[
                    "for-each-ref",
                    "--format=%(refname:short)",
                    "refs/heads",
                    "refs/remotes",
                ],
                &control,
            )?;
            let branch = git_text(
                &resolved.root,
                &["symbolic-ref", "--quiet", "--short", "HEAD"],
                &control,
            )
            .ok();
            control.check()?;
            let candidates = candidate_refs(&candidates, branch.as_deref())?;
            if !candidates.contains(base) {
                return Err("Select a listed local or remote-tracking base branch.".into());
            }
        }
        let collect = || -> Result<LocalReviewSnapshot, String> {
            self.check()?;
            control.check()?;
            let remaining = TIMEOUT.checked_sub(started.elapsed()).ok_or_else(invalid)?;
            let repo = &resolved.repository;
            let result = match comparison.mode {
                Mode::WorkingTree => local_review_snapshot_for_verified_path(
                    repo.provider,
                    &repo.workspace,
                    &repo.repo,
                    &resolved.root,
                    cancel.clone(),
                    remaining,
                ),
                Mode::Branch => local_branch_review_snapshot_for_verified_path(
                    repo.provider,
                    &repo.workspace,
                    &repo.repo,
                    &resolved.root,
                    comparison.base_ref.as_deref().ok_or_else(invalid)?,
                    cancel.clone(),
                    remaining,
                ),
            };
            result.map_err(|_| "Local capture failed or was canceled. Check the selected comparison and retry explicitly.".into())
        };
        let first = collect()?;
        let second = collect()?;
        if first.snapshot_sha256 != second.snapshot_sha256 {
            return Err("The local comparison changed during capture. Refresh explicitly.".into());
        }
        let after = resolve_source(&comparison.source, &self.startup_root, &control)?;
        if after.root != resolved.root || after.repository != resolved.repository {
            return Err(invalid());
        }
        self.check()?;
        control.check()?;
        let mut layers = Vec::new();
        let mut layer_info = Vec::new();
        for kind in &comparison.layers {
            let native = first.layers.iter().find(|layer| layer.kind == *kind);
            // Empty layers remain explicit, including a working-tree committed layer
            // when no upstream changes exist; absence never switches comparison mode.
            let patch = native.map_or_else(String::new, |layer| layer.diff.clone());
            let sections = super::super::bitbucket::workspace_prs::snapshot_sections_with_limit(
                &patch,
                PATCH_BYTES,
            )?;
            if native.is_some_and(|layer| layer.diffstat.len() != sections.len()) {
                return Err(invalid());
            }
            layers.push(StoredLayer { kind: *kind, patch });
            layer_info.push(LayerInfo {
                kind: *kind,
                file_count: sections.len(),
                patch_bytes: layers.last().ok_or_else(invalid)?.patch.len(),
            });
        }
        let mut warnings = Vec::new();
        let mut truncated = false;
        for warning in first.warnings {
            if warnings.len() >= 128 || warning.len() > 512 || warning.chars().any(char::is_control)
            {
                truncated = true;
                continue;
            }
            warnings.push(warning);
        }
        let mut stored = Stored {
            format_version: 1,
            root: resolved.root,
            info: Info {
                comparison: comparison.clone(),
                snapshot: SnapshotRef {
                    source: comparison.source.clone(),
                    repository: resolved.repository,
                    root_id: resolved.root_id,
                    snapshot_id: String::new(),
                },
                branch: first.current_branch,
                head_sha: first.head_sha,
                base_sha: first.base_sha,
                native_snapshot_sha256: first.snapshot_sha256,
                local_review_id: 0,
                file_count: layer_info.iter().map(|layer| layer.file_count).sum(),
                patch_bytes: layer_info.iter().map(|layer| layer.patch_bytes).sum(),
                layers: layer_info,
                warnings,
                warnings_truncated: truncated,
            },
            layers,
        };
        let id = identity(&stored)?;
        stored.info.snapshot.snapshot_id = id.clone();
        stored.info.local_review_id = review_id(&id)?;
        let reference = stored.info.snapshot.clone();
        stored.validate(&reference)
    }
    fn cache(&self, data: Arc<Data>) -> Result<(), String> {
        self.check()?;
        let mut cache = self.cache.lock().map_err(|_| invalid())?;
        cache.retain(|entry| entry.info.snapshot != data.info.snapshot);
        while cache.len() >= 16
            || cache.iter().map(|entry| entry.bytes).sum::<usize>() + data.bytes > 64 * 1024 * 1024
        {
            cache.pop_front();
        }
        cache.push_back(data);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn capture(&self, request: CaptureParams) -> Result<CaptureResult, String> {
        let (_guard, cancel) = self.begin(&request)?;
        self.capture_inner(request, cancel)
    }
    fn capture_inner(
        &self,
        request: CaptureParams,
        cancel: LocalReviewCancellation,
    ) -> Result<CaptureResult, String> {
        let data = Arc::new(self.collect(&request.comparison, cancel)?);
        self.check()?;
        let json = serde_json::to_string(&data.stored()).map_err(|_| invalid())?;
        crate::review_storage::local_evidence::save(&data.info, &json)?;
        self.check()?;
        self.cache(data.clone())?;
        Ok(CaptureResult {
            request,
            info: data.info.clone(),
        })
    }
    pub(crate) fn get(&self, snapshot: &SnapshotRef) -> Result<Arc<Data>, String> {
        snapshot.validate()?;
        self.check()?;
        // Validate durable ownership even for cached data: disconnect/retention
        // cannot leave an otherwise reachable snapshot after its native row is gone.
        let json = crate::review_storage::local_evidence::load(snapshot)?.ok_or_else(|| {
            "Saved local evidence is unavailable. Capture the selected comparison explicitly."
                .to_string()
        })?;
        if let Some(cached) = self
            .cache
            .lock()
            .map_err(|_| invalid())?
            .iter()
            .find(|entry| entry.info.snapshot == *snapshot)
            .cloned()
        {
            if serde_json::to_string(&cached.stored()).map_err(|_| invalid())? == json {
                return Ok(cached);
            }
        }
        let stored: Stored = serde_json::from_str(&json).map_err(|_| invalid())?;
        let data = Arc::new(stored.validate(snapshot)?);
        self.cache(data.clone())?;
        Ok(data)
    }
    pub(crate) fn info(&self, request: LookupParams) -> Result<InfoResult, String> {
        request.validate()?;
        let data = self.get(&request.snapshot)?;
        Ok(InfoResult {
            request,
            info: data.info.clone(),
        })
    }
    #[cfg(test)]
    pub(crate) fn recheck(&self, request: RecheckParams) -> Result<RecheckResult, String> {
        request.validate()?;
        let capture = CaptureParams {
            capture_id: request.capture_id.clone(),
            comparison: Comparison {
                source: request.snapshot.source.clone(),
                mode: Mode::WorkingTree,
                base_ref: None,
                layers: vec![Layer::Committed],
            },
        };
        let (_guard, cancel) = self.begin(&capture)?;
        self.recheck_inner(request, cancel)
    }
    fn recheck_inner(
        &self,
        request: RecheckParams,
        cancel: LocalReviewCancellation,
    ) -> Result<RecheckResult, String> {
        let saved = self.get(&request.snapshot)?;
        let current = self.collect(&saved.info.comparison, cancel)?;
        if current.info.snapshot.root_id != saved.info.snapshot.root_id
            || current.info.snapshot.repository != saved.info.snapshot.repository
            || current.info.native_snapshot_sha256 != saved.info.native_snapshot_sha256
        {
            return Err("The saved local comparison is stale. Capture it again explicitly before starting an operation.".into());
        }
        self.check()?;
        Ok(RecheckResult {
            request,
            info: saved.info.clone(),
            unchanged: true,
        })
    }
    pub(crate) fn files(&self, request: FilesParams) -> Result<FilesPage, String> {
        request.validate()?;
        let data = self.get(&request.snapshot)?;
        if request.offset > data.files.len() {
            return Err(invalid());
        }
        let end = (request.offset + request.limit).min(data.files.len());
        let files = data.files[request.offset..end]
            .iter()
            .map(|section| section.file.clone())
            .collect();
        Ok(FilesPage {
            request,
            files,
            total: data.files.len(),
            next_offset: (end < data.files.len()).then_some(end),
        })
    }
    pub(crate) fn diff(&self, request: DiffParams) -> Result<DiffPage, String> {
        request.validate()?;
        let data = self.get(&request.snapshot)?;
        let section = data
            .files
            .iter()
            .find(|section| {
                section.file.layer == request.layer && section.file.path == request.path
            })
            .ok_or_else(invalid)?;
        let patch = &data.layers[section.layer_index].patch[section.range.clone()];
        if request.offset > patch.len() || !patch.is_char_boundary(request.offset) {
            return Err(invalid());
        }
        let mut end = (request.offset + 64 * 1024).min(patch.len());
        while !patch.is_char_boundary(end) {
            end -= 1;
        }
        Ok(DiffPage {
            text: patch[request.offset..end].into(),
            total_bytes: patch.len(),
            content_sha256: hash(patch.as_bytes()),
            next_offset: (end < patch.len()).then_some(end),
            request,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    struct Owned {
        root: tempfile::TempDir,
        _data: tempfile::TempDir,
        previous: Option<std::ffi::OsString>,
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var("NORN_REVIEW_DATA_DIR", v),
                None => std::env::remove_var("NORN_REVIEW_DATA_DIR"),
            }
        }
    }
    fn git(root: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "owned fixture Git: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    impl Owned {
        fn new(committed: bool) -> Self {
            let root = tempfile::tempdir().unwrap();
            let data = tempfile::tempdir().unwrap();
            let previous = std::env::var_os("NORN_REVIEW_DATA_DIR");
            std::env::set_var("NORN_REVIEW_DATA_DIR", data.path());
            git(root.path(), &["init"]);
            git(root.path(), &["config", "user.name", "Owned"]);
            git(
                root.path(),
                &["config", "user.email", "owned@example.invalid"],
            );
            git(
                root.path(),
                &[
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/owned/same.git",
                ],
            );
            if committed {
                std::fs::write(root.path().join("same.txt"), "base\n").unwrap();
                git(root.path(), &["add", "same.txt"]);
                git(
                    root.path(),
                    &["-c", "commit.gpgsign=false", "commit", "-m", "owned base"],
                );
                git(root.path(), &["branch", "comparison"]);
            }
            Self {
                root,
                _data: data,
                previous,
            }
        }
        fn manager(&self) -> Manager {
            Manager::new(self.root.path().to_path_buf())
        }
    }
    fn request(mode: Mode) -> CaptureParams {
        CaptureParams {
            capture_id: "a".repeat(64),
            comparison: Comparison {
                source: Source::Current {},
                mode,
                base_ref: (mode == Mode::Branch).then(|| "comparison".into()),
                layers: if mode == Mode::Branch {
                    vec![Layer::Committed]
                } else {
                    vec![Layer::Committed, Layer::Staged, Layer::Unstaged]
                },
            },
        }
    }
    fn diff(manager: &Manager, reference: &SnapshotRef, layer: Layer, path: &str) -> String {
        manager
            .diff(DiffParams {
                snapshot: reference.clone(),
                layer,
                path: path.into(),
                offset: 0,
            })
            .unwrap()
            .text
    }
    #[test]
    fn overlapping_layers_restore_exact_bytes_and_recheck_never_reads_fresh_as_saved() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        std::fs::write(owned.root.path().join("same.txt"), "staged\n").unwrap();
        git(owned.root.path(), &["add", "same.txt"]);
        std::fs::write(owned.root.path().join("same.txt"), "unstaged 世界\n").unwrap();
        std::fs::write(
            owned.root.path().join("untracked.txt"),
            "private untracked\n",
        )
        .unwrap();
        let manager = owned.manager();
        let result = manager.capture(request(Mode::WorkingTree)).unwrap();
        let reference = &result.info.snapshot;
        let files = manager
            .files(FilesParams {
                snapshot: reference.clone(),
                offset: 0,
                limit: 32,
            })
            .unwrap();
        assert_eq!(
            files.files.iter().map(|f| f.layer).collect::<Vec<_>>(),
            [Layer::Staged, Layer::Unstaged]
        );
        assert!(diff(&manager, reference, Layer::Staged, "same.txt").contains("+staged"));
        let old = diff(&manager, reference, Layer::Unstaged, "same.txt");
        assert!(old.contains("+unstaged 世界"));
        assert!(!old.contains("private untracked"));
        let fresh = owned.manager();
        assert_eq!(
            fresh
                .info(LookupParams {
                    snapshot: reference.clone()
                })
                .unwrap()
                .info,
            result.info
        );
        fresh
            .recheck(RecheckParams {
                snapshot: reference.clone(),
                capture_id: "b".repeat(64),
            })
            .unwrap();
        std::fs::write(owned.root.path().join("same.txt"), "changed later\n").unwrap();
        assert_eq!(diff(&fresh, reference, Layer::Unstaged, "same.txt"), old);
        assert!(fresh
            .recheck(RecheckParams {
                snapshot: reference.clone(),
                capture_id: "c".repeat(64)
            })
            .unwrap_err()
            .contains("stale"));
        let mut wrong = reference.clone();
        wrong.repository.provider = crate::config::ReviewProvider::Bitbucket;
        assert!(fresh.get(&wrong).is_err());
        wrong = reference.clone();
        wrong.root_id = "f".repeat(64);
        assert!(fresh.get(&wrong).is_err());
        assert!(fresh
            .diff(DiffParams {
                snapshot: reference.clone(),
                layer: Layer::Committed,
                path: "same.txt".into(),
                offset: 0
            })
            .is_err());
        let backup = tempfile::tempdir().unwrap();
        let path = backup.path().join("owned.sqlite3");
        crate::review_storage::create_consistent_database_backup(&path).unwrap();
        crate::review_storage::verify_database_backup(&path).unwrap();
        assert_eq!(
            rusqlite::Connection::open(path)
                .unwrap()
                .query_row("SELECT COUNT(*) FROM local_review_evidence", [], |row| {
                    row.get::<_, usize>(0)
                })
                .unwrap(),
            1
        );
        let conn = crate::review_storage::publication_database().unwrap();
        conn.execute("DELETE FROM ai_review_stores WHERE target_kind='local'", [])
            .unwrap();
        assert!(manager.get(reference).is_err());
    }
    #[test]
    fn branch_comparison_is_committed_only_and_pins_same_comparison_after_dirty_changes() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        std::fs::write(owned.root.path().join("same.txt"), "committed\n").unwrap();
        git(owned.root.path(), &["add", "same.txt"]);
        git(
            owned.root.path(),
            &["-c", "commit.gpgsign=false", "commit", "-m", "owned change"],
        );
        std::fs::write(owned.root.path().join("same.txt"), "staged excluded\n").unwrap();
        git(owned.root.path(), &["add", "same.txt"]);
        std::fs::write(owned.root.path().join("same.txt"), "unstaged excluded\n").unwrap();
        let manager = owned.manager();
        let result = manager.capture(request(Mode::Branch)).unwrap();
        let patch = diff(
            &manager,
            &result.info.snapshot,
            Layer::Committed,
            "same.txt",
        );
        assert!(patch.contains("+committed"));
        assert!(!patch.contains("excluded"));
        assert_eq!(result.info.layers.len(), 1);
        std::fs::write(owned.root.path().join("same.txt"), "changed excluded\n").unwrap();
        manager
            .recheck(RecheckParams {
                snapshot: result.info.snapshot.clone(),
                capture_id: "c".repeat(64),
            })
            .unwrap();
        let mut invalid = request(Mode::Branch);
        invalid.comparison.base_ref = Some("HEAD~1".into());
        assert!(manager.capture(invalid).is_err());
        git(owned.root.path(), &["checkout", "--detach", "HEAD"]);
        let detached = manager.capture(request(Mode::Branch)).unwrap();
        assert_eq!(detached.info.branch, "detached HEAD");
        git(owned.root.path(), &["branch", "-f", "comparison", "HEAD"]);
        assert!(manager
            .recheck(RecheckParams {
                snapshot: detached.info.snapshot,
                capture_id: "d".repeat(64)
            })
            .unwrap_err()
            .contains("stale"));
    }
    #[test]
    fn unborn_staged_capture_and_corrupt_persistence_are_bounded_and_fail_closed() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(false);
        std::fs::write(owned.root.path().join("new.txt"), "new\n").unwrap();
        git(owned.root.path(), &["add", "new.txt"]);
        let manager = owned.manager();
        let result = manager.capture(request(Mode::WorkingTree)).unwrap();
        assert!(result.info.head_sha.is_none());
        assert_eq!(result.info.file_count, 1);
        assert!(manager.capture(request(Mode::Branch)).is_err());
        let conn = crate::review_storage::publication_database().unwrap();
        let mut value: serde_json::Value = serde_json::from_str(
            &crate::review_storage::local_evidence::load(&result.info.snapshot)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        value["layers"][0]["patch"] = serde_json::json!("tampered");
        conn.execute(
            "UPDATE local_review_evidence SET metadata_json=?1",
            [value.to_string()],
        )
        .unwrap();
        assert!(manager.get(&result.info.snapshot).is_err());
        manager.stop();
        assert!(manager
            .options(OptionsParams {
                source: Source::Current {},
                offset: 0,
                limit: 32
            })
            .is_err());
    }
    #[test]
    fn capture_registration_rejects_duplicate_and_foreign_cancel_and_shutdown_cancels_native_control(
    ) {
        let manager = Manager::new(PathBuf::from("/owned"));
        let request = request(Mode::WorkingTree);
        let (_guard, cancel) = manager.begin(&request).unwrap();
        assert!(manager.begin(&request).is_err());
        let mut foreign = CancelParams {
            source: Source::Configured {
                repository: RepositoryIdentity {
                    provider: crate::config::ReviewProvider::Github,
                    workspace: "owned".into(),
                    repo: "same".into(),
                },
            },
            capture_id: request.capture_id.clone(),
        };
        assert!(manager.cancel(foreign.clone()).is_err());
        foreign.source = Source::Current {};
        assert!(manager.cancel(foreign).unwrap());
        assert!(GitRunControl::new(TIMEOUT, cancel, "owned")
            .check()
            .is_err());
        let mut other = request;
        other.capture_id = "b".repeat(64);
        let (_guard, other_cancel) = manager.begin(&other).unwrap();
        manager.stop();
        assert!(GitRunControl::new(TIMEOUT, other_cancel, "owned")
            .check()
            .is_err());
    }
    #[test]
    fn native_rename_delete_and_binary_metadata_keep_exact_layer_inventory() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        std::fs::write(owned.root.path().join("delete.txt"), "remove me\n").unwrap();
        git(owned.root.path(), &["add", "delete.txt"]);
        git(
            owned.root.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "owned deletion baseline",
            ],
        );
        git(owned.root.path(), &["mv", "same.txt", "renamed 世界.txt"]);
        git(owned.root.path(), &["rm", "delete.txt"]);
        std::fs::write(owned.root.path().join("binary.png"), b"\0owned binary\0").unwrap();
        git(owned.root.path(), &["add", "binary.png"]);
        let manager = owned.manager();
        let result = manager.capture(request(Mode::WorkingTree)).unwrap();
        let page = manager
            .files(FilesParams {
                snapshot: result.info.snapshot.clone(),
                offset: 0,
                limit: 32,
            })
            .unwrap();
        assert_eq!(page.files.len(), 3);
        assert!(page.files.iter().any(|file| file.status == "renamed"
            && file.old_path.as_deref() == Some("same.txt")
            && file.path == "renamed 世界.txt"));
        assert!(page
            .files
            .iter()
            .any(|file| file.status == "deleted" && file.path == "delete.txt"));
        assert!(page
            .files
            .iter()
            .any(|file| file.binary && file.path == "binary.png" && file.layer == Layer::Staged));
    }
    #[test]
    fn local_patch_above_provider_limit_is_supported_and_reads_are_utf8_bounded() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(false);
        std::fs::write(
            owned.root.path().join("large.txt"),
            format!("{}\n", "世界".repeat(1_500_000)),
        )
        .unwrap();
        git(owned.root.path(), &["add", "large.txt"]);
        let manager = owned.manager();
        let mut request = request(Mode::WorkingTree);
        request.comparison.layers = vec![Layer::Staged];
        let result = manager.capture(request).unwrap();
        assert!(result.info.patch_bytes > 8 * 1024 * 1024);
        assert!(result.info.patch_bytes < PATCH_BYTES);
        let first = manager
            .diff(DiffParams {
                snapshot: result.info.snapshot.clone(),
                layer: Layer::Staged,
                path: "large.txt".into(),
                offset: 0,
            })
            .unwrap();
        assert!(first.text.len() <= 64 * 1024);
        let next = first.next_offset.unwrap();
        let second = manager
            .diff(DiffParams {
                snapshot: result.info.snapshot.clone(),
                layer: Layer::Staged,
                path: "large.txt".into(),
                offset: next,
            })
            .unwrap();
        assert_eq!(second.content_sha256, first.content_sha256);
        assert!(second.text.len() <= 64 * 1024);
        let start = first.text.find('世').unwrap() + 1;
        assert!(manager
            .diff(DiffParams {
                snapshot: result.info.snapshot,
                layer: Layer::Staged,
                path: "large.txt".into(),
                offset: start
            })
            .is_err());
        assert!(
            super::super::super::bitbucket::workspace_prs::snapshot_sections_with_limit(
                &"a".repeat(PATCH_BYTES + 1),
                PATCH_BYTES
            )
            .is_err()
        );
    }
    #[test]
    fn durable_quota_rejection_is_atomic_and_ticket_reservations_release_on_drop() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let manager = Arc::new(owned.manager());
        let first = manager.capture(request(Mode::WorkingTree)).unwrap();
        let mut conn = crate::review_storage::publication_database().unwrap();
        let owner: String = conn
            .query_row(
                "SELECT scope_key FROM local_review_evidence WHERE evidence_id=?1",
                [&first.info.snapshot.snapshot_id],
                |row| row.get(0),
            )
            .unwrap();
        let tx = conn.transaction().unwrap();
        for index in 0..999 {
            tx.execute("INSERT INTO local_review_evidence(evidence_id,scope_key,metadata_json,created_at) VALUES (?1,?2,'{}','1')",rusqlite::params![format!("{index:064x}"),owner]).unwrap();
        }
        tx.commit().unwrap();
        std::fs::write(owned.root.path().join("same.txt"), "new unsaved snapshot\n").unwrap();
        assert!(manager
            .capture(request(Mode::WorkingTree))
            .unwrap_err()
            .contains("full"));
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM ai_review_stores WHERE target_kind='local'",
                [],
                |row| row.get::<_, usize>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(manager.get(&first.info.snapshot).unwrap().info, first.info);
        let params = |letter: &str| CancelParams {
            source: Source::Current {},
            capture_id: letter.repeat(64),
        };
        let ticket = manager.reserve(params("a")).unwrap();
        let second = manager.reserve(params("b")).unwrap();
        assert!(manager.reserve(params("c")).is_err());
        assert!(manager.cancel(params("a")).unwrap());
        assert!(manager
            .capture_reserved(request(Mode::WorkingTree), ticket)
            .is_err());
        drop(second);
        assert!(manager.reserve(params("a")).is_ok());
    }
    #[test]
    fn identical_configured_clones_keep_distinct_handles_and_root_change_is_stale() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let config = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        struct Restore(Option<std::ffi::OsString>);
        impl Drop for Restore {
            fn drop(&mut self) {
                match &self.0 {
                    Some(value) => std::env::set_var("NORN_CONFIG_DIR", value),
                    None => std::env::remove_var("NORN_CONFIG_DIR"),
                }
            }
        }
        let _restore = Restore(std::env::var_os("NORN_CONFIG_DIR"));
        std::env::set_var("NORN_CONFIG_DIR", config.path());
        git(
            owned.root.path(),
            &[
                "clone",
                "--local",
                owned.root.path().to_str().unwrap(),
                other.path().to_str().unwrap(),
            ],
        );
        git(
            other.path(),
            &[
                "remote",
                "set-url",
                "origin",
                "https://github.com/owned/same.git",
            ],
        );
        git(other.path(), &["branch", "--unset-upstream"]);
        let configure = |root: &Path| {
            std::fs::write(config.path().join("settings.json"),serde_json::json!({"repos":[{"provider":"github","workspace":"owned","repo":"same","localPath":root}],"defaultDiffView":"unified","theme":"dark"}).to_string()).unwrap();
        };
        let mut request = request(Mode::WorkingTree);
        request.comparison.source = Source::Configured {
            repository: RepositoryIdentity {
                provider: crate::config::ReviewProvider::Github,
                workspace: "owned".into(),
                repo: "same".into(),
            },
        };
        configure(owned.root.path());
        let manager = owned.manager();
        let first = manager.capture(request.clone()).unwrap();
        configure(other.path());
        let second = manager.capture(request).unwrap();
        assert_eq!(
            first.info.native_snapshot_sha256,
            second.info.native_snapshot_sha256
        );
        assert_ne!(first.info.snapshot.root_id, second.info.snapshot.root_id);
        assert_ne!(
            first.info.snapshot.snapshot_id,
            second.info.snapshot.snapshot_id
        );
        assert_eq!(
            manager
                .info(LookupParams {
                    snapshot: first.info.snapshot.clone()
                })
                .unwrap()
                .info,
            first.info
        );
        assert!(manager
            .recheck(RecheckParams {
                snapshot: first.info.snapshot,
                capture_id: "b".repeat(64)
            })
            .unwrap_err()
            .contains("stale"));
    }
}
