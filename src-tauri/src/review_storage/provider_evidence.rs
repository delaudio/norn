//! Content-addressed local provider evidence; ownership follows the native row.
use super::*;
use crate::services::review::AiReviewStoreData;
use crate::services::review_scope::Scope;

const METADATA_BYTES: usize = 16 * 1024 * 1024;
const PATCH_BYTES: usize = 8 * 1024 * 1024;
const TOTAL_BYTES: i64 = 128 * 1024 * 1024;
const RECORDS: i64 = 1000;

fn invalid() -> String {
    "Saved provider evidence is unavailable, invalid or outside its storage limit.".into()
}

fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

pub(crate) fn ensure_owner(connection: &Connection, scope: &Scope) -> Result<(), String> {
    let parent = connection.query_row(
        "SELECT workspace, repo, pr_id, target_kind, CASE WHEN length(CAST(store_json AS BLOB)) <= 16777216 THEN store_json ELSE NULL END
         FROM ai_review_stores WHERE tenant_id='local' AND review_key=?1",
        [scope.key()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, u32>(2)?, row.get::<_, String>(3)?, row.get::<_, Option<String>>(4)?))
    ).optional().map_err(|_| invalid())?;
    match parent {
        Some((workspace, repo, pr_id, kind, json)) => {
            if workspace != scope.workspace
                || repo != scope.repo
                || pr_id != scope.pr_id
                || kind != "pull_request"
            {
                return Err(invalid());
            }
            let owned: AiReviewStoreData =
                serde_json::from_str(&json.ok_or_else(invalid)?).map_err(|_| invalid())?;
            scope.validate_owned(&owned)?;
        }
        None => {
            let empty =
                serde_json::to_string(&AiReviewStoreData::default()).map_err(|_| invalid())?;
            upsert_review_json_for_connection(
                connection,
                "local",
                &scope.key(),
                &scope.workspace,
                &scope.repo,
                scope.pr_id,
                &empty,
                false,
                "pull_request",
                &now_ms(),
                0,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn save(scope: &Scope, id: &str, metadata: &str, patch: &[u8]) -> Result<(), String> {
    if !valid_id(id)
        || metadata.is_empty()
        || metadata.len() > METADATA_BYTES
        || patch.len() > PATCH_BYTES
    {
        return Err(invalid());
    }
    let mut connection = publication_database()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| invalid())?;
    ensure_owner(&tx, scope)?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT scope_key FROM provider_review_evidence WHERE evidence_id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| invalid())?;
    if let Some(owner) = existing {
        if owner != scope.key() {
            return Err(invalid());
        }
        // The content hash is verified by the caller; never replace a durable record.
        let stored = read_from(&tx, scope, id)?.ok_or_else(invalid)?;
        if stored.0 != metadata || stored.1 != patch {
            return Err(invalid());
        }
    } else {
        let (count, bytes): (i64, i64) = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(metadata_json AS BLOB)) + length(patch)), 0) FROM provider_review_evidence",
            [], |row| Ok((row.get(0)?, row.get(1)?))
        ).map_err(|_| invalid())?;
        if count >= RECORDS || bytes + metadata.len() as i64 + patch.len() as i64 > TOTAL_BYTES {
            return Err("Saved provider evidence storage is full; no review was started.".into());
        }
        tx.execute("INSERT INTO provider_review_evidence(evidence_id,scope_key,metadata_json,patch,created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, scope.key(), metadata, patch, now_ms()]).map_err(|_| invalid())?;
    }
    tx.commit().map_err(|_| invalid())
}

fn read_from(
    connection: &Connection,
    scope: &Scope,
    id: &str,
) -> Result<Option<(String, Vec<u8>)>, String> {
    let row = connection
        .query_row(
            "SELECT scope_key,
         CASE WHEN length(CAST(metadata_json AS BLOB)) <= 16777216 THEN metadata_json ELSE NULL END,
         CASE WHEN length(patch) <= 8388608 THEN patch ELSE NULL END
         FROM provider_review_evidence WHERE tenant_id='local' AND evidence_id=?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|_| invalid())?;
    match row {
        None => Ok(None),
        Some((owner, metadata, patch)) if owner == scope.key() => Ok(Some((
            metadata.ok_or_else(invalid)?,
            patch.ok_or_else(invalid)?,
        ))),
        Some(_) => Err(invalid()),
    }
}

pub(crate) fn load(scope: &Scope, id: &str) -> Result<Option<(String, Vec<u8>)>, String> {
    if !valid_id(id) {
        return Err(invalid());
    }
    read_from(&open()?, scope, id)
}
