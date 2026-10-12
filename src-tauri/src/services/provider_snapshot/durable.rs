//! Durable exact evidence contains no cache handle, agent identity or credentials.
use super::*;
use crate::services::review_scope::Scope;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format_version: u32,
    capture: Capture,
}

fn identity(metadata: &[u8], patch: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"norn-provider-evidence-v1");
    hash.update((metadata.len() as u64).to_be_bytes());
    hash.update(metadata);
    hash.update((patch.len() as u64).to_be_bytes());
    hash.update(patch);
    format!("{:x}", hash.finalize())
}

fn scope_matches(scope: &Scope, request: &DetailParams) -> bool {
    scope.provider
        == match request.repository.provider {
            crate::config::ReviewProvider::Github => {
                crate::services::review::ReviewProvider::Github
            }
            crate::config::ReviewProvider::Bitbucket => {
                crate::services::review::ReviewProvider::Bitbucket
            }
        }
        && scope.workspace == request.repository.workspace
        && scope.repo == request.repository.repo
        && scope.pr_id == request.pr_id
}

pub(crate) fn save(scope: &Scope, data: &SnapshotData) -> Result<String, String> {
    if !scope_matches(scope, &data.info.request) {
        return Err(invalid());
    }
    let capture = Capture {
        request: data.info.request.clone(),
        title: data.info.title.clone(),
        source_branch: data.info.source_branch.clone(),
        destination_branch: data.info.destination_branch.clone(),
        head_sha: data.info.head_sha.clone(),
        destination_sha: data.info.destination_sha.clone(),
        base_sha: data.info.base_sha.clone(),
        patch: String::new(),
        expected_count: data.files.len(),
        expected_files: Some(data.files.iter().map(|file| file.file.clone()).collect()),
        source_repository: data.source_repository.clone(),
    };
    let metadata = serde_json::to_string(&Envelope {
        format_version: 1,
        capture,
    })
    .map_err(|_| invalid())?;
    let id = identity(metadata.as_bytes(), data.patch.as_bytes());
    crate::review_storage::provider_evidence::save(scope, &id, &metadata, data.patch.as_bytes())?;
    Ok(id)
}

pub(crate) fn load(scope: &Scope, id: &str) -> Result<Capture, String> {
    // A durable record cannot outlive or bypass its validated native owning row.
    crate::services::review_scope::load_owned(scope)?
        .ok_or_else(|| "Saved provider evidence has no valid owning review.".to_string())?;
    let (metadata, patch) =
        crate::review_storage::provider_evidence::load(scope, id)?.ok_or_else(|| {
            "Saved provider evidence is missing; no current PR or checkout fallback was used."
                .to_string()
        })?;
    if identity(metadata.as_bytes(), &patch) != id {
        return Err(invalid());
    }
    let envelope: Envelope = serde_json::from_str(&metadata).map_err(|_| invalid())?;
    if envelope.format_version != 1 {
        return Err(invalid());
    }
    let mut capture = envelope.capture;
    if !scope_matches(scope, &capture.request) {
        return Err(invalid());
    }
    capture.patch = String::from_utf8(patch).map_err(|_| invalid())?;
    // Reprove inventory and all native bounds; durable records never bypass the
    // same verifier used for freshly captured evidence.
    Manager::default().insert(capture.clone())?;
    Ok(capture)
}

pub(crate) fn load_run(
    scope: &Scope,
    run: &crate::services::review::ReviewRun,
) -> Result<Capture, String> {
    if !scope.owns(run) {
        return Err(invalid());
    }
    let id = run.provider_evidence_id.as_deref().ok_or_else(invalid)?;
    let capture = load(scope, id)?;
    if run.reviewed_head_sha.as_deref() != Some(capture.head_sha.as_str())
        || run.reviewed_base_sha.as_deref() != Some(capture.destination_sha.as_str())
        || run.reviewed_diff_base_sha.as_deref() != Some(capture.base_sha.as_str())
        || run.source_branch != capture.source_branch
        || run.destination_branch != capture.destination_branch
    {
        return Err(
            "Saved review pins do not match its provider evidence; no fallback was used.".into(),
        );
    }
    Ok(capture)
}

pub(crate) fn restore(manager: &Manager, scope: &Scope, id: &str) -> Result<SnapshotInfo, String> {
    let capture = load(scope, id)?;
    let cached = manager
        .saved
        .lock()
        .map_err(|_| invalid())?
        .iter()
        .find(|(saved_id, _)| saved_id == id)
        .map(|(_, snapshot)| snapshot.clone());
    if let Some(snapshot) = cached {
        if let Ok(data) = manager.get(&snapshot) {
            return Ok(data.info.clone());
        }
    }
    let info = manager.insert(capture)?;
    let mut saved = manager.saved.lock().map_err(|_| invalid())?;
    saved.retain(|(saved_id, _)| saved_id != id);
    while saved.len() >= manager.count {
        saved.pop_front();
    }
    saved.push_back((id.to_string(), info.snapshot.clone()));
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReviewProvider;

    struct OwnedStorage {
        directory: tempfile::TempDir,
        previous: Option<std::ffi::OsString>,
    }
    impl OwnedStorage {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let previous = std::env::var_os("NORN_REVIEW_DATA_DIR");
            std::env::set_var("NORN_REVIEW_DATA_DIR", directory.path());
            Self {
                directory,
                previous,
            }
        }
    }
    impl Drop for OwnedStorage {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var("NORN_REVIEW_DATA_DIR", value),
                None => std::env::remove_var("NORN_REVIEW_DATA_DIR"),
            }
        }
    }
    fn fixture(provider: ReviewProvider) -> (Manager, Scope, SnapshotInfo) {
        let manager = Manager::default();
        let capture = super::super::tests::capture(provider);
        let scope = Scope {
            provider: match provider {
                ReviewProvider::Github => crate::services::review::ReviewProvider::Github,
                ReviewProvider::Bitbucket => crate::services::review::ReviewProvider::Bitbucket,
            },
            workspace: capture.request.repository.workspace.clone(),
            repo: capture.request.repository.repo.clone(),
            pr_id: capture.request.pr_id,
        };
        let info = manager.insert(capture).unwrap();
        (manager, scope, info)
    }

    #[test]
    fn saved_run_proves_approval_and_diff_pins_and_reuses_only_exact_cached_source() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        let (manager, scope, info) = fixture(ReviewProvider::Github);
        let id = save(&scope, &manager.get(&info.snapshot).unwrap()).unwrap();
        let mut run: crate::services::review::ReviewRun =
            serde_json::from_value(serde_json::json!({
                "id":"owned-run", "schemaVersion":"1", "provider":"github",
                "workspace":scope.workspace, "repo":scope.repo, "prId":scope.pr_id,
                "sourceBranch":info.source_branch, "destinationBranch":info.destination_branch,
                "reviewedBaseSha":info.destination_sha, "reviewedDiffBaseSha":info.base_sha,
                "reviewedHeadSha":info.head_sha, "providerEvidenceId":id,
                "status":"succeeded", "turnKind":"initial", "createdAt":"1",
                "diffFingerprint":"owned", "threadId":null
            }))
            .unwrap();
        assert_eq!(
            load_run(&scope, &run).unwrap().patch,
            manager.get(&info.snapshot).unwrap().patch
        );
        let fresh = Manager::default();
        let restored = restore(&fresh, &scope, &id).unwrap();
        assert_eq!(
            restore(&fresh, &scope, &id).unwrap().snapshot,
            restored.snapshot
        );
        run.reviewed_base_sha = Some(info.base_sha.clone());
        assert!(load_run(&scope, &run).is_err());
        run.reviewed_base_sha = Some(info.destination_sha.clone());
        run.reviewed_diff_base_sha = Some(info.destination_sha.clone());
        assert!(load_run(&scope, &run).is_err());
        run.reviewed_diff_base_sha = Some(info.base_sha.clone());
        run.repo = "foreign".into();
        assert!(load_run(&scope, &run).is_err());
    }

    #[test]
    fn exact_evidence_deduplicates_restores_and_rejects_foreign_or_corrupt_bytes() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let owned = OwnedStorage::new();
        let (manager, scope, info) = fixture(ReviewProvider::Github);
        let data = manager.get(&info.snapshot).unwrap();
        let id = save(&scope, &data).unwrap();
        assert_eq!(save(&scope, &data).unwrap(), id);
        let connection = crate::review_storage::publication_database().unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM provider_review_evidence", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            1
        );
        let (metadata, patch): (String, Vec<u8>) = connection
            .query_row(
                "SELECT metadata_json,patch FROM provider_review_evidence WHERE evidence_id=?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&metadata)
            .unwrap()
            .pointer("/capture/patch")
            .is_none());
        assert_eq!(patch, data.patch.as_bytes());
        assert!(!metadata.contains(&info.snapshot.snapshot_id));
        let fresh = Manager::default();
        let restored = restore(&fresh, &scope, &id).unwrap();
        assert_ne!(restored.snapshot.snapshot_id, info.snapshot.snapshot_id);
        assert_eq!(restored.patch_sha256, info.patch_sha256);
        assert_eq!(fresh.get(&restored.snapshot).unwrap().patch, data.patch);
        let mut foreign = scope.clone();
        foreign.provider = crate::services::review::ReviewProvider::Bitbucket;
        assert!(load(&foreign, &id).is_err());
        connection
            .execute(
                "UPDATE provider_review_evidence SET patch=?1 WHERE evidence_id=?2",
                rusqlite::params![b"tampered".as_slice(), id],
            )
            .unwrap();
        assert!(load(&scope, &id).is_err());
        assert!(owned.directory.path().join("norn.sqlite3").exists());
    }

    #[test]
    fn evidence_follows_native_owner_cleanup_and_consistent_backup() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let _owned = OwnedStorage::new();
        let (gh_manager, gh, gh_info) = fixture(ReviewProvider::Github);
        let (bb_manager, bb, bb_info) = fixture(ReviewProvider::Bitbucket);
        let gh_id = save(&gh, &gh_manager.get(&gh_info.snapshot).unwrap()).unwrap();
        let bb_id = save(&bb, &bb_manager.get(&bb_info.snapshot).unwrap()).unwrap();
        assert_ne!(gh_id, bb_id);
        crate::review_storage::cleanup_stale_reviews(&[]).unwrap();
        assert!(load(&gh, &gh_id).is_ok());
        assert!(load(&bb, &bb_id).is_ok());
        let backup = tempfile::tempdir().unwrap();
        let backup_path = backup.path().join("owned.sqlite3");
        crate::review_storage::create_consistent_database_backup(&backup_path).unwrap();
        crate::review_storage::verify_database_backup(&backup_path).unwrap();
        let copied = rusqlite::Connection::open(&backup_path).unwrap();
        assert_eq!(
            copied
                .query_row("SELECT COUNT(*) FROM provider_review_evidence", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            2
        );
        let connection = crate::review_storage::publication_database().unwrap();
        connection
            .execute(
                "DELETE FROM ai_review_stores WHERE tenant_id='local' AND review_key=?1",
                [gh.key()],
            )
            .unwrap();
        assert!(load(&gh, &gh_id).is_err());
        assert!(load(&bb, &bb_id).is_ok());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM provider_review_evidence", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            1
        );
    }
}
