//! Bounded immutable read snapshots and per-record compare-and-swap updates.
use super::*;
use std::collections::VecDeque;
use std::sync::Arc;

const PAGE_BYTES: usize = 256 * 1024;
const CACHE_BYTES: usize = 32 * 1024 * 1024;
const CACHE_ENTRIES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordReadParams {
    pub key: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub record_version: Option<String>,
}
fn default_limit() -> usize {
    100
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPage {
    pub key: String,
    pub record_version: String,
    pub available: bool,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub total_items: usize,
    pub payload: WorkspaceValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RecordMutation {
    SetPrompt {
        value: String,
    },
    SetTheme {
        value: String,
    },
    AddViewed {
        path: String,
    },
    RemoveViewed {
        path: String,
    },
    PutReference {
        reference: Box<ReviewReference>,
    },
    RemoveReference {
        id: String,
    },
    PutDraft {
        draft: Box<DraftComment>,
    },
    RemoveDraft {
        #[serde(rename = "localId")]
        local_id: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordMutateParams {
    pub key: String,
    pub expected_version: String,
    pub mutation: RecordMutation,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordMutationResult {
    pub key: String,
    pub record_version: String,
    pub changed: bool,
}

struct Snapshot {
    path: PathBuf,
    record: WorkspaceRecord,
    version: String,
    available: bool,
    bytes: usize,
}
#[derive(Default)]
struct SnapshotCache {
    entries: VecDeque<Arc<Snapshot>>,
}
fn cache() -> &'static Mutex<SnapshotCache> {
    static CACHE: OnceLock<Mutex<SnapshotCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(SnapshotCache::default()))
}

fn empty_record(key: &str) -> Result<WorkspaceRecord, String> {
    let payload = if key == "theme" {
        WorkspaceValue::Theme("dark".into())
    } else {
        match key.split_once('.').map(|pair| pair.0) {
            Some("drafts") => WorkspaceValue::Drafts(vec![]),
            Some("references") => WorkspaceValue::References(vec![]),
            Some("viewed") => WorkspaceValue::Viewed(vec![]),
            Some("prompt") => WorkspaceValue::Prompt(String::new()),
            _ => return Err("Unsupported user-data record kind.".into()),
        }
    };
    let record = WorkspaceRecord {
        key: key.into(),
        payload,
    };
    record.validate()?;
    Ok(record)
}
fn version(record: Option<&WorkspaceRecord>) -> Result<String, String> {
    match record {
        None => Ok(digest(&[])),
        Some(record) => serde_json::to_vec(record)
            .map(|bytes| digest(&bytes))
            .map_err(|_| "Cannot encode user-data record.".into()),
    }
}
fn hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn read_at(
    path: &Path,
    params: RecordReadParams,
    snapshots: &Mutex<SnapshotCache>,
) -> Result<RecordPage, String> {
    empty_record(&params.key)?;
    if params.limit == 0
        || params.limit > 100
        || params.offset > 10_000
        || params.record_version.as_deref().is_some_and(|v| !hash(v))
        || (params.offset > 0 && params.record_version.is_none())
    {
        return Err("Invalid user-data page parameters.".into());
    }
    let snapshot = if let Some(expected) = &params.record_version {
        let entries = snapshots
            .lock()
            .map_err(|_| "User-data snapshots are unavailable.")?;
        entries
            .entries
            .iter()
            .find(|entry| {
                entry.path == path && entry.record.key == params.key && &entry.version == expected
            })
            .cloned()
            .ok_or("User-data read snapshot expired; refresh this record.")?
    } else {
        let _guard = data_lock()
            .lock()
            .map_err(|_| "User data is unavailable.")?;
        let (document, _, _) = read_destination(path)?;
        let record = document
            .records
            .into_iter()
            .find(|record| record.key == params.key);
        let available = record.is_some();
        let version = version(record.as_ref())?;
        let record = record.unwrap_or(empty_record(&params.key)?);
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| "Cannot encode user-data record.")?
            .len();
        if bytes > CACHE_BYTES {
            return Err("User-data record exceeds the snapshot limit; preserve its export.".into());
        }
        let snapshot = Arc::new(Snapshot {
            path: path.into(),
            record,
            version,
            available,
            bytes,
        });
        let mut cache = snapshots
            .lock()
            .map_err(|_| "User-data snapshots are unavailable.")?;
        cache.entries.retain(|entry| {
            !(entry.path == path
                && entry.record.key == params.key
                && entry.version == snapshot.version)
        });
        while cache.entries.len() >= CACHE_ENTRIES
            || cache.entries.iter().map(|entry| entry.bytes).sum::<usize>() + bytes > CACHE_BYTES
        {
            cache.entries.pop_front();
        }
        cache.entries.push_back(snapshot.clone());
        snapshot
    };
    let total = if !snapshot.available {
        0
    } else {
        match &snapshot.record.payload {
            WorkspaceValue::Drafts(items) => items.len(),
            WorkspaceValue::References(items) => items.len(),
            WorkspaceValue::Viewed(items) => items.len(),
            _ => 1,
        }
    };
    if (matches!(
        snapshot.record.payload,
        WorkspaceValue::Prompt(_) | WorkspaceValue::Theme(_)
    ) && params.offset != 0)
        || params.offset > total
    {
        return Err("User-data page offset is outside the record.".into());
    }
    let requested_end = (params.offset + params.limit).min(total);
    let end = match &snapshot.record.payload {
        WorkspaceValue::Drafts(items) => page_end(items, params.offset, requested_end)?,
        WorkspaceValue::References(items) => page_end(items, params.offset, requested_end)?,
        WorkspaceValue::Viewed(items) => page_end(items, params.offset, requested_end)?,
        _ => requested_end,
    };
    let payload = match &snapshot.record.payload {
        WorkspaceValue::Drafts(items) => WorkspaceValue::Drafts(items[params.offset..end].to_vec()),
        WorkspaceValue::References(items) => {
            WorkspaceValue::References(items[params.offset..end].to_vec())
        }
        WorkspaceValue::Viewed(items) => WorkspaceValue::Viewed(items[params.offset..end].to_vec()),
        scalar => scalar.clone(),
    };
    let page = RecordPage {
        key: params.key,
        record_version: snapshot.version.clone(),
        available: snapshot.available,
        offset: params.offset,
        next_offset: (end < total).then_some(end),
        total_items: total,
        payload,
    };
    if serde_json::to_vec(&page)
        .map_err(|_| "Cannot encode user-data page.")?
        .len()
        > PAGE_BYTES
    {
        return Err("A user-data item exceeds its page limit; preserve its export.".into());
    }
    Ok(page)
}

fn page_end<T: Serialize>(items: &[T], start: usize, end: usize) -> Result<usize, String> {
    let mut bytes = 0;
    for (index, item) in items.iter().enumerate().take(end).skip(start) {
        let size = serde_json::to_vec(item)
            .map_err(|_| "Cannot encode user-data item.")?
            .len()
            + 1;
        if bytes + size > PAGE_BYTES - 8192 {
            if index == start {
                return Err("A user-data item exceeds its page limit; preserve its export.".into());
            }
            return Ok(index);
        }
        bytes += size;
    }
    Ok(end)
}

fn mutate_at(path: &Path, params: RecordMutateParams) -> Result<RecordMutationResult, String> {
    let empty = empty_record(&params.key)?;
    if !hash(&params.expected_version) {
        return Err("Invalid user-data record version.".into());
    }
    let _guard = data_lock()
        .lock()
        .map_err(|_| "User data is unavailable.")?;
    let _file_guard = crate::runtime_identity::acquire_storage_lock(path)?;
    let (mut document, _, _) = read_destination(path)?;
    let index = document
        .records
        .iter()
        .position(|record| record.key == params.key);
    let old = index.map(|index| &document.records[index]);
    if version(old)? != params.expected_version {
        return Err("User-data record changed; refresh before saving your edit.".into());
    }
    let mut record = old.cloned().unwrap_or(empty);
    match (&mut record.payload, params.mutation) {
        (WorkspaceValue::Prompt(value), RecordMutation::SetPrompt { value: new })
        | (WorkspaceValue::Theme(value), RecordMutation::SetTheme { value: new }) => *value = new,
        (WorkspaceValue::Viewed(items), RecordMutation::AddViewed { path }) => {
            bounded_text(&path, 4096, false)?;
            if path.trim().is_empty() {
                return Err("Viewed path must not be empty.".into());
            }
            if !items.contains(&path) {
                items.push(path);
            }
        }
        (WorkspaceValue::Viewed(items), RecordMutation::RemoveViewed { path }) => {
            bounded_text(&path, 4096, false)?;
            if path.trim().is_empty() {
                return Err("Viewed path must not be empty.".into());
            }
            items.retain(|item| item != &path);
        }
        (WorkspaceValue::References(items), RecordMutation::PutReference { reference }) => {
            if let Some(item) = items.iter_mut().find(|item| item.id == reference.id) {
                *item = *reference;
            } else {
                items.push(*reference);
            }
        }
        (WorkspaceValue::References(items), RecordMutation::RemoveReference { id }) => {
            bounded_text(&id, 256, false)?;
            if id.trim().is_empty() {
                return Err("Reference id must not be empty.".into());
            }
            items.retain(|item| item.id != id);
        }
        (WorkspaceValue::Drafts(items), RecordMutation::PutDraft { draft }) => {
            if let Some(item) = items
                .iter_mut()
                .find(|item| item.local_id == draft.local_id)
            {
                *item = *draft;
            } else {
                items.push(*draft);
            }
        }
        (WorkspaceValue::Drafts(items), RecordMutation::RemoveDraft { local_id }) => {
            bounded_text(&local_id, 256, false)?;
            if local_id.trim().is_empty() {
                return Err("Draft id must not be empty.".into());
            }
            items.retain(|item| item.local_id != local_id);
        }
        _ => return Err("User-data mutation does not match its record kind.".into()),
    }
    record.validate()?;
    let changed = old != Some(&record);
    let record_version = version(Some(&record))?;
    if changed {
        if let Some(index) = index {
            document.records[index] = record;
        } else {
            document.records.push(record);
        }
        validate_document(&document)?;
        let bytes =
            serde_json::to_vec_pretty(&document).map_err(|_| "Cannot encode stored user data.")?;
        atomic_write(path, &bytes, false)?;
    }
    Ok(RecordMutationResult {
        key: params.key,
        record_version,
        changed,
    })
}

pub(crate) fn record_read(params: RecordReadParams) -> Result<RecordPage, String> {
    read_at(&canonical_path()?, params, cache())
}
pub(crate) fn record_mutate(params: RecordMutateParams) -> Result<RecordMutationResult, String> {
    mutate_at(&canonical_path()?, params)
}
pub(crate) fn prompt_for_scope(
    provider: ReviewProvider,
    workspace: &str,
    repo: &str,
) -> Result<Option<String>, String> {
    let key = format!("prompt.{}:{workspace}/{repo}", provider_name(provider));
    empty_record(&key)?;
    let _guard = data_lock()
        .lock()
        .map_err(|_| "User data is unavailable.")?;
    let (document, _, _) = read_destination(&canonical_path()?)?;
    Ok(document.records.into_iter().find_map(|record| {
        if record.key == key {
            if let WorkspaceValue::Prompt(prompt) = record.payload {
                Some(prompt)
            } else {
                None
            }
        } else {
            None
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(key: &str) -> RecordReadParams {
        RecordReadParams {
            key: key.into(),
            offset: 0,
            limit: 100,
            record_version: None,
        }
    }
    fn write(path: &Path, records: Vec<WorkspaceRecord>) {
        fs::write(
            path,
            serde_json::to_vec_pretty(&WorkspaceDocument {
                schema_version: 1,
                records,
            })
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn record_cas_preserves_other_scopes_and_rejects_stale_or_invalid_writes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("records.json");
        let cache = Mutex::new(SnapshotCache::default());
        let key = "prompt.github:acme/repo";
        let absent = read_at(&path, params(key), &cache).unwrap();
        assert!(!absent.available);
        assert!(!path.exists());
        let saved = mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: absent.record_version.clone(),
                mutation: RecordMutation::SetPrompt {
                    value: "Review security ✓".into(),
                },
            },
        )
        .unwrap();
        assert!(saved.changed);
        let other = "prompt.bitbucket:acme/repo";
        mutate_at(
            &path,
            RecordMutateParams {
                key: other.into(),
                expected_version: version(None).unwrap(),
                mutation: RecordMutation::SetPrompt {
                    value: "Keep other provider".into(),
                },
            },
        )
        .unwrap();
        let unchanged = mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: saved.record_version.clone(),
                mutation: RecordMutation::SetPrompt {
                    value: "Review security ✓".into(),
                },
            },
        )
        .unwrap();
        assert!(!unchanged.changed);
        let before = fs::read(&path).unwrap();
        assert!(mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: absent.record_version,
                mutation: RecordMutation::SetPrompt {
                    value: "stale".into()
                }
            }
        )
        .unwrap_err()
        .contains("changed"));
        assert!(mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: saved.record_version,
                mutation: RecordMutation::AddViewed {
                    path: "file".into()
                }
            }
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        let page = read_at(&path, params(other), &cache).unwrap();
        assert_eq!(
            page.payload,
            WorkspaceValue::Prompt("Keep other provider".into())
        );
    }

    #[test]
    fn large_imported_references_page_within_frame_and_pin_a_consistent_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("records.json");
        let cache = Mutex::new(SnapshotCache::default());
        let key = "references.github:acme/repo/7";
        let items = (0..100)
            .map(|index| ReviewReference {
                id: format!("ref-{index}"),
                r#type: "note".into(),
                source: "manual".into(),
                title: Some(format!("Reference {index}")),
                url: None,
                key: None,
                workspace: None,
                repo: None,
                pr_id: None,
                local_path: None,
                body: Some("\n".repeat(65_536)),
                created_at: 1,
                updated_at: 2,
            })
            .collect::<Vec<_>>();
        write(
            &path,
            vec![WorkspaceRecord {
                key: key.into(),
                payload: WorkspaceValue::References(items.clone()),
            }],
        );
        let first = read_at(&path, params(key), &cache).unwrap();
        assert_eq!(first.total_items, 100);
        assert!(first.next_offset.is_some());
        let version = first.record_version.clone();
        let mut updated = items[0].clone();
        updated.body = Some("Newer saved content".into());
        mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: version.clone(),
                mutation: RecordMutation::PutReference {
                    reference: Box::new(updated),
                },
            },
        )
        .unwrap();
        let mut page = first;
        let mut restored = vec![];
        loop {
            assert!(serde_json::to_vec(&page).unwrap().len() <= PAGE_BYTES);
            assert_eq!(page.record_version, version);
            if let WorkspaceValue::References(items) = page.payload {
                restored.extend(items);
            } else {
                panic!("wrong page kind");
            }
            let Some(offset) = page.next_offset else {
                break;
            };
            page = read_at(
                &path,
                RecordReadParams {
                    offset,
                    record_version: Some(version.clone()),
                    ..params(key)
                },
                &cache,
            )
            .unwrap();
        }
        assert_eq!(restored, items);
        assert!(mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: version,
                mutation: RecordMutation::RemoveReference { id: "ref-1".into() }
            }
        )
        .unwrap_err()
        .contains("changed"));
        let current = read_at(&path, params(key), &cache).unwrap();
        if let WorkspaceValue::References(items) = current.payload {
            assert_eq!(items[0].body.as_deref(), Some("Newer saved content"));
        } else {
            panic!("wrong record");
        }
    }

    #[test]
    fn draft_mutations_preserve_publication_identity_and_reject_wrong_pr() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("records.json");
        let cache = Mutex::new(SnapshotCache::default());
        let key = "drafts.github:acme/repo/7";
        let draft: DraftComment = serde_json::from_value(serde_json::json!({
            "localId":"draft-1", "prId":7, "path":"src/lib.rs", "to":12, "from":null,
            "raw":"Finding reply ✓", "parentId":"123", "createdAt":1000, "source":"aiFinding",
            "findingRef":{"reviewRunId":"run-1","findingId":"finding-1","findingFingerprint":"fingerprint"},
            "publicationMode":"inline", "reviewBaseSha":"a".repeat(40), "reviewHeadSha":"b".repeat(40)
        })).unwrap();
        let saved = mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: version(None).unwrap(),
                mutation: RecordMutation::PutDraft {
                    draft: Box::new(draft.clone()),
                },
            },
        )
        .unwrap();
        let page = read_at(&path, params(key), &cache).unwrap();
        assert_eq!(
            serde_json::to_value(page.payload).unwrap(),
            serde_json::json!({"kind":"drafts","value":[draft]})
        );
        let bytes = fs::read(&path).unwrap();
        let mut wrong = draft;
        wrong.pr_id = 8;
        assert!(mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: saved.record_version.clone(),
                mutation: RecordMutation::PutDraft {
                    draft: Box::new(wrong)
                }
            }
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        mutate_at(
            &path,
            RecordMutateParams {
                key: key.into(),
                expected_version: saved.record_version,
                mutation: RecordMutation::RemoveDraft {
                    local_id: "draft-1".into(),
                },
            },
        )
        .unwrap();
        assert_eq!(read_at(&path, params(key), &cache).unwrap().total_items, 0);
    }

    #[test]
    fn snapshots_are_bounded_and_expired_offsets_fail_explicitly() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("records.json");
        let cache = Mutex::new(SnapshotCache::default());
        let key = "viewed.github:acme/repo/7";
        write(
            &path,
            vec![WorkspaceRecord {
                key: key.into(),
                payload: WorkspaceValue::Viewed(vec!["a".into(), "b".into()]),
            }],
        );
        let first = read_at(
            &path,
            RecordReadParams {
                limit: 1,
                ..params(key)
            },
            &cache,
        )
        .unwrap();
        for index in 0..5 {
            read_at(
                &path,
                params(&format!("prompt.github:acme/repo-{index}")),
                &cache,
            )
            .unwrap();
        }
        assert!(cache.lock().unwrap().entries.len() <= CACHE_ENTRIES);
        assert!(read_at(
            &path,
            RecordReadParams {
                offset: 1,
                record_version: Some(first.record_version),
                ..params(key)
            },
            &cache
        )
        .unwrap_err()
        .contains("expired"));
        assert!(read_at(
            &path,
            RecordReadParams {
                offset: 1,
                ..params(key)
            },
            &cache
        )
        .is_err());
        assert!(read_at(&path, params("viewed.github:../repo/7"), &cache).is_err());
        fs::write(&path, b"{interrupted").unwrap();
        assert!(read_at(&path, params(key), &cache).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{interrupted");
    }
}
