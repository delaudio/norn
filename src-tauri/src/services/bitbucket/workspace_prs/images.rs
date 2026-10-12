//! Immutable regular-file raster bytes; no mutable content or download URLs.
use super::*;
use crate::services::provider_snapshot::{full_sha, safe_path};
use sha2::{Digest, Sha256};
const MAX_IMAGE: usize = 2 * 1024 * 1024;
const MAX_METADATA: usize = 8 * 1024 * 1024;
impl Api {
    pub(crate) fn immutable_image(
        &self,
        repository: &RepositoryIdentity,
        revision: &str,
        path: &str,
    ) -> Result<Vec<u8>, String> {
        repository.validate()?;
        let revision = full_sha(revision)?;
        if repository.provider != self.repository.provider || !safe_path(path) {
            return Err(error());
        }
        let parts = path.split('/').collect::<Vec<_>>();
        if parts.len() > 64 {
            return Err(error());
        }
        let api = Self {
            http: self.http.clone(),
            auth: self.auth.clone(),
            repository: repository.clone(),
            root: self.root.clone(),
        };
        let started = Instant::now();
        match repository.provider {
            ReviewProvider::Github => {
                let (commit, _) =
                    api.get(&format!("{}/git/commits/{revision}", api.base()), started)?;
                if full_sha(
                    commit
                        .get("sha")
                        .and_then(Value::as_str)
                        .ok_or_else(error)?,
                )? != revision
                {
                    return Err(error());
                }
                let mut tree = full_sha(
                    commit
                        .pointer("/tree/sha")
                        .and_then(Value::as_str)
                        .ok_or_else(error)?,
                )?;
                let mut metadata_bytes = serde_json::to_vec(&commit).map_err(|_| error())?.len();
                for (index, part) in parts.iter().enumerate() {
                    if tree.len() != revision.len() {
                        return Err(error());
                    }
                    let (value, _) =
                        api.get(&format!("{}/git/trees/{tree}", api.base()), started)?;
                    metadata_bytes += serde_json::to_vec(&value).map_err(|_| error())?.len();
                    if metadata_bytes > MAX_METADATA
                        || value.get("truncated") != Some(&Value::Bool(false))
                        || full_sha(value.get("sha").and_then(Value::as_str).ok_or_else(error)?)?
                            != tree
                    {
                        return Err(error());
                    }
                    let entries = value
                        .get("tree")
                        .and_then(Value::as_array)
                        .ok_or_else(error)?;
                    if entries.len() > 10_000 {
                        return Err(error());
                    }
                    let mut matching = entries
                        .iter()
                        .filter(|entry| entry.get("path").and_then(Value::as_str) == Some(*part));
                    let entry = matching.next().ok_or_else(error)?;
                    if matching.next().is_some() {
                        return Err(error());
                    }
                    let sha =
                        full_sha(entry.get("sha").and_then(Value::as_str).ok_or_else(error)?)?;
                    if sha.len() != revision.len() {
                        return Err(error());
                    }
                    if index + 1 < parts.len() {
                        if entry.get("type").and_then(Value::as_str) != Some("tree")
                            || entry.get("mode").and_then(Value::as_str) != Some("040000")
                        {
                            return Err(error());
                        }
                        tree = sha;
                    } else {
                        if entry.get("type").and_then(Value::as_str) != Some("blob")
                            || !matches!(
                                entry.get("mode").and_then(Value::as_str),
                                Some("100644" | "100755")
                            )
                        {
                            return Err(
                                "Only regular immutable image files can be previewed.".into()
                            );
                        }
                        let size = entry
                            .get("size")
                            .and_then(Value::as_u64)
                            .filter(|size| *size > 0 && *size <= MAX_IMAGE as u64)
                            .ok_or_else(error)?;
                        let (bytes, _) = api.get_bytes(
                            &format!("{}/git/blobs/{sha}", api.base()),
                            started,
                            MAX_IMAGE,
                            "application/vnd.github.raw+json",
                        )?;
                        if bytes.len() as u64 != size || git_blob_hash(&bytes, sha.len()) != sha {
                            return Err(error());
                        }
                        return Ok(bytes);
                    }
                }
                Err(error())
            }
            ReviewProvider::Bitbucket => {
                let encoded = parts
                    .iter()
                    .map(|part| encode_path_segment(part))
                    .collect::<Vec<_>>()
                    .join("/");
                let url = format!("{}/src/{revision}/{encoded}", api.base());
                let (meta, _) = api.get(&format!("{url}?format=meta"), started)?;
                if meta.get("type").and_then(Value::as_str) != Some("commit_file")
                    || meta.get("path").and_then(Value::as_str) != Some(path)
                    || full_sha(
                        meta.pointer("/commit/hash")
                            .and_then(Value::as_str)
                            .ok_or_else(error)?,
                    )? != revision
                {
                    return Err(error());
                }
                let attributes = meta
                    .get("attributes")
                    .and_then(Value::as_array)
                    .ok_or_else(error)?;
                if attributes.len() > 16
                    || attributes
                        .iter()
                        .any(|attr| !matches!(attr.as_str(), Some("binary" | "executable")))
                {
                    return Err("Only regular immutable image files can be previewed.".into());
                }
                let size = meta
                    .get("size")
                    .and_then(Value::as_u64)
                    .filter(|size| *size > 0 && *size <= MAX_IMAGE as u64)
                    .ok_or_else(error)?;
                let (bytes, _) =
                    api.get_bytes(&url, started, MAX_IMAGE, "application/octet-stream")?;
                if bytes.len() as u64 != size {
                    return Err(error());
                }
                Ok(bytes)
            }
        }
    }
}
fn git_blob_hash(bytes: &[u8], length: usize) -> String {
    let header = format!("blob {}\0", bytes.len());
    if length == 40 {
        let mut hash = sha1::Sha1::new();
        hash.update(header.as_bytes());
        hash.update(bytes);
        format!("{:x}", hash.finalize())
    } else {
        let mut hash = Sha256::new();
        hash.update(header.as_bytes());
        hash.update(bytes);
        format!("{:x}", hash.finalize())
    }
}
