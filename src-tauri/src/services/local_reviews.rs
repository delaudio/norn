//! Exact native Local ownership, independent of provider PR keys and process cwd.
use super::{
    local_snapshot::{Data, Manager, SnapshotRef},
    review::{
        AiReviewRunStatus, AiReviewStoreData, AiReviewTurnKind, ReviewAnchorSide,
        ReviewFindingSeverity, ReviewRun,
    },
};
use crate::local_review::LocalReviewDiffLayerKind as Layer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
fn invalid() -> String {
    "The saved local review could not be verified for this snapshot.".into()
}
fn valid_run(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartParams {
    pub snapshot: SnapshotRef,
    pub profile: Option<String>,
}
impl StartParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if self.profile.as_ref().is_some_and(|v| {
            v.trim() != v
                || v.is_empty()
                || v.chars().count() > 128
                || v.len() > 512
                || v.chars().any(char::is_control)
        }) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryParams {
    pub snapshot: SnapshotRef,
    pub run_id: Option<String>,
}
impl QueryParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if self.run_id.as_deref().is_some_and(|id| !valid_run(id)) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryParams {
    pub snapshot: SnapshotRef,
    pub offset: usize,
    pub limit: usize,
}
impl HistoryParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if self.offset > 1000 || !(1..=32).contains(&self.limit) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingsParams {
    pub snapshot: SnapshotRef,
    pub run_id: Option<String>,
    pub offset: usize,
    pub limit: usize,
}
impl FindingsParams {
    pub fn validate(&self) -> Result<(), String> {
        QueryParams {
            snapshot: self.snapshot.clone(),
            run_id: self.run_id.clone(),
        }
        .validate()?;
        if self.offset > 6000 || !(1..=32).contains(&self.limit) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunSummary {
    pub run_id: String,
    pub created_at: String,
    pub status: AiReviewRunStatus,
    pub turn_kind: AiReviewTurnKind,
    pub findings_count: usize,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryPage {
    pub request: HistoryParams,
    pub runs: Vec<RunSummary>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Anchor {
    pub path: String,
    pub start_line: u32,
    pub end_line: Option<u32>,
    pub side: ReviewAnchorSide,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub severity: ReviewFindingSeverity,
    pub summary: String,
    pub anchor: Option<Anchor>,
    pub candidate_layers: Vec<Layer>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingsPage {
    pub request: FindingsParams,
    pub run_id: Option<String>,
    pub findings: Vec<Finding>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub request: QueryParams,
    pub run_id: String,
    pub text: String,
}

pub(crate) fn store(
    manager: &Manager,
    snapshot: &SnapshotRef,
) -> Result<(Arc<Data>, AiReviewStoreData), String> {
    let data = manager.get(snapshot)?;
    let store = crate::review_storage::local_evidence::owned_store(&data.info)?;
    if store.threads.len() > 1000
        || store.threads.iter().any(|thread| {
            !valid_run(&thread.id)
                || thread.messages.len() > 10000
                || thread
                    .messages
                    .iter()
                    .any(|message| !valid_run(&message.id))
        })
        || store.review_runs.len() > 1000
        || store.review_runs.iter().any(|run| {
            !valid_run(&run.id)
                || run.created_at.is_empty()
                || run.created_at.len() > 32
                || !run.created_at.bytes().all(|b| b.is_ascii_digit())
                || run.findings.len() > 6000
                || run.reviewed_base_sha.as_deref() != Some(data.info.base_sha.as_str())
                || run.source_branch != data.info.branch
        })
    {
        return Err(invalid());
    }
    Ok((data, store))
}
fn selected<'a>(
    store: &'a AiReviewStoreData,
    id: Option<&str>,
) -> Result<Option<&'a ReviewRun>, String> {
    match id {
        Some(id) => store
            .review_runs
            .iter()
            .find(|run| run.id == id)
            .map(Some)
            .ok_or_else(invalid),
        None => Ok(store.review_runs.last()),
    }
}
pub(crate) fn history(manager: &Manager, request: HistoryParams) -> Result<HistoryPage, String> {
    request.validate()?;
    let (_, store) = store(manager, &request.snapshot)?;
    let total = store.review_runs.len();
    if request.offset > total {
        return Err(invalid());
    }
    let end = (request.offset + request.limit).min(total);
    let runs = store
        .review_runs
        .iter()
        .rev()
        .skip(request.offset)
        .take(end - request.offset)
        .map(|run| RunSummary {
            run_id: run.id.clone(),
            created_at: run.created_at.clone(),
            status: run.status,
            turn_kind: run.turn_kind,
            findings_count: run.findings.len(),
        })
        .collect();
    Ok(HistoryPage {
        request,
        runs,
        total,
        next_offset: (end < total).then_some(end),
    })
}
pub(crate) fn findings(manager: &Manager, request: FindingsParams) -> Result<FindingsPage, String> {
    request.validate()?;
    let (data, store) = store(manager, &request.snapshot)?;
    let run = selected(&store, request.run_id.as_deref())?;
    let total = run.map_or(0, |run| run.findings.len());
    if request.offset > total {
        return Err(invalid());
    }
    let end = (request.offset + request.limit).min(total);
    let mut findings = Vec::new();
    if let Some(run) = run {
        for finding in &run.findings[request.offset..end] {
            if !valid_run(&finding.id) || finding.title.len() > 4096 || finding.summary.len() > 8192
            {
                return Err("Saved local finding exceeds the terminal page limit.".into());
            }
            let (anchor, candidate_layers) = if let Some(anchor) = &finding.anchor {
                if !super::provider_snapshot::safe_path(&anchor.path)
                    || anchor.start_line == 0
                    || anchor.end_line.is_some_and(|end| end < anchor.start_line)
                {
                    return Err(invalid());
                }
                (
                    Some(Anchor {
                        path: anchor.path.clone(),
                        start_line: anchor.start_line,
                        end_line: anchor.end_line,
                        side: anchor.side,
                    }),
                    data.layers_for_path(&anchor.path),
                )
            } else {
                (None, Vec::new())
            };
            findings.push(Finding {
                id: finding.id.clone(),
                title: finding.title.clone(),
                severity: finding.severity,
                summary: finding.summary.clone(),
                anchor,
                candidate_layers,
            });
        }
    }
    let page = FindingsPage {
        run_id: run.map(|run| run.id.clone()),
        request,
        findings,
        total,
        next_offset: (end < total).then_some(end),
    };
    if serde_json::to_vec(&page).map_err(|_| invalid())?.len() > 768 * 1024 {
        return Err(invalid());
    }
    Ok(page)
}
pub(crate) fn output(manager: &Manager, request: QueryParams) -> Result<Output, String> {
    request.validate()?;
    let (_, store) = store(manager, &request.snapshot)?;
    let run = selected(&store, request.run_id.as_deref())?.ok_or_else(invalid)?;
    let text = run
        .summary_markdown
        .as_ref()
        .filter(|text| !text.trim().is_empty() && text.len() <= 64 * 1024)
        .ok_or_else(|| {
            "The selected local run has no bounded stored output to copy.".to_string()
        })?;
    Ok(Output {
        run_id: run.id.clone(),
        request,
        text: text.clone(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Message {
    pub id: String,
    pub role: super::review::AiReviewMessageRole,
    pub content: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub latest_run_id: String,
    pub selected: bool,
    pub messages: Vec<Message>,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Threads {
    pub request: QueryParams,
    pub run_id: Option<String>,
    pub threads: Vec<Thread>,
    pub truncated: bool,
}
pub(crate) fn threads(manager: &Manager, request: QueryParams) -> Result<Threads, String> {
    request.validate()?;
    let (_, store) = store(manager, &request.snapshot)?;
    let selected = selected(&store, request.run_id.as_deref())?;
    let mut threads = Vec::new();
    let mut total = 0;
    for thread in store.threads.iter().rev() {
        let Some(latest) = store
            .review_runs
            .iter()
            .rev()
            .find(|run| run.thread_id.as_deref() == Some(thread.id.as_str()))
        else {
            continue;
        };
        total += 1;
        if threads.len() >= 12 {
            continue;
        }
        let messages = thread
            .messages
            .iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|message| Message {
                id: message.id.clone(),
                role: message.role,
                content: message.content.chars().take(1000).collect(),
                truncated: message.content.chars().count() > 1000,
            })
            .collect();
        threads.push(Thread {
            id: thread.id.clone(),
            title: thread.title.chars().take(128).collect(),
            latest_run_id: latest.id.clone(),
            selected: selected
                .is_some_and(|run| run.thread_id.as_deref() == Some(thread.id.as_str())),
            messages,
            truncated: thread.messages.len() > 8,
        });
    }
    Ok(Threads {
        run_id: selected.map(|run| run.id.clone()),
        request,
        threads,
        truncated: total > 12,
    })
}
