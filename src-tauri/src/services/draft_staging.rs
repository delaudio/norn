//! Explicit, local finding staging from persisted engine-owned review records.
use super::{review, workspace_data as data};
use crate::protocol::{ProtocolError, ProtocolErrorCode, ProviderKind, TargetIdentity};
use review::{
    ReviewAnchorSide, ReviewFinding, ReviewFindingStatus, ReviewPublicationMode, ReviewRun,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareParams {
    pub target: TargetIdentity,
    pub finding_id: String,
    pub publication_mode: ReviewPublicationMode,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageParams {
    pub target: TargetIdentity,
    pub finding_id: String,
    pub publication_mode: ReviewPublicationMode,
    pub expected_version: String,
    pub preview_hash: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preparation {
    pub target: TargetIdentity,
    pub key: String,
    pub record_version: String,
    pub preview_hash: String,
    pub draft: data::DraftComment,
    pub already_staged: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StageResult {
    pub target: TargetIdentity,
    pub key: String,
    pub record_version: String,
    pub local_id: String,
    pub changed: bool,
    pub already_staged: bool,
}
fn fail(code: ProtocolErrorCode, message: &str) -> ProtocolError {
    ProtocolError::new(code, message)
}
pub(crate) fn validate(params: &PrepareParams) -> Result<(), ProtocolError> {
    let TargetIdentity::Provider {
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id: Some(run_id),
        ..
    } = &params.target
    else {
        return Err(fail(
            ProtocolErrorCode::InvalidParams,
            "Select a pull request and a pinned review run before staging.",
        ));
    };
    if *pr_id == 0
        || [workspace, repo, run_id, &params.finding_id]
            .iter()
            .any(|value| {
                value.is_empty() || value.len() > 256 || value.chars().any(char::is_control)
            })
    {
        return Err(fail(
            ProtocolErrorCode::InvalidParams,
            "Invalid finding staging identity.",
        ));
    }
    Ok(())
}
fn hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn preview_hash(draft: &data::DraftComment) -> Result<String, ProtocolError> {
    // New drafts get their creation time when staged. It is not comment content
    // or an approval boundary; every other previewed field is bound exactly.
    let mut normalized = draft.clone();
    normalized.created_at = 0;
    serde_json::to_vec(&normalized)
        .map(|bytes| hash(&bytes))
        .map_err(|_| fail(ProtocolErrorCode::Internal, "Cannot prepare this draft."))
}
fn mode_name(mode: ReviewPublicationMode) -> &'static str {
    match mode {
        ReviewPublicationMode::Inline => "inline",
        ReviewPublicationMode::File => "file",
        ReviewPublicationMode::General => "general",
        ReviewPublicationMode::LocalOnly => "localOnly",
    }
}
fn safe_path(path: &str) -> bool {
    let drive_prefix = path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && path.as_bytes().get(1) == Some(&b':');
    !(path.is_empty()
        || path.len() > 4096
        || path.starts_with('/')
        || path.contains('\\')
        || drive_prefix
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | "..")))
}
fn revision(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|value| {
        matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}
fn candidate(
    run: &ReviewRun,
    finding: &ReviewFinding,
    mode: ReviewPublicationMode,
) -> Result<data::DraftComment, ProtocolError> {
    if finding.id.is_empty()
        || finding.fingerprint.is_empty()
        || matches!(
            finding.status,
            ReviewFindingStatus::Dismissed
                | ReviewFindingStatus::Published
                | ReviewFindingStatus::Fixed
                | ReviewFindingStatus::Stale
        )
    {
        return Err(fail(ProtocolErrorCode::Conflict, "This finding is no longer available for staging. Use a fresh review or a manual draft."));
    }
    let anchored = matches!(
        mode,
        ReviewPublicationMode::Inline | ReviewPublicationMode::File
    );
    if anchored && (!revision(&run.reviewed_base_sha) || !revision(&run.reviewed_head_sha)) {
        return Err(fail(ProtocolErrorCode::TargetStale, "Reviewed revisions are unavailable. Use a general/local-only draft or rerun the review."));
    }
    let anchor = finding.anchor.as_ref().filter(|anchor| {
        safe_path(&anchor.path)
            && anchor.start_line > 0
            && anchor.end_line.is_none_or(|end| end >= anchor.start_line)
    });
    if anchored && anchor.is_none() {
        return Err(fail(
            ProtocolErrorCode::InvalidParams,
            "This finding has no supported file position. Choose general/local-only explicitly.",
        ));
    }
    let path = if anchored {
        anchor.unwrap().path.clone()
    } else {
        String::new()
    };
    let (to, from) = if mode == ReviewPublicationMode::Inline {
        let anchor = anchor.unwrap();
        match anchor.side {
            ReviewAnchorSide::New => (Some(anchor.start_line), None),
            ReviewAnchorSide::Old => (None, Some(anchor.start_line)),
        }
    } else {
        (None, None)
    };
    let mut raw = format!("### {}\n\n{}", finding.title, finding.summary);
    if let Some(rationale) = finding
        .rationale
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        raw.push_str(&format!("\n\n{rationale}"));
    }
    if let Some(fix) = finding
        .suggested_fix
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        raw.push_str(&format!("\n\nSuggested fix:\n{fix}"));
    }
    let identity = serde_json::to_vec(&(&run.id, &finding.fingerprint)).map_err(|_| {
        fail(
            ProtocolErrorCode::Internal,
            "Cannot prepare draft identity.",
        )
    })?;
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0);
    Ok(data::DraftComment {
        local_id: format!("finding.{}", hash(&identity)),
        pr_id: run.pr_id,
        path,
        to,
        from,
        raw,
        parent_id: None,
        created_at,
        source: Some("aiFinding".into()),
        finding_ref: Some(data::FindingReference {
            review_run_id: run.id.clone(),
            finding_id: finding.id.clone(),
            finding_fingerprint: finding.fingerprint.clone(),
        }),
        publication_mode: Some(mode_name(mode).into()),
        review_base_sha: run.reviewed_base_sha.clone(),
        review_head_sha: run.reviewed_head_sha.clone(),
    })
}
pub(crate) fn prepare(params: PrepareParams) -> Result<Preparation, ProtocolError> {
    validate(&params)?;
    let TargetIdentity::Provider {
        provider,
        workspace,
        repo,
        pr_id: Some(pr_id),
        run_id: Some(run_id),
    } = &params.target
    else {
        unreachable!()
    };
    let provider_name = match provider {
        ProviderKind::Github => "github",
        ProviderKind::Bitbucket => "bitbucket",
    };
    let key = format!("drafts.{provider_name}:{workspace}/{repo}/{pr_id}");
    let (record_version, items) = data::draft_snapshot(&key).map_err(|_| {
        fail(
            ProtocolErrorCode::Conflict,
            "Cannot read this PR's saved drafts. Preserve the source before repairing it.",
        )
    })?;
    let store = review::load_ai_review_store_native(workspace, repo, *pr_id)
        .map_err(|_| fail(ProtocolErrorCode::Internal, "Cannot load the saved review."))?
        .ok_or_else(|| {
            fail(
                ProtocolErrorCode::NotFound,
                "The selected review is unavailable.",
            )
        })?;
    let mut runs = store.review_runs.iter().filter(|run| run.id == *run_id);
    let run = runs.next().ok_or_else(|| {
        fail(
            ProtocolErrorCode::NotFound,
            "The pinned review run is unavailable.",
        )
    })?;
    let expected_provider = match provider {
        ProviderKind::Github => review::ReviewProvider::Github,
        ProviderKind::Bitbucket => review::ReviewProvider::Bitbucket,
    };
    if runs.next().is_some()
        || run.provider != expected_provider
        || run.workspace != *workspace
        || run.repo != *repo
        || run.pr_id != *pr_id
    {
        return Err(fail(
            ProtocolErrorCode::TargetStale,
            "The saved review does not match this provider and PR.",
        ));
    }
    let mut findings = run
        .findings
        .iter()
        .filter(|finding| finding.id == params.finding_id);
    let finding = findings.next().ok_or_else(|| {
        fail(
            ProtocolErrorCode::NotFound,
            "The selected finding is unavailable.",
        )
    })?;
    if findings.next().is_some() {
        return Err(fail(
            ProtocolErrorCode::Conflict,
            "The saved finding identity is ambiguous.",
        ));
    }
    if store
        .review_runs
        .iter()
        .filter(|prior| {
            prior.provider == expected_provider
                && prior.workspace == *workspace
                && prior.repo == *repo
                && prior.pr_id == *pr_id
        })
        .flat_map(|prior| &prior.findings)
        .any(|prior| {
            prior.fingerprint == finding.fingerprint
                && (prior.status == ReviewFindingStatus::Published
                    || prior.publication.as_ref().is_some_and(|publication| {
                        !publication.remote_comment_ids.is_empty()
                            || publication.published_at.is_some()
                    }))
        })
    {
        return Err(fail(ProtocolErrorCode::Conflict, "This finding has a saved publication acknowledgment. Use a manual follow-up instead of staging it again."));
    }
    let new = candidate(run, finding, params.publication_mode)?;
    let mut existing = items.iter().filter(|draft| {
        draft.finding_ref.as_ref().is_some_and(|reference| {
            reference.review_run_id == run.id
                && reference.finding_id == finding.id
                && reference.finding_fingerprint == finding.fingerprint
        })
    });
    let old = existing.next();
    if existing.next().is_some() {
        return Err(fail(
            ProtocolErrorCode::Conflict,
            "Multiple saved drafts refer to this finding. Inspect them before staging.",
        ));
    }
    if old.is_none() && items.iter().any(|draft| draft.local_id == new.local_id) {
        return Err(fail(
            ProtocolErrorCode::Conflict,
            "A saved draft already owns this identity. Inspect it before staging.",
        ));
    }
    let draft = old.cloned().unwrap_or(new);
    data::WorkspaceRecord { key: key.clone(), payload: data::WorkspaceValue::Drafts(vec![draft.clone()]) }.validate()
        .map_err(|_| fail(ProtocolErrorCode::InvalidParams, "Finding content exceeds draft limits or contains invalid metadata. Use a manual draft instead."))?;
    Ok(Preparation {
        target: params.target,
        key,
        record_version,
        preview_hash: preview_hash(&draft)?,
        draft,
        already_staged: old.is_some(),
    })
}
pub(crate) fn stage(params: StageParams) -> Result<StageResult, ProtocolError> {
    let prepared = prepare(PrepareParams {
        target: params.target,
        finding_id: params.finding_id,
        publication_mode: params.publication_mode,
    })?;
    if prepared.record_version != params.expected_version
        || prepared.preview_hash != params.preview_hash
    {
        return Err(fail(
            ProtocolErrorCode::Conflict,
            "The preview or saved drafts changed. Refresh the preview before staging.",
        ));
    }
    let acknowledgment = data::record_mutate(data::RecordMutateParams {
        key: prepared.key.clone(),
        expected_version: params.expected_version,
        mutation: data::RecordMutation::PutDraft {
            draft: Box::new(prepared.draft.clone()),
        },
    })
    .map_err(|_| {
        fail(
            ProtocolErrorCode::Conflict,
            "Draft staging was not confirmed. Inspect saved drafts before retrying.",
        )
    })?;
    Ok(StageResult {
        target: prepared.target,
        key: prepared.key,
        record_version: acknowledgment.record_version,
        local_id: prepared.draft.local_id,
        changed: acknowledgment.changed,
        already_staged: prepared.already_staged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run() -> ReviewRun {
        serde_json::from_value(json!({"id":"run-1","schemaVersion":"1","provider":"github","workspace":"owner","repo":"repo","prId":7,
            "sourceBranch":"feature","destinationBranch":"main","reviewedBaseSha":"a".repeat(40),"reviewedHeadSha":"b".repeat(40),
            "status":"succeeded","turnKind":"initial","createdAt":"1","finishedAt":"2","diffFingerprint":"diff-1","threadId":null,"summaryMarkdown":null,
            "findings":[{"id":"finding-1","fingerprint":"fingerprint-1","title":"Missing validation","severity":"high","confidence":"high","category":"bug","status":"new",
            "summary":"Check the value 世界","rationale":"A caller can send null.","ruleId":null,"source":"llm","anchor":{"path":"src/test.ts","startLine":12,"endLine":14,"side":"old"},
            "suggestedFix":"Return early.","evidenceIds":[],"publication":null}]})).unwrap()
    }

    #[test]
    fn modes_preserve_authoritative_text_identity_and_reviewed_revisions() {
        let run = run();
        let inline = candidate(&run, &run.findings[0], ReviewPublicationMode::Inline).unwrap();
        assert_eq!(inline.to, None);
        assert_eq!(inline.from, Some(12));
        assert_eq!(inline.path, "src/test.ts");
        assert_eq!(inline.raw, "### Missing validation\n\nCheck the value 世界\n\nA caller can send null.\n\nSuggested fix:\nReturn early.");
        assert_eq!(
            inline.finding_ref.as_ref().unwrap().finding_fingerprint,
            "fingerprint-1"
        );
        assert_eq!(inline.review_head_sha, run.reviewed_head_sha);
        let file = candidate(&run, &run.findings[0], ReviewPublicationMode::File).unwrap();
        assert_eq!(file.path, inline.path);
        assert_eq!((file.to, file.from), (None, None));
        let general = candidate(&run, &run.findings[0], ReviewPublicationMode::General).unwrap();
        assert!(general.path.is_empty());
        assert_eq!(general.local_id, inline.local_id);
        assert_ne!(
            preview_hash(&general).unwrap(),
            preview_hash(&inline).unwrap()
        );
        let mut later = inline.clone();
        later.created_at += 1;
        assert_eq!(
            preview_hash(&later).unwrap(),
            preview_hash(&inline).unwrap()
        );
        later.raw.push_str(" edited");
        assert_ne!(
            preview_hash(&later).unwrap(),
            preview_hash(&inline).unwrap()
        );
    }

    #[test]
    fn non_publishable_positions_never_fall_back_to_another_mode_or_side() {
        let mut run = run();
        for path in [
            "",
            "../secret",
            "/absolute",
            "src/../test.ts",
            "C:/test.ts",
            "src\\test.ts",
            "src/\u{1b}",
        ] {
            run.findings[0].anchor.as_mut().unwrap().path = path.into();
            assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::Inline).is_err());
            assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::File).is_err());
            assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::General).is_ok());
        }
        run.findings[0].anchor.as_mut().unwrap().path = "src/time:stamp.rs".into();
        assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::Inline).is_ok());
        run.findings[0].anchor = None;
        assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::Inline).is_err());
        assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::LocalOnly).is_ok());
        run.reviewed_head_sha = None;
        assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::File).is_err());
        assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::General).is_ok());
        run.findings[0].status = ReviewFindingStatus::Dismissed;
        assert!(candidate(&run, &run.findings[0], ReviewPublicationMode::General).is_err());
    }
}
