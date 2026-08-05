use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const CANONICAL_APP_IDENTIFIER: &str = "app.norn.desktop";
const LEGACY_APP_IDENTIFIER: &str = "app.lachesi.desktop";

/// Resolve a canonical environment variable before its compatibility alias.
pub(crate) fn env_var_os(canonical: &str, legacy: &str) -> Option<OsString> {
    std::env::var_os(canonical).or_else(|| std::env::var_os(legacy))
}

pub(crate) fn env_var(canonical: &str, legacy: &str) -> Option<String> {
    env_var_os(canonical, legacy).map(|value| value.to_string_lossy().into_owned())
}

/// Copy a legacy file into its canonical location without replacing either
/// an existing canonical file or the recoverable legacy source.
pub(crate) fn migrate_file_atomically(
    source: &Path,
    destination: &Path,
    validate: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<bool, String> {
    if destination.exists() || !source.is_file() {
        return Ok(false);
    }

    let bytes = fs::read(source).map_err(|error| error.to_string())?;
    validate(&bytes)?;
    let parent = destination
        .parent()
        .ok_or_else(|| "Canonical migration path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;

    let mut temporary = tempfile::Builder::new()
        .prefix(".norn-migration-")
        .tempfile_in(parent)
        .map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    match temporary.persist_noclobber(destination) {
        Ok(_) => Ok(true),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.error.to_string()),
    }
}

/// Copy the legacy WebView profile before Tauri creates the canonical profile.
/// The source is retained for rollback and publication is atomic/no-clobber.
pub(crate) fn migrate_webview_storage() -> Result<bool, String> {
    let Some((legacy, canonical)) = webview_storage_directories() else {
        return Ok(false);
    };
    migrate_directory_atomically(&legacy, &canonical).map_err(|error| {
        format!(
            "Norn could not migrate legacy browser storage from {} to {}: {error}. The legacy profile remains unchanged; remove any incomplete canonical profile and restart Norn to retry.",
            legacy.display(),
            canonical.display()
        )
    })
}

#[cfg(target_os = "macos")]
fn webview_storage_directories() -> Option<(PathBuf, PathBuf)> {
    let root = dirs::home_dir()?.join("Library/WebKit");
    Some((
        root.join(LEGACY_APP_IDENTIFIER),
        root.join(CANONICAL_APP_IDENTIFIER),
    ))
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn webview_storage_directories() -> Option<(PathBuf, PathBuf)> {
    let root = dirs::data_local_dir()?;
    Some((
        root.join(LEGACY_APP_IDENTIFIER),
        root.join(CANONICAL_APP_IDENTIFIER),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webview_storage_directories() -> Option<(PathBuf, PathBuf)> {
    None
}

fn migrate_directory_atomically(source: &Path, destination: &Path) -> Result<bool, String> {
    let source_metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Err("legacy browser storage must be a regular directory".to_string());
    }
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            return Ok(false)
        }
        Ok(_) => {
            return Err(
                "canonical browser storage exists but is not a regular directory".to_string(),
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    let parent = destination
        .parent()
        .ok_or_else(|| "Canonical browser-storage path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let staged = tempfile::Builder::new()
        .prefix(".norn-webview-migration-")
        .tempdir_in(parent)
        .map_err(|error| error.to_string())?;
    copy_directory_contents(source, staged.path())?;
    let staged_path = staged.keep();
    if let Err(error) = rename_directory_noclobber(&staged_path, destination) {
        let _ = fs::remove_dir_all(&staged_path);
        if fs::symlink_metadata(destination).is_ok() {
            return Ok(false);
        }
        return Err(error.to_string());
    }
    Ok(true)
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "legacy browser storage contains unsupported symbolic link {}",
                source_path.display()
            ));
        }
        if metadata.is_dir() {
            fs::create_dir(&destination_path).map_err(|error| error.to_string())?;
            copy_directory_contents(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|error| error.to_string())?;
            fs::set_permissions(&destination_path, metadata.permissions())
                .map_err(|error| error.to_string())?;
        } else {
            return Err(format!(
                "legacy browser storage contains unsupported entry {}",
                source_path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn rename_directory_noclobber(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid source path")
    })?;
    let target = CString::new(target.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid target path")
    })?;

    #[cfg(target_os = "macos")]
    let result = unsafe { libc::renamex_np(source.as_ptr(), target.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };

    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn rename_directory_noclobber(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(crate) fn rename_directory_noclobber(_source: &Path, _target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-clobber directory migration is unsupported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_migration_is_validated_idempotent_and_keeps_legacy_source() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("lachesi/settings.json");
        let destination = root.path().join("norn/settings.json");
        fs::create_dir_all(source.parent().expect("source parent")).expect("source directory");
        fs::write(&source, br#"{"theme":"dark"}"#).expect("legacy settings");

        let validate = |bytes: &[u8]| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .map(|_| ())
                .map_err(|error| error.to_string())
        };
        assert!(migrate_file_atomically(&source, &destination, validate).expect("first migration"));
        assert!(
            !migrate_file_atomically(&source, &destination, validate).expect("second migration")
        );
        assert!(source.exists());
        assert_eq!(
            fs::read(&destination).expect("canonical settings"),
            br#"{"theme":"dark"}"#
        );
    }

    #[test]
    fn invalid_source_never_creates_a_canonical_file() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("lachesi/settings.json");
        let destination = root.path().join("norn/settings.json");
        fs::create_dir_all(source.parent().expect("source parent")).expect("source directory");
        fs::write(&source, b"not-json").expect("legacy settings");

        let result = migrate_file_atomically(&source, &destination, |bytes| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .map(|_| ())
                .map_err(|error| error.to_string())
        });

        assert!(result.is_err());
        assert!(source.exists());
        assert!(!destination.exists());
    }

    #[test]
    fn directory_migration_is_atomic_idempotent_and_keeps_legacy_source() {
        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("app.lachesi.desktop");
        let destination = root.path().join("app.norn.desktop");
        fs::create_dir_all(source.join("WebsiteData/LocalStorage"))
            .expect("legacy browser directory");
        fs::write(
            source.join("WebsiteData/LocalStorage/state.sqlite"),
            b"legacy browser state",
        )
        .expect("legacy browser state");

        assert!(migrate_directory_atomically(&source, &destination).expect("first migration"));
        assert!(!migrate_directory_atomically(&source, &destination).expect("second migration"));
        assert!(source.exists());
        assert_eq!(
            fs::read(destination.join("WebsiteData/LocalStorage/state.sqlite"))
                .expect("canonical browser state"),
            b"legacy browser state"
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_migration_rejects_symlinks_without_publishing() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("tempdir");
        let source = root.path().join("app.lachesi.desktop");
        let destination = root.path().join("app.norn.desktop");
        fs::create_dir(&source).expect("legacy browser directory");
        symlink(root.path(), source.join("escape")).expect("legacy symlink");

        assert!(migrate_directory_atomically(&source, &destination).is_err());
        assert!(source.exists());
        assert!(!destination.exists());
    }
}
