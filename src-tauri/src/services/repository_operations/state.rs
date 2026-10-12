use super::*;
use crate::local_review::GitRunControl;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
};

pub(super) struct Snapshot {
    pub checkout: Checkout,
    pub branches: Vec<Branch>,
    pub configuration: git::Configuration,
    pub working_version: String,
}
fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn optional_ref(
    root: &Path,
    args: &[&str],
    cfg: &git::Configuration,
    control: &GitRunControl,
) -> Result<Option<String>, String> {
    let output = git::run(root, args, 4096, Some(cfg), true, control)?;
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(invalid());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| invalid())?
        .trim_end_matches(['\r', '\n'])
        .to_owned();
    Ok(Some(text))
}
pub(super) fn snapshot(
    context: &Context,
    root: &Path,
    control: &GitRunControl,
) -> Result<Snapshot, String> {
    control.check()?;
    let configuration = git::configuration(root, control)?;
    // The write command resolves URLs with normal user Git configuration. Its
    // effective destination must independently match the selected provider.
    let origin = git::text(
        root,
        &["remote", "get-url", "--all", "origin"],
        &configuration,
        control,
    )?;
    let origins: Vec<_> = origin.lines().collect();
    if origins.len() != 1 {
        return Err(invalid());
    }
    let identity = crate::local_repo::parse_git_remote(origins[0]).map_err(|_| invalid())?;
    if identity
        != (
            context.repository.provider,
            context.repository.workspace.clone(),
            context.repository.repo.clone(),
        )
    {
        return Err(
            "The effective Git origin does not match the selected provider repository.".into(),
        );
    }
    let head = optional_ref(
        root,
        &["rev-parse", "--verify", "-q", "HEAD"],
        &configuration,
        control,
    )?;
    if head.as_ref().is_some_and(|value| !oid(value)) {
        return Err(invalid());
    }
    let branch = optional_ref(
        root,
        &["symbolic-ref", "-q", "HEAD"],
        &configuration,
        control,
    )?;
    if branch
        .as_ref()
        .is_some_and(|value| !value.starts_with("refs/heads/") || !safe_reference(value))
    {
        return Err(invalid());
    }
    let upstream = if let Some(branch) = branch.as_deref().filter(|_| head.is_some()) {
        let value = git::text(
            root,
            &["for-each-ref", "--format=%(upstream)", branch],
            &configuration,
            control,
        )?;
        if value.is_empty() {
            None
        } else {
            if !safe_reference(&value) {
                return Err(invalid());
            }
            Some(value)
        }
    } else {
        None
    };
    let refs = git::checked(
        root,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(symref)",
        ],
        2 * 1024 * 1024,
        Some(&configuration),
        control,
    )?;
    let mut branches = Vec::new();
    let mut stash_sha = None;
    for row in refs.split(|b| *b == b'\n').filter(|s| !s.is_empty()) {
        let fields: Vec<_> = row.split(|b| *b == 0).collect();
        if fields.len() != 3 {
            return Err(invalid());
        }
        if !fields[2].is_empty() {
            continue;
        }
        let reference = std::str::from_utf8(fields[0]).map_err(|_| invalid())?;
        let object = std::str::from_utf8(fields[1]).map_err(|_| invalid())?;
        if !safe_reference(reference) || !oid(object) {
            return Err(invalid());
        }
        if reference == "refs/stash" {
            stash_sha = Some(object.to_owned());
        }
        let (kind, name) = if let Some(name) = reference.strip_prefix("refs/heads/") {
            (BranchKind::Local, name)
        } else if let Some(name) = reference.strip_prefix("refs/remotes/") {
            (BranchKind::Remote, name)
        } else {
            continue;
        };
        branches.push(Branch {
            reference: reference.into(),
            name: name.into(),
            kind,
            oid: object.into(),
            current: branch.as_deref() == Some(reference),
        });
        if branches.len() > 10_000 {
            return Err("Too many branch references to verify this checkout.".into());
        }
    }
    let status = git::checked(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        1024 * 1024,
        Some(&configuration),
        control,
    )?;
    let index = git::checked(
        root,
        &["ls-files", "--stage", "-z"],
        4 * 1024 * 1024,
        Some(&configuration),
        control,
    )?;
    let mut digest = Sha256::new();
    for bytes in [
        context.root_id.as_bytes(),
        configuration.digest.as_bytes(),
        origin.as_bytes(),
        head.as_deref().unwrap_or("").as_bytes(),
        branch.as_deref().unwrap_or("").as_bytes(),
        upstream.as_deref().unwrap_or("").as_bytes(),
        index.as_slice(),
        status.as_slice(),
    ] {
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    for argument in ["--absolute-git-dir", "--git-common-dir"] {
        let value = git::text(root, &["rev-parse", argument], &configuration, control)?;
        let path = root.join(value).canonicalize().map_err(|_| invalid())?;
        let value = path.to_str().ok_or_else(invalid)?;
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    let mut pending = false;
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "REBASE_HEAD",
        "rebase-apply",
        "rebase-merge",
        "sequencer",
        "BISECT_LOG",
    ] {
        let value = git::text(
            root,
            &["rev-parse", "--git-path", name],
            &configuration,
            control,
        )?;
        let path = root.join(value);
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                pending = true;
                digest.update(name.as_bytes());
                digest.update(metadata.len().to_be_bytes());
                let modified = metadata
                    .modified()
                    .map_err(|_| invalid())?
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| invalid())?
                    .as_nanos();
                digest.update(modified.to_be_bytes());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(invalid()),
        }
    }
    let mut entries = status.split(|b| *b == 0).filter(|s| !s.is_empty());
    let mut count = 0usize;
    let mut bytes = 0usize;
    let mut conflicted = false;
    while let Some(entry) = entries.next() {
        control.check()?;
        count += 1;
        if count > 10_000 || entry.len() < 4 || entry[2] != b' ' {
            return Err(invalid());
        }
        conflicted |= entry[..2].contains(&b'U') || matches!(&entry[..2], b"AA" | b"DD");
        let name = std::str::from_utf8(&entry[3..]).map_err(|_| invalid())?;
        let relative = Path::new(name);
        if name.len() > 4096
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(invalid());
        }
        if entry[..2].iter().any(|b| matches!(b, b'R' | b'C')) {
            let old = entries.next().ok_or_else(invalid)?;
            digest.update((old.len() as u64).to_be_bytes());
            digest.update(old);
        }
        let path = root.join(relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && entry[..2].contains(&b'D') =>
            {
                digest.update(b"deleted");
                continue;
            }
            Err(_) => return Err(invalid()),
        };
        if metadata.is_symlink() {
            let parent = path
                .parent()
                .ok_or_else(invalid)?
                .canonicalize()
                .map_err(|_| invalid())?;
            if !parent.starts_with(root) {
                return Err(invalid());
            }
            let target = fs::read_link(&path).map_err(|_| invalid())?;
            if path
                .parent()
                .ok_or_else(invalid)?
                .canonicalize()
                .map_err(|_| invalid())?
                != parent
            {
                return Err(invalid());
            }
            let target = target.to_str().ok_or_else(invalid)?;
            digest.update(b"link");
            digest.update((target.len() as u64).to_be_bytes());
            digest.update(target.as_bytes());
        } else if metadata.is_file() {
            let canonical = path.canonicalize().map_err(|_| invalid())?;
            let name = canonical
                .strip_prefix(root)
                .map_err(|_| invalid())?
                .to_str()
                .ok_or_else(invalid)?;
            #[cfg(windows)]
            let normalized_name = name.replace('\\', "/");
            #[cfg(windows)]
            let name = normalized_name.as_str();
            let mut file =
                crate::services::local_fs::open_file(root, name, control)?.ok_or_else(invalid)?;
            let length = file.metadata().map_err(|_| invalid())?.len();
            if length > 4 * 1024 * 1024 {
                return Err("A changed file exceeds repository approval limits.".into());
            }
            bytes = bytes.checked_add(length as usize).ok_or_else(invalid)?;
            if bytes > 16 * 1024 * 1024 {
                return Err("Changed content exceeds repository approval limits.".into());
            }
            digest.update(length.to_be_bytes());
            let mut buffer = [0u8; 32 * 1024];
            let mut read = 0u64;
            loop {
                control.check()?;
                let size = file.read(&mut buffer).map_err(|_| invalid())?;
                if size == 0 {
                    break;
                }
                read += size as u64;
                if read > length {
                    return Err(invalid());
                }
                digest.update(&buffer[..size]);
            }
            if read != length {
                return Err(invalid());
            }
        } else {
            return Err("Special files cannot be approved for repository operations.".into());
        }
    }
    if git::configuration(root, control)?.digest != configuration.digest
        || git::checked(
            root,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            1024 * 1024,
            Some(&configuration),
            control,
        )? != status
    {
        return Err("The checkout changed while being read. Refresh it.".into());
    }
    for branch in branches
        .iter()
        .filter(|branch| branch.kind == BranchKind::Local)
    {
        digest.update((branch.reference.len() as u64).to_be_bytes());
        digest.update(branch.reference.as_bytes());
        digest.update(branch.oid.as_bytes());
    }
    let working_version = format!("{:x}", digest.clone().finalize());
    digest.update((refs.len() as u64).to_be_bytes());
    digest.update(&refs);
    let filtered = !configuration.filters.is_empty();
    let checkout = Checkout {repository: context.repository.clone(), context: Some(context.clone()),
        state: if head.is_some() { CheckoutState::Ready } else { CheckoutState::Unborn },
        branch: branch.and_then(|s| s.strip_prefix("refs/heads/").map(ToOwned::to_owned)),
        head_sha: head, upstream, stash_sha, dirty: !status.is_empty(), filtered, conflicted, pending,
        version: Some(format!("{:x}", digest.finalize())),
        message: filtered.then(|| "Git filters are configured; this preview compares raw file contents. Git checks its filtered state only after confirmation.".into())};
    Ok(Snapshot {
        checkout,
        branches,
        configuration,
        working_version,
    })
}
