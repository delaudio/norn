//! Content addressed local captures follow their actual native Local parent.
use super::*;
use crate::services::{
    local_snapshot::{Info, SnapshotRef, METADATA_BYTES},
    review::AiReviewStoreData,
    review_scope::Scope,
};
fn invalid() -> String {
    "Saved local evidence is invalid, unavailable or outside its storage limit.".into()
}
fn key(snapshot: &SnapshotRef) -> Result<String, String> {
    snapshot.validate()?;
    local_review_key(
        &snapshot.repository.workspace,
        &snapshot.repository.repo,
        &snapshot.snapshot_id,
    )
}
fn ensure_owner(
    connection: &Connection,
    info: &Info,
    create: bool,
) -> Result<AiReviewStoreData, String> {
    let snapshot = &info.snapshot;
    let key = key(snapshot)?;
    let row:Option<(String,String,u32,String,Option<String>)>=connection.query_row(
        "SELECT workspace,repo,pr_id,target_kind,CASE WHEN length(CAST(store_json AS BLOB))<=16777216 THEN store_json ELSE NULL END FROM ai_review_stores WHERE tenant_id='local' AND review_key=?1",
        [&key], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))
    ).optional().map_err(|_|invalid())?;
    if let Some((workspace, repo, id, kind, json)) = row {
        if workspace != snapshot.repository.workspace
            || repo != snapshot.repository.repo
            || id != info.local_review_id
            || kind != "local"
        {
            return Err(invalid());
        }
        let owned: AiReviewStoreData =
            serde_json::from_str(&json.ok_or_else(invalid)?).map_err(|_| invalid())?;
        let scope = Scope {
            provider: match snapshot.repository.provider {
                crate::config::ReviewProvider::Github => {
                    crate::services::review::ReviewProvider::Github
                }
                crate::config::ReviewProvider::Bitbucket => {
                    crate::services::review::ReviewProvider::Bitbucket
                }
            },
            workspace,
            repo,
            pr_id: id,
        };
        scope.validate_owned(&owned)?;
        if owned
            .review_runs
            .iter()
            .any(|run| run.reviewed_head_sha.as_deref() != Some(snapshot.snapshot_id.as_str()))
        {
            return Err(invalid());
        }
        return Ok(owned);
    } else if create {
        let empty = serde_json::to_string(&AiReviewStoreData::default()).map_err(|_| invalid())?;
        upsert_review_json_for_connection(
            connection,
            "local",
            &key,
            &snapshot.repository.workspace,
            &snapshot.repository.repo,
            info.local_review_id,
            &empty,
            false,
            "local",
            &now_ms(),
            0,
        )?;
    } else {
        return Err(invalid());
    }
    Ok(AiReviewStoreData::default())
}
pub(crate) fn owned_store(info: &Info) -> Result<AiReviewStoreData, String> {
    ensure_owner(&open()?, info, false)
}
pub(crate) fn save(info: &Info, json: &str) -> Result<(), String> {
    info.snapshot.validate()?;
    if json.is_empty() || json.len() > METADATA_BYTES {
        return Err(invalid());
    }
    let mut connection = publication_database()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| invalid())?;
    ensure_owner(&tx, info, true)?;
    let existing = read(&tx, &info.snapshot)?;
    if let Some(existing) = existing {
        if existing != json {
            return Err(invalid());
        }
    } else {
        let (count,bytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(metadata_json AS BLOB))),0) FROM local_review_evidence",[],|row|Ok((row.get(0)?,row.get(1)?))).map_err(|_|invalid())?;
        if count >= 1000 || bytes + json.len() as i64 > 128 * 1024 * 1024 {
            return Err(
                "Saved local evidence storage is full. Free local history before capturing again."
                    .into(),
            );
        }
        tx.execute("INSERT INTO local_review_evidence(evidence_id,scope_key,metadata_json,created_at) VALUES (?1,?2,?3,?4)",params![info.snapshot.snapshot_id,key(&info.snapshot)?,json,now_ms()]).map_err(|_|invalid())?;
    }
    tx.commit().map_err(|_| invalid())
}
fn read(connection: &Connection, snapshot: &SnapshotRef) -> Result<Option<String>, String> {
    let owner = key(snapshot)?;
    let row:Option<(String,Option<String>)>=connection.query_row("SELECT scope_key,CASE WHEN scope_key=?2 AND length(CAST(metadata_json AS BLOB))<=33554432 THEN metadata_json ELSE NULL END FROM local_review_evidence WHERE tenant_id='local' AND evidence_id=?1",params![snapshot.snapshot_id,owner],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(|_|invalid())?;
    match row {
        None => Ok(None),
        Some((stored, json)) if stored == owner => Ok(Some(json.ok_or_else(invalid)?)),
        Some(_) => Err(invalid()),
    }
}
pub(crate) fn load(snapshot: &SnapshotRef) -> Result<Option<String>, String> {
    let connection = open()?;
    let json = read(&connection, snapshot)?;
    if let Some(json) = &json {
        // Only this bounded header is deserialized here; the caller then proves
        // the complete content hash, format, inventory and canonical root ID.
        let value: serde_json::Value = serde_json::from_str(json).map_err(|_| invalid())?;
        let info: Info = serde_json::from_value(value.get("info").cloned().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        if info.snapshot != *snapshot {
            return Err(invalid());
        }
        ensure_owner(&connection, &info, false)?;
    }
    Ok(json)
}
