//! Bounded immutable raster cache shared by native terminal and browser previews.
pub use super::super::raster::ImageSide;
pub(crate) use super::super::raster::Raster;
use super::super::raster::{raster_format, validate_raster, MAX_IMAGE_BYTES as MAX_IMAGE};
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::atomic::{AtomicUsize, Ordering};
const MAX_CHUNK: usize = 192 * 1024;
const IMAGE_CACHE_BYTES: usize = 32 * 1024 * 1024;
const IMAGE_CACHE_COUNT: usize = 32;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageParams {
    pub snapshot: SnapshotRef,
    pub path: String,
    pub side: ImageSide,
    pub offset: usize,
    pub max_bytes: usize,
    #[serde(deserialize_with = "required_nullable_hash")]
    pub content_sha256: Option<String>,
}
fn required_nullable_hash<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
impl ImageParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if !safe_path(&self.path)
            || self.offset > MAX_IMAGE
            || !(1..=MAX_CHUNK).contains(&self.max_bytes)
            || self.content_sha256.as_ref().is_some_and(|sha| {
                sha.len() != 64
                    || !sha
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        {
            return Err(invalid());
        }
        if self.offset > 0 && self.content_sha256.is_none() {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImagePage {
    pub request: ImageParams,
    pub available: bool,
    pub mime_type: String,
    pub size: usize,
    pub data_base64: String,
    pub next_offset: Option<usize>,
    pub content_sha256: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
#[derive(Clone, PartialEq, Eq)]
struct ImageKey {
    snapshot: SnapshotRef,
    path: String,
    side: ImageSide,
}
struct CachedRaster {
    key: ImageKey,
    raster: Arc<Raster>,
    created: Instant,
}
pub(super) struct RasterCache {
    entries: Mutex<VecDeque<CachedRaster>>,
    maximum: usize,
    count: usize,
}
impl Default for RasterCache {
    fn default() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            maximum: IMAGE_CACHE_BYTES,
            count: IMAGE_CACHE_COUNT,
        }
    }
}
struct RemoteGuard(Arc<AtomicUsize>);
impl RemoteGuard {
    fn acquire(active: &Arc<AtomicUsize>) -> Result<Self, String> {
        active
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| {
                "Two provider reads are already in progress; wait before loading another image."
                    .to_string()
            })?;
        Ok(Self(active.clone()))
    }
}
impl Drop for RemoteGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
pub(crate) struct ImageContext {
    pub repository: RepositoryIdentity,
    pub revision: String,
    pub path: String,
}
impl Manager {
    pub(crate) fn with_runtime(
        remote_gate: Arc<AtomicUsize>,
        shutdown: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            remote_gate,
            shutdown,
            ..Self::default()
        }
    }
    fn image_context(&self, request: &ImageParams) -> Result<Option<ImageContext>, String> {
        request.validate()?;
        if self.shutdown.load(Ordering::SeqCst) {
            return Err("Image preview canceled during shutdown.".into());
        }
        let data = self.get(&request.snapshot)?;
        let file = &data
            .files
            .iter()
            .find(|entry| entry.file.path == request.path)
            .ok_or_else(invalid)?
            .file;
        if (request.side == ImageSide::Old && file.status == "added")
            || (request.side == ImageSide::New && file.status == "deleted")
        {
            if request.offset != 0 || request.content_sha256.is_some() {
                return Err(invalid());
            }
            return Ok(None);
        }
        let path = if request.side == ImageSide::Old {
            file.old_path.as_deref().unwrap_or(&file.path)
        } else {
            &file.path
        };
        raster_format(path)?;
        let (repository, revision) = if request.side == ImageSide::Old {
            (&data.info.snapshot.repository, &data.info.base_sha)
        } else {
            (
                data.source_repository
                    .as_ref()
                    .unwrap_or(&data.info.snapshot.repository),
                &data.info.head_sha,
            )
        };
        Ok(Some(ImageContext {
            repository: repository.clone(),
            revision: revision.clone(),
            path: path.into(),
        }))
    }
    fn raster_with(
        &self,
        request: &ImageParams,
        fetch: impl FnOnce(&ImageContext) -> Result<Vec<u8>, String>,
    ) -> Result<Option<Arc<Raster>>, String> {
        let Some(context) = self.image_context(request)? else {
            return Ok(None);
        };
        let key = ImageKey {
            snapshot: request.snapshot.clone(),
            path: request.path.clone(),
            side: request.side,
        };
        {
            let mut entries = self.images.entries.lock().map_err(|_| invalid())?;
            entries.retain(|entry| entry.created.elapsed() < self.ttl);
            if let Some(index) = entries.iter().position(|entry| entry.key == key) {
                let entry = entries.remove(index).ok_or_else(invalid)?;
                let raster = entry.raster.clone();
                entries.push_back(entry);
                if request
                    .content_sha256
                    .as_ref()
                    .is_some_and(|sha| sha != &raster.sha256)
                {
                    return Err(invalid());
                }
                return Ok(Some(raster));
            }
        }
        if request.offset != 0 || request.content_sha256.is_some() {
            return Err("Immutable image transfer is expired or unavailable. Reload its preview explicitly.".into());
        }
        let _read = RemoteGuard::acquire(&self.remote_gate)?;
        let raster = Arc::new(validate_raster(&context.path, fetch(&context)?)?);
        // A snapshot can expire or be evicted while its bounded HTTP read runs.
        self.get(&request.snapshot)?;
        if self.shutdown.load(Ordering::SeqCst) {
            return Err("Image preview canceled during shutdown.".into());
        }
        let mut entries = self.images.entries.lock().map_err(|_| invalid())?;
        entries.retain(|entry| entry.created.elapsed() < self.ttl && entry.key != key);
        if raster.bytes.len() > self.images.maximum || self.images.count == 0 {
            return Err(invalid());
        }
        while entries.len() >= self.images.count
            || entries
                .iter()
                .map(|entry| entry.raster.bytes.len())
                .sum::<usize>()
                + raster.bytes.len()
                > self.images.maximum
        {
            entries.pop_front();
        }
        entries.push_back(CachedRaster {
            key,
            raster: raster.clone(),
            created: Instant::now(),
        });
        Ok(Some(raster))
    }
    #[cfg(test)]
    pub(crate) fn cache_owned_image(
        &self,
        request: &ImageParams,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        self.raster_with(request, |_| Ok(bytes)).map(|_| ())
    }
    pub(crate) fn raster(&self, request: &ImageParams) -> Result<Option<Arc<Raster>>, String> {
        self.raster_with(request, |context| {
            super::super::bitbucket::workspace_prs::Api::from_stored(
                request.snapshot.repository.clone(),
            )?
            .immutable_image(&context.repository, &context.revision, &context.path)
        })
    }
    pub(crate) fn image(&self, request: ImageParams) -> Result<ImagePage, String> {
        let raster = self.raster(&request)?;
        image_page(request, raster)
    }
}
fn image_page(request: ImageParams, raster: Option<Arc<Raster>>) -> Result<ImagePage, String> {
    let Some(raster) = raster else {
        return Ok(ImagePage {
            request,
            available: false,
            mime_type: String::new(),
            size: 0,
            data_base64: String::new(),
            next_offset: None,
            content_sha256: None,
            width: None,
            height: None,
        });
    };
    if request.offset > raster.bytes.len() {
        return Err(invalid());
    }
    let end = (request.offset + request.max_bytes).min(raster.bytes.len());
    Ok(ImagePage {
        available: true,
        mime_type: raster.mime.into(),
        size: raster.bytes.len(),
        data_base64: STANDARD.encode(&raster.bytes[request.offset..end]),
        next_offset: (end < raster.bytes.len()).then_some(end),
        content_sha256: Some(raster.sha256.clone()),
        width: Some(raster.width),
        height: Some(raster.height),
        request,
    })
}
#[derive(Clone)]
pub struct BrowserSnapshotSource {
    snapshot: SnapshotRef,
    manager: Arc<Manager>,
}
impl std::fmt::Debug for BrowserSnapshotSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSnapshotSource")
            .field("snapshot", &self.snapshot)
            .finish_non_exhaustive()
    }
}
impl PartialEq for BrowserSnapshotSource {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot && Arc::ptr_eq(&self.manager, &other.manager)
    }
}
impl Eq for BrowserSnapshotSource {}
impl BrowserSnapshotSource {
    pub(crate) fn new(snapshot: SnapshotRef, manager: Arc<Manager>) -> Self {
        Self { snapshot, manager }
    }
    pub(crate) fn raster(&self, path: &str, side: &str) -> Result<Option<Arc<Raster>>, String> {
        let side = match side {
            "old" => ImageSide::Old,
            "new" => ImageSide::New,
            _ => return Err(invalid()),
        };
        let data = self.manager.get(&self.snapshot)?;
        let file = if side == ImageSide::Old {
            data.files
                .iter()
                .find(|entry| {
                    entry.file.status != "added"
                        && entry.file.old_path.as_deref().unwrap_or(&entry.file.path) == path
                })
                .or_else(|| data.files.iter().find(|entry| entry.file.path == path))
        } else {
            data.files.iter().find(|entry| entry.file.path == path)
        }
        .ok_or_else(invalid)?;
        self.manager.raster(&ImageParams {
            snapshot: self.snapshot.clone(),
            path: file.file.path.clone(),
            side,
            offset: 0,
            max_bytes: MAX_CHUNK,
            content_sha256: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReviewProvider;

    fn capture(provider: ReviewProvider, status: &str) -> Capture {
        let patch = match status {
            "added" => "diff --git a/picture.png b/picture.png\nnew file mode 100644\nBinary files /dev/null and b/picture.png differ\n",
            "deleted" => "diff --git a/picture.png b/picture.png\ndeleted file mode 100644\nBinary files a/picture.png and /dev/null differ\n",
            "renamed" => "diff --git a/old.png b/picture.png\nsimilarity index 100%\nrename from old.png\nrename to picture.png\n",
            _ => "diff --git a/picture.png b/picture.png\nBinary files a/picture.png and b/picture.png differ\n",
        };
        Capture {
            request: DetailParams {
                repository: RepositoryIdentity {
                    provider,
                    workspace: "Owner".into(),
                    repo: "Repo".into(),
                },
                pr_id: 7,
            },
            title: "Owned image".into(),
            source_branch: "feature".into(),
            destination_branch: "main".into(),
            head_sha: "a".repeat(40),
            destination_sha: "b".repeat(40),
            base_sha: "c".repeat(40),
            patch: patch.into(),
            expected_count: 1,
            expected_files: None,
            source_repository: Some(RepositoryIdentity {
                provider,
                workspace: "Fork".into(),
                repo: "Source".into(),
            }),
        }
    }
    fn params(snapshot: SnapshotRef, side: ImageSide) -> ImageParams {
        ImageParams {
            snapshot,
            path: "picture.png".into(),
            side,
            offset: 0,
            max_bytes: 16,
            content_sha256: None,
        }
    }
    fn png() -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 3)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }
    #[test]
    fn image_pins_rename_fork_and_missing_sides_are_native_for_both_providers() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            for status in ["added", "deleted", "renamed", "modified"] {
                let manager = Manager::default();
                let info = manager.insert(capture(provider, status)).unwrap();
                for side in [ImageSide::Old, ImageSide::New] {
                    let request = params(info.snapshot.clone(), side);
                    let mut fetched = false;
                    let raster = manager
                        .raster_with(&request, |context| {
                            fetched = true;
                            assert_eq!(context.repository.provider, provider);
                            assert_eq!(
                                context.repository.workspace,
                                if side == ImageSide::Old {
                                    "Owner"
                                } else {
                                    "Fork"
                                }
                            );
                            assert_eq!(
                                context.revision,
                                if side == ImageSide::Old {
                                    "c".repeat(40)
                                } else {
                                    "a".repeat(40)
                                }
                            );
                            assert_eq!(
                                context.path,
                                if status == "renamed" && side == ImageSide::Old {
                                    "old.png"
                                } else {
                                    "picture.png"
                                }
                            );
                            Ok(png())
                        })
                        .unwrap();
                    let absent = status == "added" && side == ImageSide::Old
                        || status == "deleted" && side == ImageSide::New;
                    assert_eq!(fetched, !absent);
                    let page = image_page(request, raster).unwrap();
                    assert_eq!(page.available, !absent);
                    if absent {
                        assert_eq!(page.size, 0);
                        assert!(page.content_sha256.is_none());
                    } else {
                        assert_eq!(page.width, Some(2));
                        assert_eq!(page.height, Some(3));
                        assert_eq!(page.next_offset, Some(16));
                    }
                }
            }
        }
    }
    #[test]
    fn image_scope_path_limits_hash_shutdown_and_restart_fail_before_fetch() {
        let manager = Manager::default();
        let info = manager
            .insert(capture(ReviewProvider::Github, "modified"))
            .unwrap();
        let good = params(info.snapshot, ImageSide::New);
        for fault in 0..10 {
            let mut request = good.clone();
            match fault {
                0 => request.snapshot.repository.provider = ReviewProvider::Bitbucket,
                1 => request.snapshot.pr_id += 1,
                2 => request.snapshot.snapshot_id = "f".repeat(64),
                3 => request.path = "../picture.png".into(),
                4 => request.path = "other.png".into(),
                5 => request.max_bytes = MAX_CHUNK + 1,
                6 => request.offset = MAX_IMAGE + 1,
                7 => request.offset = 1,
                8 => request.content_sha256 = Some("f".repeat(64)),
                _ => request.content_sha256 = Some("F".repeat(64)),
            }
            assert!(manager
                .raster_with(&request, |_| panic!(
                    "invalid input fetched credentials or bytes"
                ))
                .is_err());
        }
        assert!(Manager::default()
            .raster_with(&good, |_| panic!("restart fetched"))
            .is_err());
        manager.stop();
        assert!(manager
            .raster_with(&good, |_| panic!("shutdown fetched"))
            .is_err());
    }
    #[test]
    fn chunks_reassemble_cached_bytes_without_refetch_and_detect_eviction() {
        let manager = Manager {
            images: RasterCache {
                count: 1,
                ..RasterCache::default()
            },
            ..Manager::default()
        };
        let info = manager
            .insert(capture(ReviewProvider::Github, "modified"))
            .unwrap();
        let mut request = params(info.snapshot.clone(), ImageSide::New);
        let expected = png();
        let raster = manager
            .raster_with(&request, |_| Ok(expected.clone()))
            .unwrap();
        let mut page = image_page(request.clone(), raster).unwrap();
        let mut combined = STANDARD.decode(&page.data_base64).unwrap();
        while let Some(offset) = page.next_offset {
            request.offset = offset;
            request.content_sha256 = page.content_sha256.clone();
            page = image_page(
                request.clone(),
                manager
                    .raster_with(&request, |_| panic!("continuation refetched"))
                    .unwrap(),
            )
            .unwrap();
            combined.extend(STANDARD.decode(&page.data_base64).unwrap());
        }
        assert_eq!(combined, expected);
        request.content_sha256 = Some("f".repeat(64));
        assert!(manager
            .raster_with(&request, |_| panic!("hash mismatch fetched"))
            .is_err());
        let old = params(info.snapshot, ImageSide::Old);
        manager.raster_with(&old, |_| Ok(png())).unwrap();
        request.content_sha256 = Some(hash(&expected));
        assert!(manager
            .raster_with(&request, |_| panic!("evicted continuation fetched"))
            .is_err());
        let entries = manager.images.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(
            entries
                .iter()
                .map(|entry| entry.raster.bytes.len())
                .sum::<usize>()
                <= IMAGE_CACHE_BYTES
        );
    }
    #[test]
    fn image_reads_share_provider_gate_and_recheck_snapshot_after_http() {
        let active = Arc::new(AtomicUsize::new(2));
        let manager = Manager::with_runtime(
            active.clone(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        let info = manager
            .insert(capture(ReviewProvider::Github, "modified"))
            .unwrap();
        let request = params(info.snapshot, ImageSide::New);
        assert!(manager
            .raster_with(&request, |_| panic!("shared provider bound bypassed"))
            .is_err());
        active.store(1, Ordering::SeqCst);
        manager
            .raster_with(&request, |_| {
                assert_eq!(active.load(Ordering::SeqCst), 2);
                Ok(png())
            })
            .unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 1);
        active.store(2, Ordering::SeqCst);
        assert!(manager
            .raster_with(&request, |_| panic!("cached raster fetched"))
            .is_ok());
        let fresh = params(
            manager
                .insert(capture(ReviewProvider::Github, "modified"))
                .unwrap()
                .snapshot,
            ImageSide::New,
        );
        active.store(0, Ordering::SeqCst);
        assert!(manager
            .raster_with(&fresh, |_| {
                manager.entries.lock().unwrap().clear();
                Ok(png())
            })
            .is_err());
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(!manager
            .images
            .entries
            .lock()
            .unwrap()
            .iter()
            .any(|entry| entry.key.snapshot == fresh.snapshot));
    }
    #[test]
    fn expired_snapshot_and_raster_entries_cannot_resume_a_transfer() {
        let manager = Manager::default();
        let info = manager
            .insert(capture(ReviewProvider::Github, "modified"))
            .unwrap();
        let mut request = params(info.snapshot, ImageSide::New);
        manager.raster_with(&request, |_| Ok(png())).unwrap();
        request.offset = 16;
        request.content_sha256 = Some(hash(&png()));
        manager.images.entries.lock().unwrap()[0].created = Instant::now() - TTL;
        assert!(manager
            .raster_with(&request, |_| panic!("expired continuation refetched"))
            .is_err());
        request.offset = 0;
        request.content_sha256 = None;
        manager.raster_with(&request, |_| Ok(png())).unwrap();
        Arc::get_mut(&mut manager.entries.lock().unwrap()[0])
            .unwrap()
            .created = Instant::now() - TTL;
        assert!(manager
            .raster_with(&request, |_| panic!("expired snapshot fetched"))
            .is_err());
    }
    #[test]
    fn unsupported_mismatched_empty_oversized_and_extreme_rasters_are_rejected() {
        assert!(validate_raster("picture.svg", png()).is_err());
        assert!(validate_raster("picture.jpg", png()).is_err());
        assert!(validate_raster("picture.png", Vec::new()).is_err());
        assert!(validate_raster("picture.png", vec![0; MAX_IMAGE + 1]).is_err());
        let mut large = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(8193, 1)
            .write_to(&mut large, image::ImageFormat::Png)
            .unwrap();
        assert!(validate_raster("picture.png", large.into_inner()).is_err());
    }
}
