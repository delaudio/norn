//! Consistent, database-only backup and restore for a self-hosted deployment.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::review_storage::{self, DB_FILE};

const MANIFEST_FILE: &str = "manifest.json";
const BACKUP_DATABASE_FILE: &str = "lachesi.sqlite3";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupManifest {
    schema_version: String,
    created_at_ms: u64,
    database_file: String,
    database_sha256: String,
    artifact_policy: String,
}

pub fn create(destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err("Backup destination must not already exist".to_string());
    }
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let database = destination.join(BACKUP_DATABASE_FILE);
    if let Err(error) = review_storage::create_consistent_database_backup(&database) {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    let manifest = BackupManifest {
        schema_version: "v1".to_string(),
        created_at_ms: now_ms(),
        database_file: BACKUP_DATABASE_FILE.to_string(),
        database_sha256: sha256_file(&database)?,
        artifact_policy:
            "database-only; excludes source code, prompt bodies, and plaintext credentials"
                .to_string(),
    };
    write_manifest(destination, &manifest)
}

pub fn restore(source: &Path, destination_data_dir: &Path) -> Result<(), String> {
    if destination_data_dir.exists()
        && fs::read_dir(destination_data_dir)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err("Restore destination must be an empty deployment data directory".to_string());
    }
    let manifest = read_manifest(source)?;
    if manifest.schema_version != "v1" || manifest.database_file != BACKUP_DATABASE_FILE {
        return Err("Backup archive format is incompatible".to_string());
    }
    let database = source.join(&manifest.database_file);
    if sha256_file(&database)? != manifest.database_sha256 {
        return Err("Backup archive checksum does not match its manifest".to_string());
    }
    review_storage::verify_database_backup(&database)?;
    fs::create_dir_all(destination_data_dir).map_err(|error| error.to_string())?;
    fs::copy(database, destination_data_dir.join(DB_FILE)).map_err(|error| error.to_string())?;
    review_storage::verify_database_backup(&destination_data_dir.join(DB_FILE))
}

fn read_manifest(source: &Path) -> Result<BackupManifest, String> {
    let bytes = fs::read(source.join(MANIFEST_FILE))
        .map_err(|_| "Backup archive is missing its manifest".to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| "Backup archive manifest is invalid".to_string())
}

fn write_manifest(destination: &Path, manifest: &BackupManifest) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    fs::write(destination.join(MANIFEST_FILE), bytes).map_err(|error| error.to_string())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|_| "Backup archive is missing its database".to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_event::PullRequestReviewEventProvider;
    use crate::review_storage::{
        self, ReviewCursorIdentity, ReviewCursorState, ReviewRunCompletion, ReviewRunOutcome,
    };

    #[test]
    fn backup_round_trip_restores_durable_cursor_and_rejects_tampering() {
        let _lock = review_storage::TEST_DATA_DIR_ENV_LOCK.lock().expect("lock");
        let source = tempfile::tempdir().expect("source");
        let previous = std::env::var_os("LACHESI_REVIEW_DATA_DIR");
        std::env::set_var("LACHESI_REVIEW_DATA_DIR", source.path());
        let identity = ReviewCursorIdentity {
            tenant_id: "tenant-acme".to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repo: "payments".to_string(),
            pr_id: 42,
        };
        review_storage::record_review_completion(&ReviewRunCompletion {
            identity: identity.clone(),
            reviewed_base_sha: Some("1".repeat(40)),
            reviewed_head_sha: "2".repeat(40),
            current_head_sha: "2".repeat(40),
            expected_previous_head_sha: None,
            run_id: "run-1".to_string(),
            completed_at: "1000".to_string(),
            outcome: ReviewRunOutcome::Succeeded,
        })
        .expect("cursor");
        let archive_parent = tempfile::tempdir().expect("archive parent");
        let archive = archive_parent.path().join("backup");
        create(&archive).expect("backup");
        let destination = tempfile::tempdir().expect("destination parent");
        let restore_dir = destination.path().join("data");
        restore(&archive, &restore_dir).expect("restore");
        std::env::set_var("LACHESI_REVIEW_DATA_DIR", &restore_dir);
        assert!(matches!(
            review_storage::get_review_cursor(&identity).expect("cursor"),
            ReviewCursorState::Reviewed(_)
        ));
        fs::write(archive.join(BACKUP_DATABASE_FILE), b"corrupt").expect("tamper");
        let second = destination.path().join("second");
        assert!(restore(&archive, &second).is_err());
        match previous {
            Some(value) => std::env::set_var("LACHESI_REVIEW_DATA_DIR", value),
            None => std::env::remove_var("LACHESI_REVIEW_DATA_DIR"),
        }
    }
}
