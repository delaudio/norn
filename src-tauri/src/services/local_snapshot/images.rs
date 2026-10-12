//! Durable layer-specific raster bytes. Saved reads never resolve a live file.
use super::*;
use crate::services::raster::{raster_format, validate_raster, ImageSide, Raster, MAX_IMAGE_BYTES};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{io::Read, sync::Arc};
const IMAGE_COUNT: usize = 128;
const IMAGE_BYTES: usize = 8 * 1024 * 1024;
const IMAGE_CHUNK: usize = 192 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Unavailable {
    UnsupportedFormat,
    UnsupportedMode,
    Oversized,
    InvalidContent,
    CaptureLimit,
    LegacySnapshot,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Content {
    Available {
        #[serde(rename = "dataBase64")]
        data_base64: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
        width: u32,
        height: u32,
        #[serde(rename = "contentSha256")]
        content_sha256: String,
    },
    Absent {},
    Unavailable {
        reason: Unavailable,
    },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StoredImage {
    pub layer: Layer,
    pub path: String,
    pub side: ImageSide,
    pub content_path: String,
    pub content: Content,
}
impl StoredImage {
    pub(crate) fn raster(&self) -> Result<Option<Arc<Raster>>, String> {
        match &self.content {
            Content::Absent {} => Ok(None),
            Content::Unavailable { .. } => {
                Err("This image is unavailable in the saved capture.".into())
            }
            Content::Available {
                data_base64,
                mime_type,
                width,
                height,
                content_sha256,
            } => {
                if data_base64.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
                    return Err(invalid());
                }
                let bytes = STANDARD.decode(data_base64).map_err(|_| invalid())?;
                if STANDARD.encode(&bytes) != *data_base64 {
                    return Err(invalid());
                }
                let raster = validate_raster(&self.content_path, bytes)?;
                if raster.mime != mime_type
                    || raster.width != *width
                    || raster.height != *height
                    || raster.sha256 != *content_sha256
                {
                    return Err(invalid());
                }
                Ok(Some(Arc::new(raster)))
            }
        }
    }
}
pub(crate) fn image_path(path: &str) -> bool {
    matches!(
        path.rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg"
    )
}
fn git(
    root: &Path,
    args: &[&str],
    maximum: usize,
    control: &GitRunControl,
) -> Result<Vec<u8>, String> {
    let result = crate::local_review::run_git_bounded_with_control(
        root,
        args,
        None,
        maximum,
        "Captured local image",
        control,
    )
    .map_err(|_| invalid())?;
    if !result.status.success() {
        return Err(invalid());
    }
    Ok(result.stdout)
}
fn blob(
    root: &Path,
    path: &str,
    revision: Option<&str>,
    control: &GitRunControl,
) -> Result<Result<Vec<u8>, Unavailable>, String> {
    let raw = match revision {
        Some(revision) => git(
            root,
            &["--literal-pathspecs", "ls-tree", "-z", revision, "--", path],
            GIT_METADATA_BYTES,
            control,
        )?,
        None => git(
            root,
            &[
                "--literal-pathspecs",
                "ls-files",
                "--stage",
                "-z",
                "--",
                path,
            ],
            GIT_METADATA_BYTES,
            control,
        )?,
    };
    let mut entries = raw.split(|b| *b == 0).filter(|entry| !entry.is_empty());
    let entry = entries.next().ok_or_else(invalid)?;
    if entries.next().is_some() {
        return Ok(Err(Unavailable::UnsupportedMode));
    }
    let entry = std::str::from_utf8(entry).map_err(|_| invalid())?;
    let (meta, actual_path) = entry.split_once('\t').ok_or_else(invalid)?;
    if actual_path != path {
        return Err(invalid());
    }
    let fields: Vec<_> = meta.split_whitespace().collect();
    if fields.len() != 3 {
        return Err(invalid());
    }
    let (mode, object) = if revision.is_some() {
        if fields[1] != "blob" {
            return Ok(Err(Unavailable::UnsupportedMode));
        }
        (fields[0], fields[2])
    } else {
        if fields[2] != "0" {
            return Ok(Err(Unavailable::UnsupportedMode));
        }
        (fields[0], fields[1])
    };
    if !matches!(mode, "100644" | "100755") {
        return Ok(Err(Unavailable::UnsupportedMode));
    }
    super::super::provider_snapshot::full_sha(object)?;
    let size = git(root, &["cat-file", "-s", object], 256, control)?;
    let size: usize = std::str::from_utf8(&size)
        .map_err(|_| invalid())?
        .trim()
        .parse()
        .map_err(|_| invalid())?;
    if size > MAX_IMAGE_BYTES {
        return Ok(Err(Unavailable::Oversized));
    }
    Ok(Ok(git(
        root,
        &["cat-file", "blob", object],
        MAX_IMAGE_BYTES,
        control,
    )?))
}
#[cfg(unix)]
fn worktree_file(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Option<std::fs::File>, String> {
    use std::{
        ffi::CString,
        os::fd::{AsRawFd, FromRawFd},
        os::unix::fs::OpenOptionsExt,
    };
    control.check()?;
    let mut directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(root)
        .map_err(|_| invalid())?;
    let parts: Vec<_> = path.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        control.check()?;
        let part = CString::new(*part).map_err(|_| invalid())?;
        let final_component = index + 1 == parts.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if final_component {
                0
            } else {
                libc::O_DIRECTORY
            };
        let fd = unsafe { libc::openat(directory.as_raw_fd(), part.as_ptr(), flags) };
        if fd < 0 {
            return Ok(None);
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if final_component {
            return Ok(Some(file));
        }
        directory = file;
    }
    Err(invalid())
}
#[cfg(not(unix))]
fn worktree_file(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Option<std::fs::File>, String> {
    let mut candidate = root.to_path_buf();
    for part in path.split('/') {
        control.check()?;
        candidate.push(part);
        let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
            return Ok(None);
        };
        if metadata.file_type().is_symlink() {
            return Ok(None);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Ok(None);
            }
        }
    }
    if !candidate
        .canonicalize()
        .map_err(|_| invalid())?
        .starts_with(root)
    {
        return Ok(None);
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    let Some(file) = options.open(&candidate).ok() else {
        return Ok(None);
    };
    #[cfg(windows)]
    {
        // Check the opened handle, not only the path inspected before open.
        // An ancestor can be replaced by a junction between those operations.
        use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
        let mut path_buffer = vec![0u16; 32768];
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as _,
                path_buffer.as_mut_ptr(),
                path_buffer.len() as u32,
                0,
            )
        } as usize;
        if length == 0 || length >= path_buffer.len() {
            return Ok(None);
        }
        let opened_path = PathBuf::from(std::ffi::OsString::from_wide(&path_buffer[..length]));
        if !opened_path.starts_with(root) {
            return Ok(None);
        }
    }
    Ok(Some(file))
}
fn worktree(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Result<Vec<u8>, Unavailable>, String> {
    let Some(mut file) = worktree_file(root, path, control)? else {
        return Ok(Err(Unavailable::UnsupportedMode));
    };
    let metadata = file.metadata().map_err(|_| invalid())?;
    if !metadata.is_file() {
        return Ok(Err(Unavailable::UnsupportedMode));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Ok(Err(Unavailable::UnsupportedMode));
        }
    }
    if metadata.len() > MAX_IMAGE_BYTES as u64 {
        return Ok(Err(Unavailable::Oversized));
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        control.check()?;
        let count = file.read(&mut chunk).map_err(|_| invalid())?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > MAX_IMAGE_BYTES {
            return Ok(Err(Unavailable::Oversized));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    control.check()?;
    Ok(Ok(bytes))
}
pub(crate) fn collect(
    root: &Path,
    snapshot: &crate::local_review::LocalReviewSnapshot,
    comparison: &Comparison,
    control: &GitRunControl,
) -> Result<Vec<StoredImage>, String> {
    let mut images = Vec::new();
    let mut total = 0usize;
    for kind in &comparison.layers {
        let Some(layer) = snapshot.layers.iter().find(|layer| layer.kind == *kind) else {
            continue;
        };
        let sections = super::super::bitbucket::workspace_prs::snapshot_sections_with_limit(
            &layer.diff,
            PATCH_BYTES,
        )?;
        for section in sections {
            let (file, _) = section.parts();
            if !image_path(&file.path) && !file.old_path.as_deref().is_some_and(image_path) {
                continue;
            }
            for side in [ImageSide::Old, ImageSide::New] {
                control.check()?;
                if images.len() >= IMAGE_COUNT {
                    return Ok(images);
                }
                let absent = side == ImageSide::Old && file.status == "added"
                    || side == ImageSide::New && file.status == "deleted";
                let path = if side == ImageSide::Old {
                    file.old_path.as_deref().unwrap_or(&file.path)
                } else {
                    &file.path
                };
                let content = if absent {
                    Content::Absent {}
                } else if raster_format(path).is_err() {
                    Content::Unavailable {
                        reason: Unavailable::UnsupportedFormat,
                    }
                } else if total >= IMAGE_BYTES {
                    Content::Unavailable {
                        reason: Unavailable::CaptureLimit,
                    }
                } else {
                    let bytes = match (kind, side) {
                        (Layer::Committed, ImageSide::Old) | (Layer::Staged, ImageSide::Old) => {
                            blob(root, path, Some(&snapshot.base_sha), control)?
                        }
                        (Layer::Committed, ImageSide::New) => blob(
                            root,
                            path,
                            Some(snapshot.head_sha.as_deref().ok_or_else(invalid)?),
                            control,
                        )?,
                        (Layer::Staged, ImageSide::New) | (Layer::Unstaged, ImageSide::Old) => {
                            blob(root, path, None, control)?
                        }
                        (Layer::Unstaged, ImageSide::New) => worktree(root, path, control)?,
                    };
                    match bytes {
                        Err(reason) => Content::Unavailable { reason },
                        Ok(bytes) if total + bytes.len() > IMAGE_BYTES => Content::Unavailable {
                            reason: Unavailable::CaptureLimit,
                        },
                        Ok(bytes) => match validate_raster(path, bytes) {
                            Err(_) => Content::Unavailable {
                                reason: Unavailable::InvalidContent,
                            },
                            Ok(raster) => {
                                total += raster.bytes.len();
                                Content::Available {
                                    data_base64: STANDARD.encode(&raster.bytes),
                                    mime_type: raster.mime.into(),
                                    width: raster.width,
                                    height: raster.height,
                                    content_sha256: raster.sha256,
                                }
                            }
                        },
                    }
                };
                images.push(StoredImage {
                    layer: *kind,
                    path: file.path.clone(),
                    side,
                    content_path: path.into(),
                    content,
                });
            }
        }
    }
    control.check()?;
    Ok(images)
}

pub(crate) fn validate(images: &[StoredImage], files: &[&File]) -> Result<(), String> {
    if images.is_empty() || images.len() > IMAGE_COUNT || !images.len().is_multiple_of(2) {
        return Err(invalid());
    }
    let mut keys = std::collections::HashSet::new();
    let mut total = 0usize;
    for image in images {
        let file = files
            .iter()
            .find(|file| file.layer == image.layer && file.path == image.path)
            .ok_or_else(invalid)?;
        let expected_path = if image.side == ImageSide::Old {
            file.old_path.as_deref().unwrap_or(&file.path)
        } else {
            &file.path
        };
        let absent = image.side == ImageSide::Old && file.status == "added"
            || image.side == ImageSide::New && file.status == "deleted";
        if image.content_path != expected_path
            || !safe_image_key(file)
            || !keys.insert((
                rank(image.layer),
                image.path.clone(),
                image.side == ImageSide::Old,
            ))
            || matches!(image.content, Content::Absent {}) != absent
        {
            return Err(invalid());
        }
        if matches!(image.content, Content::Available { .. }) {
            let raster = image.raster()?.ok_or_else(invalid)?;
            total += raster.bytes.len();
            if total > IMAGE_BYTES {
                return Err(invalid());
            }
        }
    }
    for image in images {
        if !keys.contains(&(
            rank(image.layer),
            image.path.clone(),
            image.side != ImageSide::Old,
        )) {
            return Err(invalid());
        }
    }
    Ok(())
}
fn safe_image_key(file: &File) -> bool {
    image_path(&file.path) || file.old_path.as_deref().is_some_and(image_path)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageParams {
    pub snapshot: SnapshotRef,
    pub layer: Layer,
    pub path: String,
    pub side: ImageSide,
    pub offset: usize,
    pub max_bytes: usize,
    #[serde(deserialize_with = "required_hash")]
    pub content_sha256: Option<String>,
}
fn required_hash<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
impl ImageParams {
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if !super::super::provider_snapshot::safe_path(&self.path)
            || self.offset > MAX_IMAGE_BYTES
            || !(1..=IMAGE_CHUNK).contains(&self.max_bytes)
            || self
                .content_sha256
                .as_ref()
                .is_some_and(|sha| !valid_id(sha))
            || self.offset > 0 && self.content_sha256.is_none()
        {
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
    pub reason: Option<String>,
}
impl Manager {
    pub(crate) fn image(&self, request: ImageParams) -> Result<ImagePage, String> {
        request.validate()?;
        let data = self.get(&request.snapshot)?;
        let file = data
            .file(request.layer, &request.path)
            .ok_or_else(invalid)?;
        if !safe_image_key(file) {
            return Err(invalid());
        }
        let image = data.images.as_ref().and_then(|images| {
            images.iter().find(|image| {
                image.layer == request.layer
                    && image.path == request.path
                    && image.side == request.side
            })
        });
        let absent = (request.side == ImageSide::Old && file.status == "added")
            || (request.side == ImageSide::New && file.status == "deleted");
        let (raster, reason) = if absent {
            (None, Some("absent".into()))
        } else {
            match image {
                Some(image) => match &image.content {
                    Content::Available { .. } => (image.raster()?, None),
                    Content::Absent {} => (None, Some("absent".into())),
                    Content::Unavailable { reason } => (
                        None,
                        Some(
                            serde_json::to_value(reason)
                                .map_err(|_| invalid())?
                                .as_str()
                                .ok_or_else(invalid)?
                                .into(),
                        ),
                    ),
                },
                None => (
                    None,
                    Some(
                        if data.images.is_none() {
                            "legacySnapshot"
                        } else {
                            "captureLimit"
                        }
                        .into(),
                    ),
                ),
            }
        };
        let Some(raster) = raster else {
            if request.offset != 0 || request.content_sha256.is_some() {
                return Err(invalid());
            }
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
                reason,
            });
        };
        if request.offset > raster.bytes.len()
            || request
                .content_sha256
                .as_ref()
                .is_some_and(|sha| sha != &raster.sha256)
        {
            return Err(invalid());
        }
        let end = (request.offset + request.max_bytes).min(raster.bytes.len());
        let data_base64 = STANDARD.encode(&raster.bytes[request.offset..end]);
        Ok(ImagePage {
            request,
            available: true,
            mime_type: raster.mime.into(),
            size: raster.bytes.len(),
            data_base64,
            next_offset: (end < raster.bytes.len()).then_some(end),
            content_sha256: Some(raster.sha256.clone()),
            width: Some(raster.width),
            height: Some(raster.height),
            reason: None,
        })
    }
    pub(crate) fn raster(
        &self,
        snapshot: &SnapshotRef,
        layer: Layer,
        path: &str,
        side: ImageSide,
    ) -> Result<Option<Arc<Raster>>, String> {
        let data = self.get(snapshot)?;
        let file = data.file(layer, path).ok_or_else(invalid)?;
        if !safe_image_key(file) {
            return Err(invalid());
        }
        if (side == ImageSide::Old && file.status == "added")
            || (side == ImageSide::New && file.status == "deleted")
        {
            return Ok(None);
        }
        data.images
            .as_ref()
            .and_then(|images| {
                images
                    .iter()
                    .find(|image| image.layer == layer && image.path == path && image.side == side)
            })
            .ok_or_else(|| "This image is unavailable in the saved capture.".to_string())?
            .raster()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn owned_worktree_handle_refuses_symlink_files_and_ancestors_before_reading() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("picture.png"), b"owned outside bytes").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("picture.png"),
            root.path().join("picture.png"),
        )
        .unwrap();
        let cancellation = LocalReviewCancellation::new();
        let control = GitRunControl::new(TIMEOUT, cancellation.clone(), "Owned raster test");
        for path in ["picture.png", "linked/picture.png"] {
            assert!(worktree_file(root.path(), path, &control)
                .unwrap()
                .is_none());
        }
        std::fs::write(root.path().join("regular.png"), b"owned regular bytes").unwrap();
        assert!(worktree_file(root.path(), "regular.png", &control)
            .unwrap()
            .is_some());
        cancellation.cancel();
        assert!(worktree_file(root.path(), "regular.png", &control).is_err());
    }
}
