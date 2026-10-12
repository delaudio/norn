//! Shared bounded native file handles for a previously verified canonical root.
use crate::local_review::GitRunControl;
use std::path::Path;
#[cfg(not(unix))]
use std::path::PathBuf;
fn invalid() -> String {
    "The local file could not be verified.".into()
}
pub(crate) fn open_file(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Option<std::fs::File>, String> {
    if !root.is_absolute()
        || path.is_empty()
        || path.len() > 4096
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || path.starts_with('/')
    {
        return Err(invalid());
    }
    #[cfg(windows)]
    if path.contains(['\\', ':']) {
        return Err(invalid());
    }
    open_file_at(root, path, control)
}
#[cfg(unix)]
fn open_file_at(
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
fn open_file_at(
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
