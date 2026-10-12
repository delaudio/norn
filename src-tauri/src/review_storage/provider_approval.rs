//! Durable approval claim before POST; unresolved writes cannot be replayed.
use super::*;
use crate::services::provider_approval::{Entry, Proof, State};
use crate::services::review_scope::Scope;
const MAX_ENTRY_BYTES: usize = 65536;
const MAX_RECORDS: i64 = 1000;
fn invalid() -> String {
    "Local approval history is unavailable or invalid. Check provider status before sending again."
        .into()
}
fn provider_name(proof: &Proof) -> &'static str {
    match proof.request.repository.provider {
        crate::config::ReviewProvider::Github => "github",
        crate::config::ReviewProvider::Bitbucket => "bitbucket",
    }
}
fn state_name(state: State) -> &'static str {
    match state {
        State::Writing => "writing",
        State::Approved => "approved",
        State::Rejected => "rejected",
        State::Uncertain => "uncertain",
    }
}
pub(crate) fn pending(proof: &Proof) -> Result<Option<Entry>, String> {
    proof.validate()?;
    let entry = pending_from(&publication_database()?, proof)?;
    if entry.is_some() {
        crate::services::review_scope::load_owned(&proof.scope())?.ok_or_else(invalid)?;
    }
    Ok(entry)
}
fn pending_from(connection: &Connection, proof: &Proof) -> Result<Option<Entry>, String> {
    // Proved immutable provider/repo/principal identity blocks unresolved writes
    // even through renamed/case-variant request aliases and changed heads.
    let candidate:Option<(String,String)>=connection.query_row(
        "SELECT identity_key,scope_key FROM provider_pr_approvals WHERE provider=?1 AND repository_id=?2 AND actor_id=?3 AND pr_id=?4 AND state IN ('writing','uncertain') ORDER BY rowid DESC LIMIT 1",
        params![provider_name(proof),proof.repository_id,proof.actor.account_id,proof.request.pr_id],|row|Ok((row.get(0)?,row.get(1)?))
    ).optional().map_err(|_|invalid())?;
    let Some((identity, owner)) = candidate else {
        return Ok(None);
    };
    if owner != proof.scope().key() {
        return Err("An unresolved approval belongs to another saved repository alias. Check its provider status before sending again.".into());
    }
    let entry = read(connection, &proof.scope(), &identity)?.ok_or_else(invalid)?;
    if !matches!(entry.state, State::Writing | State::Uncertain)
        || entry.proof.repository_id != proof.repository_id
        || entry.proof.actor.account_id != proof.actor.account_id
    {
        return Err(invalid());
    }
    Ok(Some(entry))
}
fn read(connection: &Connection, scope: &Scope, identity: &str) -> Result<Option<Entry>, String> {
    type JournalRow = (String, Option<String>, String, String, String, u32, String);
    let row:Option<JournalRow>=connection.query_row(
        "SELECT scope_key,CASE WHEN scope_key=?2 AND length(CAST(entry_json AS BLOB))<=65536 THEN entry_json ELSE NULL END,provider,repository_id,actor_id,pr_id,state FROM provider_pr_approvals WHERE identity_key=?1",
        params![identity,scope.key()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))
    ).optional().map_err(|_|invalid())?;
    let Some((owner, json, provider, repository, actor, pr_id, state)) = row else {
        return Ok(None);
    };
    if owner != scope.key() {
        return Err(invalid());
    }
    let entry: Entry = serde_json::from_str(&json.ok_or_else(invalid)?).map_err(|_| invalid())?;
    entry.validate()?;
    if entry.proof.scope() != *scope
        || entry.proof.identity() != identity
        || provider != provider_name(&entry.proof)
        || repository != entry.proof.repository_id
        || actor != entry.proof.actor.account_id
        || pr_id != entry.proof.request.pr_id
        || state != state_name(entry.state)
    {
        return Err(invalid());
    }
    Ok(Some(entry))
}
pub(crate) fn load(proof: &Proof) -> Result<Option<Entry>, String> {
    proof.validate()?;
    let connection = publication_database()?;
    let entry = read(&connection, &proof.scope(), &proof.identity())?;
    if entry.is_some() {
        crate::services::review_scope::load_owned(&proof.scope())?.ok_or_else(invalid)?;
    }
    Ok(entry)
}
fn write(connection: &Connection, entry: &Entry) -> Result<(), String> {
    entry.validate()?;
    let json = serde_json::to_string(entry).map_err(|_| invalid())?;
    if json.len() > MAX_ENTRY_BYTES {
        return Err(invalid());
    }
    connection.execute("INSERT INTO provider_pr_approvals(identity_key,scope_key,provider,repository_id,actor_id,pr_id,state,entry_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(identity_key) DO UPDATE SET state=excluded.state,entry_json=excluded.entry_json WHERE provider_pr_approvals.scope_key=excluded.scope_key",
        params![entry.proof.identity(),entry.proof.scope().key(),provider_name(&entry.proof),entry.proof.repository_id,entry.proof.actor.account_id,entry.proof.request.pr_id,state_name(entry.state),json]).map_err(|_|invalid())?;
    Ok(())
}
pub(crate) fn claim(proof: &Proof, now: u64) -> Result<Entry, String> {
    proof.validate()?;
    let mut connection = publication_database()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| invalid())?;
    super::provider_evidence::ensure_owner(&tx, &proof.scope())?;
    if pending_from(&tx, proof)?.is_some() {
        return Err("An approval by this reviewer remains uncertain. Reconcile its provider status without sending again.".into());
    }
    let prior = read(&tx, &proof.scope(), &proof.identity())?;
    if prior
        .as_ref()
        .is_some_and(|entry| entry.state != State::Rejected)
    {
        return Err("This approval was already sent or remains uncertain. Refresh its status without sending again.".into());
    }
    if prior.is_none() {
        let count: i64 = tx
            .query_row("SELECT COUNT(*) FROM provider_pr_approvals", [], |row| {
                row.get(0)
            })
            .map_err(|_| invalid())?;
        if count >= MAX_RECORDS {
            return Err("Local approval history is full; no approval was sent.".into());
        }
    }
    let attempt = prior.map_or(Ok(1), |entry| {
        entry.attempt.checked_add(1).ok_or_else(invalid)
    })?;
    let entry = Entry {
        format_version: 1,
        proof: proof.clone(),
        state: State::Writing,
        remote_review_id: None,
        updated_at: now,
        attempt,
    };
    write(&tx, &entry)?;
    tx.commit().map_err(|_| invalid())?;
    Ok(entry)
}
pub(crate) fn finish(
    claimed: &Entry,
    state: State,
    remote_review_id: Option<String>,
    now: u64,
) -> Result<Entry, String> {
    if state == State::Writing {
        return Err(invalid());
    }
    let mut connection = publication_database()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| invalid())?;
    super::provider_evidence::ensure_owner(&tx, &claimed.proof.scope())?;
    let current =
        read(&tx, &claimed.proof.scope(), &claimed.proof.identity())?.ok_or_else(invalid)?;
    if current != *claimed || !matches!(current.state, State::Writing | State::Uncertain) {
        return Err(invalid());
    }
    let entry = Entry {
        state,
        remote_review_id,
        updated_at: now,
        ..current
    };
    write(&tx, &entry)?;
    tx.commit().map_err(|_| invalid())?;
    Ok(entry)
}
