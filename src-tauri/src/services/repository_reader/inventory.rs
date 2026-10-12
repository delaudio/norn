use super::*;
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct File {
    pub path: String,
    pub old_path: Option<String>,
    pub status: RepositoryFileStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilesParams {
    pub context: Context,
    pub offset: usize,
    pub limit: usize,
    pub query: String,
    pub changed_only: bool,
    #[serde(deserialize_with = "required_hash")]
    pub version: Option<String>,
}
fn required_hash<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
impl FilesParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        if self.offset > MAX_FILES
            || !(1..=32).contains(&self.limit)
            || self.query.len() > 256
            || self.query.chars().any(char::is_control)
            || self
                .version
                .as_ref()
                .is_some_and(|value| !valid_hash(value))
            || self.offset > 0 && self.version.is_none()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilesPage {
    pub request: FilesParams,
    pub version: String,
    pub total: usize,
    pub files: Vec<File>,
    pub next_offset: Option<usize>,
}
pub(crate) struct Inventory {
    pub files: Vec<File>,
    pub version: String,
}
pub(crate) fn collect(root: &Path, control: &GitRunControl) -> Result<Inventory, String> {
    let initial_head = git::head(root, control)?;
    let index = git::checked(root, &["ls-files", "--stage", "-z"], MAX_METADATA, control)?;
    let tracked = git::checked(
        root,
        &["ls-files", "-z", "-co", "--exclude-standard"],
        MAX_METADATA,
        control,
    )?;
    let status = git::checked(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        MAX_METADATA,
        control,
    )?;
    let tracked_text = std::str::from_utf8(&tracked)
        .map_err(|_| "Repository contains filenames that are not valid UTF-8.".to_string())?;
    let status_text = std::str::from_utf8(&status)
        .map_err(|_| "Repository contains filenames that are not valid UTF-8.".to_string())?;
    let mut files = BTreeMap::new();
    for path in tracked_text.split_terminator('\0') {
        if path.is_empty() {
            return Err(invalid());
        }
        context::validate_path(path)?;
        files.entry(path.to_owned()).or_insert(File {
            path: path.into(),
            old_path: None,
            status: RepositoryFileStatus::Unchanged,
        });
        if files.len() > MAX_FILES {
            return Err("Repository inventory exceeds its preview limit.".into());
        }
    }
    let mut parts = status_text.split_terminator('\0');
    while let Some(entry) = parts.next() {
        control.check()?;
        let bytes = entry.as_bytes();
        if bytes.len() < 4 || bytes[2] != b' ' || !bytes[0].is_ascii() || !bytes[1].is_ascii() {
            return Err(invalid());
        }
        let path = &entry[3..];
        context::validate_path(path)?;
        let old_path = if matches!(bytes[0], b'R' | b'C') || matches!(bytes[1], b'R' | b'C') {
            let old = parts.next().ok_or_else(invalid)?;
            context::validate_path(old)?;
            Some(old.to_owned())
        } else {
            None
        };
        files.insert(
            path.into(),
            File {
                path: path.into(),
                old_path,
                status: legacy::file_status_from_porcelain(bytes[0] as char, bytes[1] as char),
            },
        );
        if files.len() > MAX_FILES {
            return Err("Repository inventory exceeds its preview limit.".into());
        }
    }
    let head = git::head(root, control)?;
    if head != initial_head {
        return Err("Repository changed during listing. Refresh explicitly.".into());
    }
    let mut sha = Sha256::new();
    // Length delimiters avoid concatenation ambiguity, including hostile paths.
    for bytes in [
        b"norn-repository-inventory-v1".as_slice(),
        tracked.as_slice(),
        status.as_slice(),
        index.as_slice(),
        head.as_deref().unwrap_or("").as_bytes(),
    ] {
        sha.update((bytes.len() as u64).to_le_bytes());
        sha.update(bytes);
    }
    Ok(Inventory {
        files: files.into_values().collect(),
        version: format!("{:x}", sha.finalize()),
    })
}
pub(crate) fn list_at(
    root: &Path,
    control: &GitRunControl,
) -> Result<Vec<RepositoryFileEntry>, String> {
    Ok(collect(root, control)?
        .files
        .into_iter()
        .map(|file| RepositoryFileEntry {
            path: file.path,
            status: file.status,
        })
        .collect())
}
pub(crate) fn page(params: FilesParams, control: &GitRunControl) -> Result<FilesPage, String> {
    params.validate()?;
    let root = context::resolve_context(&params.context, control)?;
    let first = collect(&root, control)?;
    let query = params.query.trim().to_lowercase();
    let mut sha = Sha256::new();
    sha.update(first.version.as_bytes());
    sha.update([u8::from(params.changed_only)]);
    sha.update(query.as_bytes());
    let version = format!("{:x}", sha.finalize());
    if params
        .version
        .as_ref()
        .is_some_and(|expected| expected != &version)
    {
        return Err("Repository files changed. Refresh the file list explicitly.".into());
    }
    let matching: Vec<_> = first
        .files
        .into_iter()
        .filter(|file| {
            (!params.changed_only || file.status != RepositoryFileStatus::Unchanged)
                && (query.is_empty() || file.path.to_lowercase().contains(&query))
        })
        .collect();
    let total = matching.len();
    if params.offset > total {
        return Err(invalid());
    }
    let end = (params.offset + params.limit).min(total);
    let files = matching[params.offset..end].to_vec();
    let second = collect(&root, control)?;
    if first.version != second.version
        || context::resolve_context(&params.context, control)? != root
    {
        return Err("Repository files changed during listing. Refresh explicitly.".into());
    }
    Ok(FilesPage {
        request: params,
        version,
        total,
        files,
        next_offset: (end < total).then_some(end),
    })
}
