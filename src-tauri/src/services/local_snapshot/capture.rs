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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    images: Option<Vec<super::images::StoredImage>>,
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
    pub(super) images: Option<Vec<super::images::StoredImage>>,
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
impl Ticket {
    pub(crate) fn cancellation(&self) -> LocalReviewCancellation {
        self.cancellation.clone()
    }
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
    let bytes = serde_json::to_vec(&(
        if stored.format_version == 1 {
            "norn-local-snapshot-v1"
        } else {
            "norn-local-snapshot-v2"
        },
        content,
    ))
    .map_err(|_| invalid())?;
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
        if !matches!(
            (self.format_version, self.images.is_some()),
            (1, false) | (2, true)
        ) || info.snapshot != *expected
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
        if let Some(images) = &self.images {
            super::images::validate(
                images,
                &files
                    .iter()
                    .map(|section| &section.file)
                    .collect::<Vec<_>>(),
            )?;
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
            images: self.images,
            bytes,
        })
    }
}
impl Data {
    pub(super) fn browser_files(&self, layer: Layer) -> impl Iterator<Item = &File> {
        self.files
            .iter()
            .filter(move |section| section.file.layer == layer)
            .map(|section| &section.file)
    }
    pub(super) fn browser_file(
        &self,
        layer: Layer,
        path: &str,
        side: crate::services::raster::ImageSide,
    ) -> Option<&File> {
        let mut files = self.browser_files(layer);
        if side == crate::services::raster::ImageSide::Old {
            if let Some(file) = files.find(|file| {
                file.status != "added" && file.old_path.as_deref().unwrap_or(&file.path) == path
            }) {
                return Some(file);
            }
        }
        self.file(layer, path)
    }
    pub(super) fn file(&self, layer: Layer, path: &str) -> Option<&File> {
        self.files
            .iter()
            .find(|section| section.file.layer == layer && section.file.path == path)
            .map(|section| &section.file)
    }
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn layers_for_path(&self, path: &str) -> Vec<Layer> {
        let mut layers = Vec::new();
        for file in &self.files {
            if (file.file.path == path || file.file.old_path.as_deref() == Some(path))
                && !layers.contains(&file.file.layer)
            {
                layers.push(file.file.layer);
            }
        }
        layers
    }
    pub(crate) fn layers(&self) -> impl Iterator<Item = (Layer, &str)> {
        self.layers
            .iter()
            .map(|layer| (layer.kind, layer.patch.as_str()))
    }

    fn stored(&self) -> Stored {
        Stored {
            format_version: if self.images.is_some() { 2 } else { 1 },
            images: self.images.clone(),
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
    pub(super) fn check(&self) -> Result<(), String> {
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
    /// Live policy reads require the same actual source/root/provider, while
    /// saved history/diff reads deliberately remain independent of the checkout.
    pub(crate) fn live_data(&self, snapshot: &SnapshotRef) -> Result<Arc<Data>, String> {
        self.check()?;
        let data = self.get(snapshot)?;
        let control =
            GitRunControl::new(TIMEOUT, self.options_cancel.clone(), "Local review policy");
        let live = resolve_source(&snapshot.source, &self.startup_root, &control)?;
        if live.root != data.root
            || live.root_id != snapshot.root_id
            || live.repository != snapshot.repository
        {
            return Err(
                "The local policy source no longer matches its captured repository.".into(),
            );
        }
        self.check()?;
        Ok(data)
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
        let first_images = super::images::collect(&resolved.root, &first, comparison, &control)?;
        let second = collect()?;
        let second_images = super::images::collect(&resolved.root, &second, comparison, &control)?;
        if first.snapshot_sha256 != second.snapshot_sha256 || first_images != second_images {
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
            // The legacy preview contract skips dirty images. Replace that one
            // obsolete instruction only when this capture actually owns the bytes.
            let captured = first_images.iter().find(|image| {
                image.layer == Layer::Unstaged
                    && image.side == crate::services::raster::ImageSide::New
                    && matches!(image.content, super::images::Content::Available { .. })
                    && warning == format!("Skipped image preview with unstaged content `{}`; stage it to create an immutable preview.", image.path.escape_default())
            });
            let warning = captured.map_or(warning.clone(), |image| {
                format!(
                    "Unstaged image `{}` is preserved in this local capture.",
                    image.path.escape_default()
                )
            });
            if warnings.len() >= 128 || warning.len() > 512 || warning.chars().any(char::is_control)
            {
                truncated = true;
                continue;
            }
            warnings.push(warning);
        }
        let mut stored = Stored {
            format_version: if first_images.is_empty() { 1 } else { 2 },
            images: if first_images.is_empty() {
                None
            } else {
                Some(first_images)
            },
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
            || saved.images.is_some() && current.images != saved.images
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
    fn owned_png(color: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 3, image::Rgba(color));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }
    fn captured_image(
        manager: &Manager,
        snapshot: &SnapshotRef,
        layer: Layer,
        side: crate::services::raster::ImageSide,
    ) -> Vec<u8> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let mut offset = 0;
        let mut sha = None;
        let mut bytes = Vec::new();
        loop {
            let page = manager
                .image(super::super::ImageParams {
                    snapshot: snapshot.clone(),
                    layer,
                    path: "picture.png".into(),
                    side,
                    offset,
                    max_bytes: 7,
                    content_sha256: sha.clone(),
                })
                .unwrap();
            assert!(page.available);
            assert_eq!((page.width, page.height), (Some(2), Some(3)));
            assert_eq!(page.mime_type, "image/png");
            bytes.extend(STANDARD.decode(page.data_base64).unwrap());
            sha = page.content_sha256;
            match page.next_offset {
                Some(next) => {
                    assert!(next > offset);
                    offset = next
                }
                None => {
                    assert_eq!(bytes.len(), page.size);
                    assert_eq!(hash(&bytes), sha.unwrap());
                    return bytes;
                }
            }
        }
    }
    #[test]
    fn captured_rasters_keep_distinct_layer_sides_across_worktree_drift_and_missing_root() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let base = owned_png([255, 0, 0, 255]);
        let staged = owned_png([0, 255, 0, 255]);
        let unstaged = owned_png([0, 0, 255, 255]);
        std::fs::write(owned.root.path().join("picture.png"), &base).unwrap();
        git(owned.root.path(), &["add", "picture.png"]);
        git(
            owned.root.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "owned image base",
            ],
        );
        std::fs::write(owned.root.path().join("picture.png"), &staged).unwrap();
        git(owned.root.path(), &["add", "picture.png"]);
        std::fs::write(owned.root.path().join("picture.png"), &unstaged).unwrap();
        std::fs::write(
            owned.root.path().join("untracked.png"),
            owned_png([255, 255, 0, 255]),
        )
        .unwrap();
        let manager = owned.manager();
        let capture = manager.capture(request(Mode::WorkingTree)).unwrap();
        let reference = &capture.info.snapshot;
        assert!(capture
            .info
            .warnings
            .iter()
            .any(|warning| warning.contains("Unstaged image `picture.png` is preserved")));
        assert!(!capture
            .info
            .warnings
            .iter()
            .any(|warning| warning.contains("stage it to create an immutable preview")));
        use crate::services::raster::ImageSide;
        assert_eq!(
            captured_image(&manager, reference, Layer::Staged, ImageSide::Old),
            base
        );
        assert_eq!(
            captured_image(&manager, reference, Layer::Staged, ImageSide::New),
            staged
        );
        assert_eq!(
            captured_image(&manager, reference, Layer::Unstaged, ImageSide::Old),
            staged
        );
        assert_eq!(
            captured_image(&manager, reference, Layer::Unstaged, ImageSide::New),
            unstaged
        );
        let json = crate::review_storage::local_evidence::load(reference)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap()["formatVersion"],
            2
        );
        let fresh = owned.manager();
        fresh
            .recheck(RecheckParams {
                snapshot: reference.clone(),
                capture_id: "b".repeat(64),
            })
            .unwrap();
        std::fs::write(
            owned.root.path().join("picture.png"),
            owned_png([255, 255, 255, 255]),
        )
        .unwrap();
        assert!(fresh
            .recheck(RecheckParams {
                snapshot: reference.clone(),
                capture_id: "c".repeat(64)
            })
            .is_err());
        assert_eq!(
            captured_image(&fresh, reference, Layer::Unstaged, ImageSide::New),
            unstaged
        );
        let destination = tempfile::tempdir().unwrap();
        std::fs::rename(owned.root.path(), destination.path().join("removed-root")).unwrap();
        let browser_manager = Arc::new(owned.manager());
        for layer in [Layer::Staged, Layer::Unstaged] {
            let state = browser_manager
                .browser_state(&super::super::BrowserParams {
                    snapshot: reference.clone(),
                    layer,
                })
                .unwrap();
            assert_eq!(
                state.target_kind,
                crate::browser_diff::WebDiffTargetKind::Local
            );
            assert_eq!(state.pr_id, 0);
            assert_eq!(
                state.local_snapshot_sha256.as_deref(),
                Some(reference.snapshot_id.as_str())
            );
            assert_eq!(
                state.diff.as_deref(),
                browser_manager
                    .get(reference)
                    .unwrap()
                    .layers()
                    .find(|(kind, _)| *kind == layer)
                    .map(|(_, text)| text)
            );
            assert!(state.snapshot_source.is_some());
            let public = serde_json::to_string(&state).unwrap();
            assert!(!public.contains("snapshot_source"));
            assert!(!public.contains(destination.path().to_str().unwrap()));
        }
        let mut foreign = reference.clone();
        foreign.root_id = "d".repeat(64);
        assert!(browser_manager
            .browser_state(&super::super::BrowserParams {
                snapshot: foreign,
                layer: Layer::Staged,
            })
            .is_err());
        assert_eq!(
            captured_image(&owned.manager(), reference, Layer::Staged, ImageSide::New),
            staged
        );
        assert!(owned
            .manager()
            .image(super::super::ImageParams {
                snapshot: reference.clone(),
                layer: Layer::Unstaged,
                path: "untracked.png".into(),
                side: ImageSide::New,
                offset: 0,
                max_bytes: 7,
                content_sha256: None
            })
            .is_err());
    }

    #[test]
    fn saved_snapshot_pages_verify_catalog_continuity_and_survive_missing_checkout() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let manager = owned.manager();
        let first = manager.capture(request(Mode::WorkingTree)).unwrap();
        let initial = manager
            .list(super::super::ListParams {
                offset: 0,
                limit: 1,
                catalog_version: None,
            })
            .unwrap();
        assert_eq!(initial.total, 1);
        assert_eq!(initial.snapshots[0].snapshot, first.info.snapshot);
        std::fs::write(owned.root.path().join("same.txt"), "another capture\n").unwrap();
        let second = manager.capture(request(Mode::WorkingTree)).unwrap();
        assert_ne!(first.info.snapshot, second.info.snapshot);
        assert!(manager
            .list(super::super::ListParams {
                offset: 1,
                limit: 1,
                catalog_version: Some(initial.catalog_version)
            })
            .is_err());
        let new_page = manager
            .list(super::super::ListParams {
                offset: 0,
                limit: 1,
                catalog_version: None,
            })
            .unwrap();
        assert_eq!(new_page.total, 2);
        assert_eq!(new_page.next_offset, Some(1));
        let removed = tempfile::tempdir().unwrap();
        std::fs::rename(owned.root.path(), removed.path().join("removed-checkout")).unwrap();
        let restored = owned
            .manager()
            .list(super::super::ListParams {
                offset: 1,
                limit: 1,
                catalog_version: Some(new_page.catalog_version.clone()),
            })
            .unwrap();
        assert_eq!(restored.next_offset, None);
        assert_eq!(restored.catalog_version, new_page.catalog_version);
        assert_ne!(
            restored.snapshots[0].snapshot,
            new_page.snapshots[0].snapshot
        );
        assert!(owned
            .manager()
            .list(super::super::ListParams {
                offset: 1,
                limit: 1,
                catalog_version: None
            })
            .is_err());
        let encoded = serde_json::to_string(&restored).unwrap();
        assert!(!encoded.contains(removed.path().to_str().unwrap()));
        assert!(!encoded.contains("localReviewId"));
    }

    #[test]
    fn committed_rasters_preserve_rename_sources_deleted_sides_and_ignore_dirty_bytes() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let old = owned_png([255, 0, 0, 255]);
        let new = owned_png([0, 0, 255, 255]);
        let deleted = owned_png([0, 255, 0, 255]);
        for path in ["picture.png", "changed.png"] {
            std::fs::write(owned.root.path().join(path), &old).unwrap();
        }
        std::fs::write(owned.root.path().join("deleted.png"), &deleted).unwrap();
        git(
            owned.root.path(),
            &["add", "picture.png", "deleted.png", "changed.png"],
        );
        git(
            owned.root.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "owned image base",
            ],
        );
        git(owned.root.path(), &["branch", "-f", "comparison", "HEAD"]);
        git(owned.root.path(), &["mv", "picture.png", "moved.png"]);
        git(owned.root.path(), &["rm", "deleted.png"]);
        std::fs::write(owned.root.path().join("changed.png"), &new).unwrap();
        git(owned.root.path(), &["add", "changed.png"]);
        git(
            owned.root.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "owned image change",
            ],
        );
        std::fs::write(owned.root.path().join("moved.png"), &new).unwrap();
        let manager = Arc::new(owned.manager());
        let capture = manager.capture(request(Mode::Branch)).unwrap();
        let state = manager
            .browser_state(&super::super::BrowserParams {
                snapshot: capture.info.snapshot.clone(),
                layer: Layer::Committed,
            })
            .unwrap();
        let crate::browser_diff::WebDiffSnapshotSource::Local(source) =
            state.snapshot_source.unwrap()
        else {
            panic!("local raster authority required")
        };
        assert!(source.raster("picture.png", "old").unwrap().unwrap().bytes == old);
        assert!(source.raster("moved.png", "new").unwrap().unwrap().bytes == old);
        assert!(source.raster("changed.png", "old").unwrap().unwrap().bytes == old);
        assert!(source.raster("changed.png", "new").unwrap().unwrap().bytes == new);
        assert!(source.raster("deleted.png", "old").unwrap().unwrap().bytes == deleted);
        assert!(source.raster("deleted.png", "new").unwrap().is_none());
        assert!(source.raster("unknown.png", "new").is_err());
    }

    #[test]
    fn raster_capture_count_limit_is_explicit_and_does_not_hide_known_absent_sides() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let png = owned_png([1, 2, 3, 255]);
        for index in 0..65 {
            std::fs::write(
                owned.root.path().join(format!("image-{index:03}.png")),
                &png,
            )
            .unwrap();
        }
        git(owned.root.path(), &["add", "--", "*.png"]);
        let manager = owned.manager();
        let capture = manager.capture(request(Mode::WorkingTree)).unwrap();
        assert_eq!(capture.info.file_count, 65);
        let data = manager.get(&capture.info.snapshot).unwrap();
        assert_eq!(data.images.as_ref().unwrap().len(), 128);
        for (side, reason) in [
            (crate::services::raster::ImageSide::Old, "absent"),
            (crate::services::raster::ImageSide::New, "captureLimit"),
        ] {
            let page = manager
                .image(super::super::ImageParams {
                    snapshot: capture.info.snapshot.clone(),
                    layer: Layer::Staged,
                    path: "image-064.png".into(),
                    side,
                    offset: 0,
                    max_bytes: 32,
                    content_sha256: None,
                })
                .unwrap();
            assert!(!page.available);
            assert_eq!(page.reason.as_deref(), Some(reason));
            assert!(page.data_base64.is_empty());
        }
    }

    #[test]
    fn raster_capture_preserves_explicit_unsupported_and_oversized_states() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        std::fs::write(owned.root.path().join("invalid.png"), b"not a raster").unwrap();
        std::fs::write(owned.root.path().join("vector.svg"), b"<svg></svg>").unwrap();
        let mut huge = owned_png([1, 2, 3, 255]);
        huge.resize(2 * 1024 * 1024 + 1, 0);
        std::fs::write(owned.root.path().join("huge.png"), huge).unwrap();
        git(
            owned.root.path(),
            &["add", "invalid.png", "vector.svg", "huge.png"],
        );
        let manager = owned.manager();
        let capture = manager.capture(request(Mode::WorkingTree)).unwrap();
        for (path, reason) in [
            ("invalid.png", "invalidContent"),
            ("vector.svg", "unsupportedFormat"),
            ("huge.png", "oversized"),
        ] {
            let page = manager
                .image(super::super::ImageParams {
                    snapshot: capture.info.snapshot.clone(),
                    layer: Layer::Staged,
                    path: path.into(),
                    side: crate::services::raster::ImageSide::New,
                    offset: 0,
                    max_bytes: 32,
                    content_sha256: None,
                })
                .unwrap();
            assert!(!page.available);
            assert_eq!(page.reason.as_deref(), Some(reason));
            assert!(page.data_base64.is_empty());
            assert_eq!(page.content_sha256, None);
            assert!(manager
                .image(super::super::ImageParams {
                    snapshot: capture.info.snapshot.clone(),
                    layer: Layer::Staged,
                    path: path.into(),
                    side: crate::services::raster::ImageSide::New,
                    offset: 1,
                    max_bytes: 32,
                    content_sha256: Some("f".repeat(64)),
                })
                .is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn raster_capture_rejects_an_unsupported_symlink_type_change() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let png = owned_png([1, 2, 3, 255]);
        let path = owned.root.path().join("picture.png");
        std::fs::write(&path, &png).unwrap();
        git(owned.root.path(), &["add", "picture.png"]);
        git(
            owned.root.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "owned regular raster",
            ],
        );
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("owned-external.png");
        std::fs::write(&outside_path, owned_png([99, 88, 77, 255])).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&outside_path, &path).unwrap();
        let manager = owned.manager();
        assert!(manager.capture(request(Mode::WorkingTree)).is_err());
        assert!(crate::review_storage::local_evidence::catalogue()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn raster_capture_validates_metadata_and_legacy_format_without_mutable_fallback() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = Owned::new(true);
        let png = owned_png([1, 2, 3, 255]);
        std::fs::write(owned.root.path().join("picture.png"), &png).unwrap();
        git(owned.root.path(), &["add", "picture.png"]);
        let manager = owned.manager();
        let capture = manager.capture(request(Mode::WorkingTree)).unwrap();
        let data = manager.get(&capture.info.snapshot).unwrap();
        let mut stored = data.stored();
        let available = stored
            .images
            .as_mut()
            .unwrap()
            .iter_mut()
            .find(|image| {
                matches!(
                    image.content,
                    super::super::images::Content::Available { .. }
                )
            })
            .unwrap();
        if let super::super::images::Content::Available { width, .. } = &mut available.content {
            *width = 3;
        }
        stored.info.snapshot.snapshot_id = identity(&stored).unwrap();
        stored.info.local_review_id = review_id(&stored.info.snapshot.snapshot_id).unwrap();
        let expected = stored.info.snapshot.clone();
        assert!(stored.validate(&expected).is_err());
        let mut legacy = data.stored();
        legacy.format_version = 1;
        legacy.images = None;
        legacy.info.snapshot.snapshot_id = identity(&legacy).unwrap();
        legacy.info.local_review_id = review_id(&legacy.info.snapshot.snapshot_id).unwrap();
        let json = serde_json::to_string(&legacy).unwrap();
        assert!(!json.contains("\"images\""));
        crate::review_storage::local_evidence::save(&legacy.info, &json).unwrap();
        use crate::services::raster::ImageSide;
        let page = owned
            .manager()
            .image(super::super::ImageParams {
                snapshot: legacy.info.snapshot.clone(),
                layer: Layer::Staged,
                path: "picture.png".into(),
                side: ImageSide::New,
                offset: 0,
                max_bytes: 32,
                content_sha256: None,
            })
            .unwrap();
        assert!(!page.available);
        assert_eq!(page.reason.as_deref(), Some("legacySnapshot"));
        assert!(page.data_base64.is_empty());
        let wrong = super::super::ImageParams {
            snapshot: capture.info.snapshot.clone(),
            layer: Layer::Staged,
            path: "picture.png".into(),
            side: ImageSide::New,
            offset: 1,
            max_bytes: 32,
            content_sha256: Some("a".repeat(64)),
        };
        assert!(manager.image(wrong).is_err());
        let absent = manager
            .image(super::super::ImageParams {
                snapshot: capture.info.snapshot,
                layer: Layer::Staged,
                path: "picture.png".into(),
                side: ImageSide::Old,
                offset: 0,
                max_bytes: 32,
                content_sha256: None,
            })
            .unwrap();
        assert!(!absent.available);
        assert_eq!(absent.reason.as_deref(), Some("absent"));
        assert!(absent.mime_type.is_empty());
    }
}
