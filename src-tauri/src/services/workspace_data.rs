//! Canonical user-authored workspace data and non-destructive desktop imports.
use crate::config::ReviewProvider;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingReference {
    pub review_run_id: String,
    pub finding_id: String,
    pub finding_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftComment {
    pub local_id: String,
    pub pr_id: u32,
    pub path: String,
    pub to: Option<u32>,
    pub from: Option<u32>,
    pub raw: String,
    pub parent_id: Option<String>,
    pub created_at: u64,
    pub source: Option<String>,
    pub finding_ref: Option<FindingReference>,
    pub publication_mode: Option<String>,
    pub review_base_sha: Option<String>,
    pub review_head_sha: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewReference {
    pub id: String,
    pub r#type: String,
    pub source: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub key: Option<String>,
    pub workspace: Option<String>,
    pub repo: Option<String>,
    pub pr_id: Option<u32>,
    pub local_path: Option<String>,
    pub body: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum WorkspaceValue {
    Drafts(Vec<DraftComment>),
    References(Vec<ReviewReference>),
    Viewed(Vec<String>),
    Prompt(String),
    Theme(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceRecord {
    pub key: String,
    pub payload: WorkspaceValue,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceDocument {
    pub schema_version: u32,
    pub records: Vec<WorkspaceRecord>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopEntry {
    pub key: String,
    pub value: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopExport {
    pub schema_version: u32,
    pub source: String,
    pub entries: Vec<DesktopEntry>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub input_version: String,
    pub destination_version: String,
    pub additions_count: usize,
    pub unchanged_count: usize,
    pub conflicts_count: usize,
    pub rejected_count: usize,
    pub details_truncated: bool,
    pub additions: Vec<String>,
    pub unchanged: Vec<String>,
    pub conflicts: Vec<String>,
    pub rejected: Vec<ImportRejection>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRejection {
    pub entry: usize,
    pub reason: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub unchanged: usize,
    pub conflicts: usize,
    pub rejected_count: usize,
    pub details_truncated: bool,
    pub rejected: Vec<ImportRejection>,
    pub backup_path: Option<String>,
    pub destination_version: String,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn bounded_text(value: &str, maximum: usize, multiline: bool) -> Result<(), String> {
    if value.len() > maximum
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
    {
        Err(
            "A user-data field exceeds its limit or contains unsupported control characters."
                .into(),
        )
    } else {
        Ok(())
    }
}
fn identity(value: &str) -> Result<(), String> {
    bounded_text(value, 256, false)?;
    if value.trim().is_empty()
        || matches!(value, "." | "..")
        || value.contains('/')
        || value.contains(':')
        || value.contains('#')
    {
        return Err("User-data scope has an invalid owner or repository.".into());
    }
    Ok(())
}
impl WorkspaceRecord {
    pub fn validate(&self) -> Result<(), String> {
        bounded_text(&self.key, 1024, false)?;
        let kind = match &self.payload {
            WorkspaceValue::Drafts(items) => {
                if items.len() > 1000 {
                    return Err("Too many drafts in one scope.".into());
                }
                let mut seen = HashSet::new();
                for item in items {
                    bounded_text(&item.local_id, 256, false)?;
                    if item.local_id.is_empty() || !seen.insert(&item.local_id) || item.pr_id == 0 {
                        return Err("Invalid or duplicate draft identity.".into());
                    }
                    bounded_text(&item.path, 4096, false)?;
                    bounded_text(&item.raw, 65_536, true)?;
                    if item.to == Some(0) || item.from == Some(0) {
                        return Err("Draft lines must be positive.".into());
                    }
                    if item
                        .source
                        .as_deref()
                        .is_some_and(|s| !matches!(s, "manual" | "aiFinding"))
                        || item.publication_mode.as_deref().is_some_and(|s| {
                            !matches!(s, "inline" | "file" | "general" | "localOnly")
                        })
                    {
                        return Err("Unsupported draft source or publication mode.".into());
                    }
                    for value in [
                        &item.parent_id,
                        &item.review_base_sha,
                        &item.review_head_sha,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        bounded_text(value, 256, false)?;
                    }
                    if let Some(reference) = &item.finding_ref {
                        for value in [
                            &reference.review_run_id,
                            &reference.finding_id,
                            &reference.finding_fingerprint,
                        ] {
                            bounded_text(value, 256, false)?;
                        }
                    }
                }
                "drafts"
            }
            WorkspaceValue::References(items) => {
                if items.len() > 1000 {
                    return Err("Too many references in one scope.".into());
                }
                let mut seen = HashSet::new();
                for item in items {
                    bounded_text(&item.id, 256, false)?;
                    if item.id.is_empty()
                        || !seen.insert(&item.id)
                        || !matches!(
                            item.r#type.as_str(),
                            "pullRequest" | "repository" | "jira" | "notion" | "note"
                        )
                        || !matches!(item.source.as_str(), "detected" | "manual")
                    {
                        return Err("Invalid reference identity or type.".into());
                    }
                    for value in [&item.title, &item.key, &item.workspace, &item.repo]
                        .into_iter()
                        .flatten()
                    {
                        bounded_text(value, 1024, false)?;
                    }
                    for value in [&item.url, &item.local_path].into_iter().flatten() {
                        bounded_text(value, 4096, false)?;
                    }
                    if let Some(body) = &item.body {
                        bounded_text(body, 65_536, true)?;
                    }
                }
                "references"
            }
            WorkspaceValue::Viewed(items) => {
                if items.len() > 10_000 {
                    return Err("Too many viewed files in one scope.".into());
                }
                let mut seen = HashSet::new();
                for item in items {
                    bounded_text(item, 4096, false)?;
                    if item.is_empty() || !seen.insert(item) {
                        return Err("Invalid or duplicate viewed path.".into());
                    }
                }
                "viewed"
            }
            WorkspaceValue::Prompt(value) => {
                bounded_text(value, 65_536, true)?;
                "prompt"
            }
            WorkspaceValue::Theme(value) => {
                if !matches!(value.as_str(), "dark" | "light") || self.key != "theme" {
                    return Err("Unsupported theme record.".into());
                }
                return Ok(());
            }
        };
        let suffix = self
            .key
            .strip_prefix(&format!("{kind}."))
            .ok_or("Record kind and scope do not match.")?;
        let (provider, scope) = suffix
            .split_once(':')
            .ok_or("Record requires a provider scope.")?;
        if !matches!(provider, "github" | "bitbucket") {
            return Err("Unsupported user-data provider.".into());
        }
        let parts = scope.split('/').collect::<Vec<_>>();
        let count = if kind == "prompt" { 2 } else { 3 };
        if parts.len() != count {
            return Err("Invalid user-data scope.".into());
        }
        identity(parts[0])?;
        identity(parts[1])?;
        if count == 3 {
            let pr = parts[2]
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("Invalid pull request identity.")?;
            if let WorkspaceValue::Drafts(items) = &self.payload {
                if items.iter().any(|item| item.pr_id != pr) {
                    return Err("Draft pull request does not match its scope.".into());
                }
            }
        }
        Ok(())
    }
}
fn read_bounded(path: &Path, missing: bool) -> Result<Vec<u8>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if missing && e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("Cannot read user-data file.".into()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("User-data file must be a regular file, not a symlink.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "Cannot open user-data file.")?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read user-data file.")?;
    if bytes.len() > MAX_BYTES {
        return Err("User-data file exceeds 16 MiB; split its export before importing.".into());
    }
    Ok(bytes)
}
fn read_destination(path: &Path) -> Result<(WorkspaceDocument, String, Vec<u8>), String> {
    let bytes = read_bounded(path, true)?;
    if bytes.is_empty() && path.exists() {
        return Err(
            "Stored user data is empty or interrupted; preserve it before retrying.".into(),
        );
    }
    let document = if bytes.is_empty() {
        WorkspaceDocument {
            schema_version: 1,
            records: Vec::new(),
        }
    } else {
        serde_json::from_slice::<WorkspaceDocument>(&bytes)
            .map_err(|_| "Stored user data is invalid; preserve it before retrying.")?
    };
    validate_document(&document)?;
    Ok((document, digest(&bytes), bytes))
}
fn validate_document(document: &WorkspaceDocument) -> Result<(), String> {
    if document.schema_version != 1 || document.records.len() > MAX_ENTRIES {
        return Err("Unsupported user-data version or record count.".into());
    }
    let mut seen = HashSet::new();
    for record in &document.records {
        record.validate()?;
        if !seen.insert(&record.key) {
            return Err("Duplicate canonical user-data scope.".into());
        }
    }
    Ok(())
}
fn provider_name(provider: ReviewProvider) -> &'static str {
    if provider == ReviewProvider::Github {
        "github"
    } else {
        "bitbucket"
    }
}
fn normalize(
    entry: &DesktopEntry,
    provider: Option<ReviewProvider>,
) -> Result<WorkspaceRecord, String> {
    bounded_text(&entry.key, 1024, false)?;
    bounded_text(&entry.value, 256 * 1024, true)?;
    let (kind, suffix) = if let Some(s) = entry
        .key
        .strip_prefix("norn.drafts.v1.")
        .or_else(|| entry.key.strip_prefix("lachesi.drafts."))
    {
        ("drafts", s)
    } else if let Some(s) = entry
        .key
        .strip_prefix("norn.reviewReferences.v1.")
        .or_else(|| entry.key.strip_prefix("lachesi.reviewReferences."))
    {
        ("references", s)
    } else if let Some(s) = entry
        .key
        .strip_prefix("norn.viewedFiles.v1.")
        .or_else(|| entry.key.strip_prefix("lachesi.viewedFiles."))
    {
        ("viewed", s)
    } else if let Some(s) = entry
        .key
        .strip_prefix("norn.reviewPrompt.v1.")
        .or_else(|| entry.key.strip_prefix("lachesi.reviewPrompt."))
    {
        ("prompt", s)
    } else if matches!(entry.key.as_str(), "norn.theme.v1" | "lachesi.theme") {
        return Ok(WorkspaceRecord {
            key: "theme".into(),
            payload: WorkspaceValue::Theme(entry.value.clone()),
        });
    } else {
        return Err("Entry is not an allowlisted user-data key.".into());
    };
    let scope = if kind == "drafts" {
        suffix.to_owned()
    } else {
        format!(
            "{}:{}",
            provider_name(provider.ok_or("Historical scope requires an explicit provider.")?),
            suffix.replace('#', "/")
        )
    };
    let payload = match kind {
        "drafts" => {
            let mut value: serde_json::Value =
                serde_json::from_str(&entry.value).map_err(|_| "Invalid draft JSON.")?;
            if let Some(items) = value.as_array_mut() {
                for item in items {
                    if let Some(parent) = item.get_mut("parentId") {
                        if parent.is_number() {
                            *parent = serde_json::Value::String(parent.to_string());
                        }
                    }
                }
            }
            WorkspaceValue::Drafts(
                serde_json::from_value(value)
                    .map_err(|_| "Draft fields do not match the supported schema.")?,
            )
        }
        "references" => WorkspaceValue::References(
            serde_json::from_str(&entry.value)
                .map_err(|_| "Reference fields do not match the supported schema.")?,
        ),
        "viewed" => WorkspaceValue::Viewed(
            serde_json::from_str(&entry.value).map_err(|_| "Invalid viewed-file JSON.")?,
        ),
        "prompt" => WorkspaceValue::Prompt(entry.value.clone()),
        _ => unreachable!(),
    };
    let record = WorkspaceRecord {
        key: format!("{kind}.{scope}"),
        payload,
    };
    record.validate()?;
    Ok(record)
}
fn input_records(
    bytes: &[u8],
    provider: Option<ReviewProvider>,
) -> Result<(Vec<WorkspaceRecord>, Vec<ImportRejection>), String> {
    // Native exports are already scoped. Desktop exports retain the original keys.
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "Invalid user-data JSON; source was not changed.")?;
    if value.get("records").is_some() {
        let document: WorkspaceDocument =
            serde_json::from_value(value).map_err(|_| "Invalid native export schema.")?;
        validate_document(&document)?;
        return Ok((document.records, Vec::new()));
    }
    let export: DesktopExport =
        serde_json::from_value(value).map_err(|_| "Invalid desktop export schema.")?;
    if export.schema_version != 1
        || export.source != "desktopLocalStorage"
        || export.entries.len() > MAX_ENTRIES
    {
        return Err("Unsupported desktop export version, source or entry count.".into());
    }
    let canonical_keys = export
        .entries
        .iter()
        .filter(|e| e.key.starts_with("norn."))
        .map(|e| e.key.as_str())
        .collect::<HashSet<_>>();
    let mut records = BTreeMap::new();
    let mut rejected = Vec::new();
    for (index, entry) in export.entries.iter().enumerate() {
        let canonical = entry
            .key
            .replace("lachesi.drafts.", "norn.drafts.v1.")
            .replace("lachesi.reviewReferences.", "norn.reviewReferences.v1.")
            .replace("lachesi.viewedFiles.", "norn.viewedFiles.v1.")
            .replace("lachesi.reviewPrompt.", "norn.reviewPrompt.v1.")
            .replace("lachesi.theme", "norn.theme.v1");
        if entry.key.starts_with("lachesi.") && canonical_keys.contains(canonical.as_str()) {
            continue;
        }
        match normalize(entry, provider) {
            Ok(record) => {
                if let Err(reason) = record.validate() {
                    rejected.push(ImportRejection {
                        entry: index,
                        reason,
                    });
                    continue;
                }
                if records.contains_key(&record.key) {
                    rejected.push(ImportRejection {
                        entry: index,
                        reason: "Duplicate input scope; first valid entry retained.".into(),
                    });
                } else {
                    records.insert(record.key.clone(), record);
                }
            }
            Err(reason) => rejected.push(ImportRejection {
                entry: index,
                reason,
            }),
        }
    }
    Ok((records.into_values().collect(), rejected))
}
fn preview_at(
    input: &Path,
    destination: &Path,
    provider: Option<ReviewProvider>,
) -> Result<
    (
        ImportPreview,
        Vec<WorkspaceRecord>,
        WorkspaceDocument,
        Vec<u8>,
    ),
    String,
> {
    let bytes = read_bounded(input, false)?;
    let (records, rejected) = input_records(&bytes, provider)?;
    let (document, version, original) = read_destination(destination)?;
    let existing = document
        .records
        .iter()
        .map(|r| (&r.key, r))
        .collect::<BTreeMap<_, _>>();
    let mut preview = ImportPreview {
        input_version: {
            let mut hash = Sha256::new();
            hash.update(&bytes);
            hash.update([0]);
            hash.update(provider.map(provider_name).unwrap_or("unset").as_bytes());
            hex::encode(hash.finalize())
        },
        destination_version: version,
        additions_count: 0,
        unchanged_count: 0,
        conflicts_count: 0,
        rejected_count: 0,
        details_truncated: false,
        additions: Vec::new(),
        unchanged: Vec::new(),
        conflicts: Vec::new(),
        rejected,
    };
    for record in &records {
        match existing.get(&record.key) {
            None => preview.additions.push(record.key.clone()),
            Some(old) if **old == *record => preview.unchanged.push(record.key.clone()),
            Some(_) => preview.conflicts.push(record.key.clone()),
        }
    }
    preview.additions_count = preview.additions.len();
    preview.unchanged_count = preview.unchanged.len();
    preview.conflicts_count = preview.conflicts.len();
    preview.rejected_count = preview.rejected.len();
    preview.details_truncated = [
        preview.additions_count,
        preview.unchanged_count,
        preview.conflicts_count,
        preview.rejected_count,
    ]
    .into_iter()
    .any(|count| count > 200);
    preview.additions.truncate(200);
    preview.unchanged.truncate(200);
    preview.conflicts.truncate(200);
    preview.rejected.truncate(200);
    Ok((preview, records, document, original))
}
fn atomic_write(path: &Path, bytes: &[u8], no_clobber: bool) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err("Canonical user data would exceed 16 MiB; no data was written.".into());
    }
    let parent = path.parent().ok_or("User-data path has no parent.")?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create user-data directory.")?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cannot stage user data.")?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| "Cannot flush user data.")?;
    if no_clobber {
        file.persist_noclobber(path)
            .map_err(|_| "User-data backup already exists or cannot be written.")?;
    } else {
        file.persist(path)
            .map_err(|_| "Cannot replace user data; original retained.")?;
    }
    Ok(())
}
fn data_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
pub(crate) fn canonical_path() -> Result<PathBuf, String> {
    let root = std::env::var_os("NORN_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|p| p.join("norn")))
        .ok_or("Cannot determine local user-data directory.")?;
    Ok(root.join("workspace-data-v1.json"))
}
pub(crate) fn preview(
    input: &Path,
    provider: Option<ReviewProvider>,
) -> Result<ImportPreview, String> {
    let _guard = data_lock()
        .lock()
        .map_err(|_| "User data is unavailable.")?;
    Ok(preview_at(input, &canonical_path()?, provider)?.0)
}
pub(crate) fn import(
    input: &Path,
    provider: Option<ReviewProvider>,
    input_version: &str,
    destination_version: &str,
) -> Result<ImportResult, String> {
    let _guard = data_lock()
        .lock()
        .map_err(|_| "User data is unavailable.")?;
    import_at(
        input,
        &canonical_path()?,
        provider,
        input_version,
        destination_version,
    )
}
fn import_at(
    input: &Path,
    destination: &Path,
    provider: Option<ReviewProvider>,
    input_version: &str,
    destination_version: &str,
) -> Result<ImportResult, String> {
    let _file_guard = crate::runtime_identity::acquire_storage_lock(destination)?;
    let (preview, records, mut document, original) = preview_at(input, destination, provider)?;
    if preview.input_version != input_version || preview.destination_version != destination_version
    {
        return Err(
            "Import source or destination changed; preview again before confirming.".into(),
        );
    }
    let existing = document
        .records
        .iter()
        .map(|r| r.key.clone())
        .collect::<HashSet<_>>();
    document
        .records
        .extend(records.into_iter().filter(|r| !existing.contains(&r.key)));
    document.records.sort_by(|a, b| a.key.cmp(&b.key));
    validate_document(&document)?;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|_| "Cannot encode user data.")?;
    let backup = if preview.additions_count == 0 {
        None
    } else {
        let path = destination.with_file_name(format!(
            "workspace-data-before-{}.json",
            preview.destination_version
        ));
        if !path.exists() {
            atomic_write(
                &path,
                if original.is_empty() {
                    b"{\"schemaVersion\":1,\"records\":[]}"
                } else {
                    &original
                },
                true,
            )?;
        } else if read_bounded(&path, false)?
            != if original.is_empty() {
                b"{\"schemaVersion\":1,\"records\":[]}".to_vec()
            } else {
                original.clone()
            }
        {
            return Err("Existing backup does not match; preserve it before retrying.".into());
        }
        atomic_write(destination, &bytes, false)?;
        Some(path.to_string_lossy().into_owned())
    };
    Ok(ImportResult {
        imported: preview.additions_count,
        unchanged: preview.unchanged_count,
        conflicts: preview.conflicts_count,
        rejected_count: preview.rejected_count,
        details_truncated: preview.details_truncated,
        rejected: preview.rejected,
        backup_path: backup,
        destination_version: if preview.additions_count == 0 {
            preview.destination_version
        } else {
            digest(&bytes)
        },
    })
}
pub(crate) fn export(destination: &Path) -> Result<usize, String> {
    let _guard = data_lock()
        .lock()
        .map_err(|_| "User data is unavailable.")?;
    let (document, _, _) = read_destination(&canonical_path()?)?;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|_| "Cannot encode user data.")?;
    atomic_write(destination, &bytes, true)?;
    Ok(document.records.len())
}

pub(crate) fn run_cli(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    const USAGE:&str="Usage:\n  norn data preview <export.json> [--provider github|bitbucket]\n  norn data import <export.json> [--provider github|bitbucket] --input-version <sha256> --destination-version <sha256> --confirm\n  norn data export <new-file.json>\n\nPreview first. Import keeps conflicting existing scopes and reports rejected entries. Source files are never removed. Export refuses to replace an existing file.";
    if args.is_empty() || args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        let _ = writeln!(stdout, "{USAGE}");
        return 0;
    }
    let result = (|| -> Result<serde_json::Value, String> {
        let action = args.first().ok_or("Missing data action.")?;
        let path = args
            .get(1)
            .filter(|p| !p.starts_with('-'))
            .ok_or("Missing user-data file path.")?;
        let mut provider = None;
        let mut input_version = None;
        let mut destination_version = None;
        let mut confirm = false;
        let mut index = 2;
        while index < args.len() {
            match args[index].as_str() {
                "--provider" => {
                    index += 1;
                    provider = Some(match args.get(index).map(String::as_str) {
                        Some("github") => ReviewProvider::Github,
                        Some("bitbucket") => ReviewProvider::Bitbucket,
                        _ => {
                            return Err(
                                "Choose github or bitbucket as the historical provider.".into()
                            )
                        }
                    });
                }
                "--input-version" | "--destination-version" => {
                    let input = args[index] == "--input-version";
                    index += 1;
                    let version = args
                        .get(index)
                        .filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
                        .ok_or("Import versions must be SHA-256 values from preview.")?
                        .clone();
                    if input {
                        input_version = Some(version);
                    } else {
                        destination_version = Some(version);
                    }
                }
                "--confirm" => confirm = true,
                _ => return Err("Unsupported user-data argument.".into()),
            }
            index += 1;
        }
        match action.as_str() {
            "preview" if !confirm && input_version.is_none() && destination_version.is_none() => {
                serde_json::to_value(preview(Path::new(path), provider)?)
                    .map_err(|_| "Cannot encode preview.".into())
            }
            "import" if confirm => serde_json::to_value(import(
                Path::new(path),
                provider,
                input_version
                    .as_deref()
                    .ok_or("Preview and provide its input version before importing.")?,
                destination_version
                    .as_deref()
                    .ok_or("Preview and provide its destination version before importing.")?,
            )?)
            .map_err(|_| "Cannot encode import result.".into()),
            "export" if args.len() == 2 => {
                Ok(serde_json::json!({"exported":export(Path::new(path))?}))
            }
            _ => Err("Unsupported data action or missing explicit confirmation.".into()),
        }
    })();
    match result {
        Ok(value) => {
            let _ = writeln!(
                stdout,
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );
            if value
                .get("rejected")
                .and_then(|v| v.as_array())
                .is_some_and(|items| !items.is_empty())
                || value.get("conflicts").is_some_and(|v| {
                    v.as_u64().is_some_and(|n| n > 0) || v.as_array().is_some_and(|a| !a.is_empty())
                })
            {
                3
            } else {
                0
            }
        }
        Err(error) => {
            let _ = writeln!(stderr, "{error}\n{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write_export(path: &Path, entries: Vec<(&str, &str)>) {
        fs::write(
            path,
            serde_json::to_vec(&DesktopExport {
                schema_version: 1,
                source: "desktopLocalStorage".into(),
                entries: entries
                    .into_iter()
                    .map(|(key, value)| DesktopEntry {
                        key: key.into(),
                        value: value.into(),
                    })
                    .collect(),
            })
            .unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn preview_import_backup_repeat_and_conflict_preserve_both_sources() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.json");
        let destination = temp.path().join("workspace-data-v1.json");
        write_export(
            &source,
            vec![
                ("norn.reviewPrompt.v1.owner/repo", "original custom prompt"),
                ("lachesi.reviewPrompt.owner/repo", "legacy prompt"),
                ("norn.viewedFiles.v1.owner/repo#7", "[\"src/a.rs\"]"),
            ],
        );
        let before = fs::read(&source).unwrap();
        let (preview, _, _, _) =
            preview_at(&source, &destination, Some(ReviewProvider::Github)).unwrap();
        assert_eq!(preview.additions.len(), 2);
        assert!(preview.rejected.is_empty());
        assert!(!destination.exists());
        let imported = import_at(
            &source,
            &destination,
            Some(ReviewProvider::Github),
            &preview.input_version,
            &preview.destination_version,
        )
        .unwrap();
        assert_eq!(imported.imported, 2);
        assert!(Path::new(imported.backup_path.as_ref().unwrap()).exists());
        assert_eq!(fs::read(&source).unwrap(), before);
        let canonical = fs::read(&destination).unwrap();
        let (repeat, _, _, _) =
            preview_at(&source, &destination, Some(ReviewProvider::Github)).unwrap();
        let result = import_at(
            &source,
            &destination,
            Some(ReviewProvider::Github),
            &repeat.input_version,
            &repeat.destination_version,
        )
        .unwrap();
        assert_eq!(result.imported, 0);
        assert_eq!(result.unchanged, 2);
        assert!(result.backup_path.is_none());
        assert_eq!(fs::read(&destination).unwrap(), canonical);
        write_export(
            &source,
            vec![(
                "norn.reviewPrompt.v1.owner/repo",
                "older conflicting prompt",
            )],
        );
        let (conflict, _, _, _) =
            preview_at(&source, &destination, Some(ReviewProvider::Github)).unwrap();
        let result = import_at(
            &source,
            &destination,
            Some(ReviewProvider::Github),
            &conflict.input_version,
            &conflict.destination_version,
        )
        .unwrap();
        assert_eq!(result.conflicts, 1);
        assert_eq!(fs::read(&destination).unwrap(), canonical);
    }
    #[test]
    fn stale_approval_corrupt_destination_oversized_source_are_non_destructive() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.json");
        let destination = temp.path().join("data.json");
        write_export(&source, vec![("norn.reviewPrompt.v1.owner/repo", "first")]);
        let (preview, _, _, _) =
            preview_at(&source, &destination, Some(ReviewProvider::Github)).unwrap();
        write_export(
            &source,
            vec![("norn.reviewPrompt.v1.owner/repo", "changed")],
        );
        assert!(import_at(
            &source,
            &destination,
            Some(ReviewProvider::Github),
            &preview.input_version,
            &preview.destination_version
        )
        .is_err());
        assert!(!destination.exists());
        fs::write(&destination, b"broken").unwrap();
        assert!(preview_at(&source, &destination, Some(ReviewProvider::Github)).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"broken");
        fs::write(&source, vec![b' '; MAX_BYTES + 1]).unwrap();
        assert!(preview_at(&source, &destination, Some(ReviewProvider::Github)).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"broken");
    }
    #[test]
    fn partial_errors_and_ambiguous_provider_are_reported_without_raw_values() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.json");
        let destination = temp.path().join("data.json");
        write_export(
            &source,
            vec![
                ("norn.reviewPrompt.v1.owner/repo", "private body"),
                ("norn.theme.v1", "dark"),
                ("credential.token", "sensitive-value"),
            ],
        );
        let (preview, _, _, _) = preview_at(&source, &destination, None).unwrap();
        assert_eq!(preview.additions, vec!["theme"]);
        assert_eq!(preview.rejected.len(), 2);
        let encoded = serde_json::to_string(&preview).unwrap();
        assert!(!encoded.contains("private body"));
        assert!(!encoded.contains("sensitive-value"));
    }
    #[test]
    fn legacy_drafts_preserve_publication_and_numeric_parent_identity() {
        let entry=DesktopEntry{key:"lachesi.drafts.bitbucket:owner/repo/7".into(),value:r#"[{"localId":"draft-1","prId":7,"path":"a.rs","to":4,"from":null,"raw":"body","parentId":123,"createdAt":100,"source":"aiFinding","findingRef":{"reviewRunId":"run","findingId":"finding","findingFingerprint":"fp"},"publicationMode":"inline","reviewBaseSha":"base","reviewHeadSha":"head"}]"#.into()};
        let record = normalize(&entry, None).unwrap();
        let WorkspaceValue::Drafts(items) = record.payload else {
            panic!()
        };
        assert_eq!(items[0].parent_id.as_deref(), Some("123"));
        assert_eq!(items[0].finding_ref.as_ref().unwrap().finding_id, "finding");
        assert_eq!(items[0].review_head_sha.as_deref(), Some("head"));
    }
    #[test]
    fn approval_binds_provider_and_bounded_details_keep_complete_counts() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.json");
        let destination = temp.path().join("data.json");
        write_export(&source, vec![("norn.reviewPrompt.v1.owner/repo", "custom")]);
        let (preview, _, _, _) =
            preview_at(&source, &destination, Some(ReviewProvider::Github)).unwrap();
        assert!(import_at(
            &source,
            &destination,
            Some(ReviewProvider::Bitbucket),
            &preview.input_version,
            &preview.destination_version
        )
        .is_err());
        assert!(!destination.exists());
        let document = WorkspaceDocument {
            schema_version: 1,
            records: (0..501)
                .map(|i| WorkspaceRecord {
                    key: format!("prompt.github:owner/repo{i}"),
                    payload: WorkspaceValue::Prompt("policy".into()),
                })
                .collect(),
        };
        fs::write(&source, serde_json::to_vec(&document).unwrap()).unwrap();
        let (preview, _, _, _) = preview_at(&source, &destination, None).unwrap();
        assert_eq!(preview.additions_count, 501);
        assert_eq!(preview.additions.len(), 200);
        assert!(preview.details_truncated);
        let result = import_at(
            &source,
            &destination,
            None,
            &preview.input_version,
            &preview.destination_version,
        )
        .unwrap();
        assert_eq!(result.imported, 501);
        let (doc, _, _) = read_destination(&destination).unwrap();
        assert_eq!(doc.records.len(), 501);
    }
    #[test]
    fn interrupted_stage_and_bad_backup_never_replace_existing_user_data() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.json");
        let destination = temp.path().join("workspace-data-v1.json");
        let document = WorkspaceDocument {
            schema_version: 1,
            records: vec![WorkspaceRecord {
                key: "theme".into(),
                payload: WorkspaceValue::Theme("dark".into()),
            }],
        };
        let original = serde_json::to_vec(&document).unwrap();
        fs::write(&destination, &original).unwrap();
        // A process interrupted before persist leaves an unrelated staging file.
        fs::write(temp.path().join(".tmp-interrupted"), b"{partial").unwrap();
        write_export(&source, vec![("norn.reviewPrompt.v1.owner/repo", "policy")]);
        let (preview, _, _, _) =
            preview_at(&source, &destination, Some(ReviewProvider::Github)).unwrap();
        assert_eq!(preview.additions_count, 1);
        assert_eq!(fs::read(&destination).unwrap(), original);
        let backup = destination.with_file_name(format!(
            "workspace-data-before-{}.json",
            preview.destination_version
        ));
        fs::write(&backup, b"unrelated backup").unwrap();
        assert!(import_at(
            &source,
            &destination,
            Some(ReviewProvider::Github),
            &preview.input_version,
            &preview.destination_version
        )
        .is_err());
        assert_eq!(fs::read(&destination).unwrap(), original);
        assert_eq!(fs::read(&backup).unwrap(), b"unrelated backup");
        fs::write(&destination, b"").unwrap();
        assert!(preview_at(&source, &destination, Some(ReviewProvider::Github)).is_err());
        assert!(fs::read(&destination).unwrap().is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_sources_and_destinations_are_refused() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.json");
        let destination = temp.path().join("data.json");
        write_export(&source, vec![("norn.theme.v1", "dark")]);
        let link = temp.path().join("link.json");
        symlink(&source, &link).unwrap();
        assert!(preview_at(&link, &destination, None).is_err());
        symlink(&source, &destination).unwrap();
        assert!(preview_at(&source, &destination, None).is_err());
        assert!(fs::symlink_metadata(&destination)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}

#[path = "workspace_records.rs"]
mod records;
pub(crate) use records::{
    draft_snapshot, prompt_for_scope, record_mutate, record_read, references_for_scope,
    remove_acknowledged_draft, with_locked_drafts,
};
pub use records::{RecordMutateParams, RecordMutation, RecordReadParams};

#[path = "workspace_scopes.rs"]
mod scopes;
pub(crate) use scopes::scope_read;
pub use scopes::ScopeReadParams;
