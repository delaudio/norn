//! Immutable, content-free inventory of canonical PR data (ADR 0020, #355).
use super::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

const PAGE_BYTES: usize = 256 * 1024;
const CACHE_BYTES: usize = 8 * 1024 * 1024;
const CACHE_ENTRIES: usize = 4;
const SNAPSHOT_AGE: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeReadParams {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub inventory_version: Option<String>,
}
fn default_limit() -> usize {
    100
}
impl ScopeReadParams {
    pub fn validate(&self) -> Result<(), String> {
        if self.limit == 0
            || self.limit > 100
            || self.offset > MAX_ENTRIES
            || self.inventory_version.as_deref().is_some_and(|v| {
                v.len() != 64
                    || !v
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            || (self.offset > 0 && self.inventory_version.is_none())
        {
            return Err("Invalid saved-scope page parameters.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceScope {
    pub provider: crate::protocol::ProviderKind,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub draft_count: usize,
    pub reference_count: usize,
    pub viewed_count: usize,
    pub local_only: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopePage {
    pub inventory_version: String,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub total_scopes: usize,
    pub scopes: Vec<WorkspaceScope>,
}
struct Snapshot {
    path: PathBuf,
    version: String,
    scopes: Vec<WorkspaceScope>,
    bytes: usize,
    created: Instant,
}
#[derive(Default)]
struct InventoryCache {
    entries: VecDeque<Arc<Snapshot>>,
}
fn cache() -> &'static Mutex<InventoryCache> {
    static CACHE: OnceLock<Mutex<InventoryCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(InventoryCache::default()))
}

fn inventory(document: WorkspaceDocument) -> Result<Vec<WorkspaceScope>, String> {
    let mut scopes: BTreeMap<(String, String, String, u32), WorkspaceScope> = BTreeMap::new();
    for record in document.records {
        record.validate()?;
        let (drafts, references, viewed) = match &record.payload {
            WorkspaceValue::Drafts(items) => (items.len(), 0, 0),
            WorkspaceValue::References(items) => (0, items.len(), 0),
            WorkspaceValue::Viewed(items) => (0, 0, items.len()),
            _ => continue,
        };
        if drafts + references + viewed == 0 {
            continue;
        }
        let (_, suffix) = record.key.split_once('.').ok_or("Invalid saved scope.")?;
        let (provider, scope) = suffix.split_once(':').ok_or("Invalid saved scope.")?;
        let parts = scope.split('/').collect::<Vec<_>>();
        let [workspace, repo, pr] = parts.as_slice() else {
            return Err("Invalid saved scope.".into());
        };
        let pr_id = pr.parse::<u32>().map_err(|_| "Invalid saved scope.")?;
        if pr_id.to_string() != *pr {
            return Err("Saved scope has an ambiguous pull-request number; export and resolve its key before retrying.".into());
        }
        let provider_kind = match provider {
            "github" => crate::protocol::ProviderKind::Github,
            "bitbucket" => crate::protocol::ProviderKind::Bitbucket,
            _ => return Err("Unsupported saved-scope provider.".into()),
        };
        let entry = scopes
            .entry((provider.into(), (*workspace).into(), (*repo).into(), pr_id))
            .or_insert_with(|| WorkspaceScope {
                provider: provider_kind,
                workspace: (*workspace).into(),
                repo: (*repo).into(),
                pr_id,
                draft_count: 0,
                reference_count: 0,
                viewed_count: 0,
                local_only: crate::local_review::is_local_review_id(u64::from(pr_id)),
            });
        entry.draft_count += drafts;
        entry.reference_count += references;
        entry.viewed_count += viewed;
    }
    Ok(scopes.into_values().collect())
}

fn read_at(
    path: &Path,
    params: ScopeReadParams,
    snapshots: &Mutex<InventoryCache>,
) -> Result<ScopePage, String> {
    params.validate()?;
    let snapshot = if let Some(version) = &params.inventory_version {
        let mut entries = snapshots
            .lock()
            .map_err(|_| "Saved scopes are unavailable.")?;
        entries
            .entries
            .retain(|entry| entry.created.elapsed() < SNAPSHOT_AGE);
        entries
            .entries
            .iter()
            .find(|entry| entry.path == path && &entry.version == version)
            .cloned()
            .ok_or("Saved-scope snapshot expired; refresh the picker.")?
    } else {
        let _guard = data_lock()
            .lock()
            .map_err(|_| "User data is unavailable.")?;
        let (document, version, _) = read_destination(path)?;
        let scopes = inventory(document)?;
        let bytes = serde_json::to_vec(&scopes)
            .map_err(|_| "Cannot encode saved scopes.")?
            .len();
        if bytes > CACHE_BYTES {
            return Err("Saved-scope inventory exceeds its limit.".into());
        }
        let snapshot = Arc::new(Snapshot {
            path: path.into(),
            version,
            scopes,
            bytes,
            created: Instant::now(),
        });
        let mut entries = snapshots
            .lock()
            .map_err(|_| "Saved scopes are unavailable.")?;
        entries.entries.retain(|entry| {
            entry.created.elapsed() < SNAPSHOT_AGE
                && !(entry.path == path && entry.version == snapshot.version)
        });
        while entries.entries.len() >= CACHE_ENTRIES
            || entries
                .entries
                .iter()
                .map(|entry| entry.bytes)
                .sum::<usize>()
                + bytes
                > CACHE_BYTES
        {
            entries.entries.pop_front();
        }
        entries.entries.push_back(snapshot.clone());
        snapshot
    };
    if params.offset > snapshot.scopes.len() {
        return Err("Saved-scope cursor is outside its snapshot.".into());
    }
    let mut page = ScopePage {
        inventory_version: snapshot.version.clone(),
        offset: params.offset,
        next_offset: None,
        total_scopes: snapshot.scopes.len(),
        scopes: Vec::new(),
    };
    // Count each scope once; serializing the growing page after each push would
    // make a maximum-size inventory perform quadratic encoding work per page.
    let mut page_bytes = serde_json::to_vec(&page)
        .map_err(|_| "Cannot encode saved-scope page.")?
        .len();
    for scope in snapshot
        .scopes
        .iter()
        .skip(params.offset)
        .take(params.limit)
    {
        let scope_bytes = serde_json::to_vec(scope)
            .map_err(|_| "Cannot encode saved-scope page.")?
            .len()
            + usize::from(!page.scopes.is_empty());
        if page_bytes + scope_bytes > PAGE_BYTES - 64 {
            break;
        }
        page_bytes += scope_bytes;
        page.scopes.push(scope.clone());
    }
    let next = params.offset + page.scopes.len();
    if next < snapshot.scopes.len() {
        if next == params.offset {
            return Err("Saved scope exceeds the page limit.".into());
        }
        page.next_offset = Some(next);
    }
    Ok(page)
}
pub(crate) fn scope_read(params: ScopeReadParams) -> Result<ScopePage, String> {
    read_at(&canonical_path()?, params, cache())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn viewed(key: &str) -> WorkspaceRecord {
        WorkspaceRecord {
            key: key.into(),
            payload: WorkspaceValue::Viewed(vec!["private/path.rs".into()]),
        }
    }
    fn write(path: &Path, records: Vec<WorkspaceRecord>) {
        fs::write(
            path,
            serde_json::to_vec(&WorkspaceDocument {
                schema_version: 1,
                records,
            })
            .unwrap(),
        )
        .unwrap();
    }
    fn params(offset: usize, version: Option<String>) -> ScopeReadParams {
        ScopeReadParams {
            offset,
            limit: 1,
            inventory_version: version,
        }
    }
    #[test]
    fn scopes_preserve_provider_case_and_local_identity_without_contents() {
        let scopes = inventory(WorkspaceDocument {
            schema_version: 1,
            records: vec![
                viewed("viewed.github:acme/repo/7"),
                viewed("viewed.bitbucket:acme/repo/7"),
                viewed("viewed.github:Acme/repo/7"),
                viewed("viewed.github:acme/repo/2147483649"),
                WorkspaceRecord {
                    key: "prompt.github:acme/repo".into(),
                    payload: WorkspaceValue::Prompt("private prompt".into()),
                },
                WorkspaceRecord {
                    key: "drafts.github:empty/repo/8".into(),
                    payload: WorkspaceValue::Drafts(vec![]),
                },
            ],
        })
        .unwrap();
        assert_eq!(scopes.len(), 4);
        assert_eq!(scopes.iter().filter(|scope| scope.local_only).count(), 1);
        let wire = serde_json::to_string(&scopes).unwrap();
        assert!(!wire.contains("private"));
        assert!(!wire.contains("empty"));
    }
    #[test]
    fn pages_are_immutable_across_edits_and_restart_requires_refresh() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.json");
        write(
            &path,
            vec![
                viewed("viewed.github:a/repo/1"),
                viewed("viewed.github:b/repo/2"),
            ],
        );
        let cache = Mutex::new(InventoryCache::default());
        let first = read_at(&path, params(0, None), &cache).unwrap();
        assert_eq!(first.next_offset, Some(1));
        write(&path, vec![viewed("viewed.github:c/repo/3")]);
        let second = read_at(
            &path,
            params(1, Some(first.inventory_version.clone())),
            &cache,
        )
        .unwrap();
        assert_eq!(second.scopes[0].workspace, "b");
        assert_eq!(second.inventory_version, first.inventory_version);
        let refreshed = read_at(&path, params(0, None), &cache).unwrap();
        assert_eq!(refreshed.scopes[0].workspace, "c");
        assert_ne!(refreshed.inventory_version, first.inventory_version);
        assert!(read_at(
            &path,
            params(1, Some(first.inventory_version)),
            &Mutex::default()
        )
        .is_err());
    }
    #[test]
    fn invalid_pages_expired_snapshots_and_corrupt_storage_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.json");
        write(&path, vec![viewed("viewed.github:a/repo/1")]);
        let cache = Mutex::new(InventoryCache::default());
        assert!(read_at(&path, params(1, None), &cache).is_err());
        assert!(read_at(
            &path,
            ScopeReadParams {
                limit: 101,
                ..params(0, None)
            },
            &cache
        )
        .is_err());
        let first = read_at(&path, params(0, None), &cache).unwrap();
        assert!(read_at(
            &path,
            params(2, Some(first.inventory_version.clone())),
            &cache
        )
        .is_err());
        let old = Instant::now() - SNAPSHOT_AGE;
        let snapshot = cache.lock().unwrap().entries.pop_front().unwrap();
        cache.lock().unwrap().entries.push_back(Arc::new(Snapshot {
            path: snapshot.path.clone(),
            version: snapshot.version.clone(),
            scopes: snapshot.scopes.clone(),
            bytes: snapshot.bytes,
            created: old,
        }));
        assert!(read_at(&path, params(0, Some(first.inventory_version)), &cache).is_err());
        fs::write(&path, b"broken").unwrap();
        assert!(read_at(&path, params(0, None), &cache).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
    }

    #[test]
    fn maximum_inventory_is_paged_and_cache_is_path_scoped_and_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.json");
        write(
            &path,
            (1..=MAX_ENTRIES)
                .map(|id| viewed(&format!("viewed.github:owned/repo/{id}")))
                .collect(),
        );
        let cache = Mutex::new(InventoryCache::default());
        let first = read_at(
            &path,
            ScopeReadParams {
                offset: 0,
                limit: 100,
                inventory_version: None,
            },
            &cache,
        )
        .unwrap();
        assert_eq!(first.total_scopes, MAX_ENTRIES);
        assert_eq!(first.scopes.len(), 100);
        assert!(serde_json::to_vec(&first).unwrap().len() < PAGE_BYTES);
        let mut page = first;
        let mut count = page.scopes.len();
        while let Some(offset) = page.next_offset {
            page = read_at(
                &path,
                ScopeReadParams {
                    offset,
                    limit: 100,
                    inventory_version: Some(page.inventory_version),
                },
                &cache,
            )
            .unwrap();
            count += page.scopes.len();
        }
        assert_eq!(count, MAX_ENTRIES);
        let first_version = page.inventory_version;
        for index in 0..CACHE_ENTRIES {
            let other = temp.path().join(format!("other-{index}.json"));
            write(&other, vec![viewed("viewed.github:owned/repo/1")]);
            assert!(read_at(&other, params(0, Some(first_version.clone())), &cache).is_err());
            read_at(&other, params(0, None), &cache).unwrap();
        }
        assert_eq!(cache.lock().unwrap().entries.len(), CACHE_ENTRIES);
        assert!(
            cache
                .lock()
                .unwrap()
                .entries
                .iter()
                .map(|entry| entry.bytes)
                .sum::<usize>()
                <= CACHE_BYTES
        );
        assert!(read_at(&path, params(0, Some(first_version)), &cache).is_err());
        write(
            &path,
            (1..=MAX_ENTRIES + 1)
                .map(|id| viewed(&format!("viewed.github:owned/repo/{id}")))
                .collect(),
        );
        assert!(read_at(&path, params(0, None), &cache).is_err());
        assert!(inventory(WorkspaceDocument {
            schema_version: 1,
            records: vec![viewed("viewed.github:owned/repo/01")]
        })
        .is_err());
    }
}
