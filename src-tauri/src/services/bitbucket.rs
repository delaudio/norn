use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use std::path::Path;

use crate::config::{self, AppConfig, RepoRef, ReviewProvider};
use crate::config::{AiProvider, ReviewTerminal};
use crate::credentials::{self, Credentials};
use crate::finding_publication::{
    dry_run_publication_identity, FindingAnchorSide, FindingLineRange, FindingPublicationError,
    FindingPublicationErrorCode, FindingPublicationRequest, FindingPublisher,
    ProviderCommentIdentity, ProviderInlineCommentApi, ProviderInlineCommentPayload,
    ProviderPublicationApiError, ProviderPublicationTarget, ProviderPullRequestRevision,
    PublishedCommentIdentity, SqliteFindingPublicationStore,
};
use crate::finding_reconciliation::{
    FindingReconciler, FindingReconciliationRequest, FindingReconciliationSummary,
    ProviderFindingComment, ProviderFindingReconciliationApi,
};
use crate::repo_config::{self, RepoReviewConfigLoadResult};
use crate::review_event::PullRequestReviewEventProvider;
use crate::review_storage::{self, ClosedPrMetric};

const BASE: &str = "https://api.bitbucket.org/2.0";
const BITBUCKET_PUBLICATION_COMMENT_FIELDS: &str = concat!(
    "next,values.id,values.deleted,values.user.account_id,values.content.raw,values.inline.path,",
    "values.inline.to,values.inline.from,values.inline.start_to,values.inline.start_from"
);

/// When `LACHESI_DRY_RUN` is truthy, comment-creating commands log and return a
/// synthetic comment instead of POSTing — lets the full UI flow run against live
/// read data without writing to a shared repo.
fn dry_run() -> bool {
    std::env::var("LACHESI_DRY_RUN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

struct BitbucketClient {
    username: String,
    token: String,
    http: reqwest::blocking::Client,
}

struct GithubClient {
    token: String,
    http: reqwest::blocking::Client,
}

impl BitbucketClient {
    fn new(creds: Credentials) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .user_agent("lachesi")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            username: creds.username,
            token: creds.token,
            http,
        })
    }

    fn from_stored() -> Result<Self, String> {
        let creds = credentials::load().ok_or_else(|| {
            "No Bitbucket credentials configured. Open Settings to add them, set BITBUCKET_USERNAME and BITBUCKET_TOKEN, or configure env refs in ~/.config/lachesi/config.toml."
                .to_string()
        })?;
        Self::new(creds)
    }

    fn get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .get(url)
            .basic_auth(&self.username, Some(&self.token))
    }

    fn post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .post(url)
            .basic_auth(&self.username, Some(&self.token))
    }

    fn put(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .put(url)
            .basic_auth(&self.username, Some(&self.token))
    }

    fn delete(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .delete(url)
            .basic_auth(&self.username, Some(&self.token))
    }
}

impl GithubClient {
    fn new(token: String) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .user_agent("lachesi")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { token, http })
    }

    fn from_stored() -> Result<Self, String> {
        let token = credentials::load_github_token().ok_or_else(|| {
            "No GitHub token configured. Open Settings to add it, set GITHUB_TOKEN, or configure an env ref in ~/.config/lachesi/config.toml."
                .to_string()
        })?;
        Self::new(token)
    }

    fn get(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .get(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
    }

    fn get_diff(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .get(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3.diff")
    }

    fn post(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .post(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
    }

    fn patch(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .patch(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
    }

    fn delete(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .delete(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
    }
}

fn repo_base(workspace: &str, repo: &str) -> Result<String, String> {
    if workspace.trim().is_empty() || repo.trim().is_empty() {
        return Err("Bitbucket workspace/repo is required.".to_string());
    }
    Ok(format!(
        "{BASE}/repositories/{}/{}",
        encode_path_segment(workspace),
        encode_path_segment(repo)
    ))
}

fn github_repo_base(owner: &str, repo: &str) -> Result<String, String> {
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err("GitHub owner/repository is required.".to_string());
    }
    Ok(format!(
        "https://api.github.com/repos/{}/{}",
        encode_path_segment(owner),
        encode_path_segment(repo)
    ))
}

fn encode_path_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn encode_path(value: &str) -> String {
    value
        .split('/')
        .map(encode_path_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn comment_id_from_value(value: serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Number(number) if number.as_u64().is_some() => Ok(number.to_string()),
        serde_json::Value::String(value) if !value.trim().is_empty() => Ok(value),
        _ => Err("Provider returned an invalid comment id.".to_string()),
    }
}

fn deserialize_comment_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    comment_id_from_value(serde_json::Value::deserialize(deserializer)?)
        .map_err(serde::de::Error::custom)
}

fn deserialize_optional_comment_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<serde_json::Value>::deserialize(deserializer)?
        .map(comment_id_from_value)
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn bitbucket_comment_id_number(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| "Bitbucket comment ids must be unsigned decimal numbers.".to_string())
}

fn image_mime_type(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        Some("image/png")
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg")
    } else if lower.ends_with(".svg") {
        Some("image/svg+xml")
    } else if lower.ends_with(".webp") {
        Some("image/webp")
    } else if lower.ends_with(".gif") {
        Some("image/gif")
    } else {
        None
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn check(resp: reqwest::blocking::Response) -> Result<reqwest::blocking::Response, String> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().unwrap_or_default();
    Err(format!("Bitbucket API error {status}: {body}"))
}

fn check_github(resp: reqwest::blocking::Response) -> Result<reqwest::blocking::Response, String> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let rate_limited_by_header = github_rate_limit_wait(status, resp.headers()).is_some();
    let body = resp.text().unwrap_or_default();
    if rate_limited_by_header || body.to_ascii_lowercase().contains("rate limit") {
        Err(format!("GitHub API rate limit error {status}: {body}"))
    } else {
        Err(format!("GitHub API error {status}: {body}"))
    }
}

fn github_rate_limit_wait(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
) -> Option<u64> {
    let retry_after = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let remaining_is_zero = headers
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "0");
    if status.as_u16() != 429
        && !(status == reqwest::StatusCode::FORBIDDEN
            && (retry_after.is_some() || remaining_is_zero))
    {
        return None;
    }
    if let Some(wait) = retry_after {
        return Some(wait);
    }
    let reset_at = headers
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Some(
        reset_at
            .map(|reset_at| reset_at.saturating_sub(now).max(1))
            .unwrap_or(1),
    )
}

/// Send a request, retrying on 429 (honoring `Retry-After`) and transient 5xx
/// with bounded exponential backoff, then surface non-success as an error.
fn send_checked(
    req: reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::Response, String> {
    send_checked_with_policy(req, BitbucketRetryPolicy::RetryTransient)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BitbucketRetryPolicy {
    RetryTransient,
    AtMostOnce,
}

fn should_retry_bitbucket_request(
    policy: BitbucketRetryPolicy,
    status: reqwest::StatusCode,
    attempt: u32,
) -> bool {
    policy == BitbucketRetryPolicy::RetryTransient
        && attempt < 3
        && (status.as_u16() == 429 || status.is_server_error())
}

fn send_checked_with_policy(
    req: reqwest::blocking::RequestBuilder,
    policy: BitbucketRetryPolicy,
) -> Result<reqwest::blocking::Response, String> {
    let mut attempt: u32 = 0;
    loop {
        let this = req
            .try_clone()
            .ok_or_else(|| "request is not retryable".to_string())?;
        let resp = this.send().map_err(|e| e.to_string())?;
        let status = resp.status();
        if should_retry_bitbucket_request(policy, status, attempt) {
            let wait = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1u64 << attempt);
            std::thread::sleep(std::time::Duration::from_secs(wait.min(10)));
            attempt += 1;
            continue;
        }
        return check(resp);
    }
}

fn send_once_checked(
    req: reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::Response, String> {
    send_checked_with_policy(req, BitbucketRetryPolicy::AtMostOnce)
}

fn get_json<T: DeserializeOwned>(req: reqwest::blocking::RequestBuilder) -> Result<T, String> {
    let resp = send_checked(req)?;
    resp.json::<T>().map_err(|e| e.to_string())
}

fn github_get_json<T: DeserializeOwned>(
    req: reqwest::blocking::RequestBuilder,
) -> Result<T, String> {
    let resp = github_send_checked(req)?;
    resp.json::<T>().map_err(|e| e.to_string())
}

fn github_send_checked(
    req: reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::Response, String> {
    let mut attempt = 0;
    loop {
        let this = req
            .try_clone()
            .ok_or_else(|| "request is not retryable".to_string())?;
        let response = this.send().map_err(|error| error.to_string())?;
        if let Some(wait) = github_rate_limit_wait(response.status(), response.headers()) {
            if attempt < 3 {
                std::thread::sleep(std::time::Duration::from_secs(wait.min(10)));
                attempt += 1;
                continue;
            }
        }
        return check_github(response);
    }
}

#[derive(Deserialize)]
struct BbCommitPage {
    #[serde(default)]
    values: Vec<serde::de::IgnoredAny>,
    next: Option<String>,
}

/// Count commits reachable from `include` but not `exclude`, capped. Returns
/// (count, capped) where `capped` means there were more than `cap`.
fn count_commits(
    client: &BitbucketClient,
    base: &str,
    include: &str,
    exclude: &str,
    cap: u32,
) -> Result<(u32, bool), String> {
    let pagelen = cap.to_string();
    let url = format!("{base}/commits");
    let page: BbCommitPage = get_json(client.get(&url).query(&[
        ("include", include),
        ("exclude", exclude),
        ("pagelen", pagelen.as_str()),
        ("fields", "values.hash,next"),
    ]))?;
    Ok((page.values.len() as u32, page.next.is_some()))
}

/// Run blocking work on a worker thread so the webview never stalls.
async fn run<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Bitbucket wire structs (deserialize only what we use)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct BbAuthor {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Clone, Deserialize)]
struct BbBranch {
    #[serde(default)]
    name: String,
}

#[derive(Clone, Deserialize)]
struct BbCommitRef {
    #[serde(default)]
    hash: String,
}

#[derive(Clone, Deserialize)]
struct BbBranchRef {
    branch: Option<BbBranch>,
    commit: Option<BbCommitRef>,
}

#[derive(Deserialize)]
struct BbPrSummary {
    id: u32,
    #[serde(default)]
    title: String,
    author: Option<BbAuthor>,
    source: Option<BbBranchRef>,
    destination: Option<BbBranchRef>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    comment_count: u32,
    #[serde(default)]
    created_on: String,
    #[serde(default)]
    updated_on: String,
    #[serde(default)]
    participants: Vec<BbParticipant>,
}

#[derive(Deserialize)]
struct BbPrPage {
    #[serde(default)]
    values: Vec<BbPrSummary>,
    #[serde(default)]
    size: u32,
    #[serde(default)]
    page: u32,
    next: Option<String>,
}

#[derive(Deserialize)]
struct BbParticipant {
    #[serde(default)]
    role: String,
    #[serde(default)]
    approved: bool,
    user: Option<BbAuthor>,
}

#[derive(Deserialize)]
struct BbPrDetail {
    id: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    author: Option<BbAuthor>,
    source: Option<BbBranchRef>,
    destination: Option<BbBranchRef>,
    #[serde(default)]
    created_on: String,
    #[serde(default)]
    updated_on: String,
    #[serde(default)]
    participants: Vec<BbParticipant>,
}

#[derive(Deserialize)]
struct BbDiffstatFile {
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
struct BbDiffstat {
    #[serde(default)]
    status: String,
    #[serde(default)]
    lines_added: u32,
    #[serde(default)]
    lines_removed: u32,
    old: Option<BbDiffstatFile>,
    new: Option<BbDiffstatFile>,
}

#[derive(Deserialize)]
struct BbDiffstatPage {
    #[serde(default)]
    values: Vec<BbDiffstat>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct BbContent {
    #[serde(default)]
    raw: String,
    html: Option<String>,
}

#[derive(Deserialize)]
struct BbInline {
    #[serde(default)]
    path: String,
    to: Option<u32>,
    from: Option<u32>,
}

#[derive(Deserialize)]
struct BbParent {
    #[serde(deserialize_with = "deserialize_comment_id")]
    id: String,
}

#[derive(Deserialize)]
struct BbComment {
    #[serde(deserialize_with = "deserialize_comment_id")]
    id: String,
    content: Option<BbContent>,
    user: Option<BbAuthor>,
    #[serde(default)]
    created_on: String,
    #[serde(default)]
    deleted: bool,
    inline: Option<BbInline>,
    parent: Option<BbParent>,
}

#[derive(Deserialize)]
struct BbCommentPage {
    #[serde(default)]
    values: Vec<BbComment>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct BbPublicationComment {
    id: serde_json::Value,
    content: Option<BbContent>,
    user: Option<BbAuthor>,
    #[serde(default)]
    deleted: bool,
    inline: Option<BbPublicationInline>,
}

#[derive(Deserialize)]
struct BbPublicationInline {
    #[serde(default)]
    path: String,
    to: Option<u32>,
    from: Option<u32>,
    start_to: Option<u32>,
    start_from: Option<u32>,
}

#[derive(Deserialize)]
struct BbPublicationCommentPage {
    #[serde(default)]
    values: Vec<BbPublicationComment>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct BbUser {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct GhUser {
    #[serde(default)]
    login: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct GhRef {
    #[serde(default)]
    #[allow(dead_code)]
    label: String,
    #[serde(default)]
    #[allow(dead_code)]
    r#ref: String,
    #[serde(default)]
    sha: String,
}

#[derive(Deserialize)]
struct GhPullRequest {
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    user: Option<GhUser>,
    head: GhRef,
    base: GhRef,
    #[serde(default)]
    comments: Option<u32>,
    #[serde(default)]
    review_comments: Option<u32>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    requested_reviewers: Vec<GhUser>,
}

#[derive(Deserialize)]
struct GhFile {
    #[serde(default)]
    filename: String,
    #[serde(default)]
    previous_filename: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
}

#[derive(Deserialize)]
struct GhContentFile {
    #[serde(default)]
    content: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    size: usize,
}

#[derive(Deserialize)]
struct GhReviewComment {
    #[serde(deserialize_with = "deserialize_comment_id")]
    id: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    body_html: Option<String>,
    user: Option<GhUser>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    original_line: Option<u32>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_comment_id")]
    in_reply_to_id: Option<String>,
}

#[derive(Deserialize)]
struct GhIssueComment {
    #[serde(deserialize_with = "deserialize_comment_id")]
    id: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    body_html: Option<String>,
    user: Option<GhUser>,
    #[serde(default)]
    created_at: String,
}

#[derive(Deserialize)]
struct GhPublicationComment {
    id: serde_json::Value,
    #[serde(default)]
    body: String,
    user: Option<GhUser>,
    path: Option<String>,
    line: Option<u32>,
    original_line: Option<u32>,
    start_line: Option<u32>,
    original_start_line: Option<u32>,
    side: Option<String>,
    start_side: Option<String>,
}

#[derive(Deserialize)]
struct GhCompare {
    #[serde(default)]
    ahead_by: u32,
    #[serde(default)]
    behind_by: u32,
}

// ---------------------------------------------------------------------------
// Output structs (camelCase to match the TS DTOs)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSummary {
    pub id: u32,
    pub title: String,
    pub author_display_name: String,
    pub author_account_id: Option<String>,
    pub source_branch: String,
    pub destination_branch: String,
    pub state: String,
    pub draft: bool,
    pub comment_count: u32,
    pub created_on: String,
    pub updated_on: String,
    pub reviewers: Vec<Participant>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestPage {
    pub values: Vec<PullRequestSummary>,
    pub size: u32,
    pub page: u32,
    pub has_next: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub display_name: String,
    pub account_id: Option<String>,
    pub role: String,
    pub approved: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestDetail {
    pub id: u32,
    pub title: String,
    pub description_raw: String,
    pub state: String,
    pub draft: bool,
    pub author_display_name: String,
    pub reviewers: Vec<Participant>,
    pub source_branch: String,
    pub destination_branch: String,
    pub source_commit_hash: Option<String>,
    pub destination_commit_hash: Option<String>,
    pub created_on: String,
    pub updated_on: String,
}

pub struct PullRequestReviewSnapshot {
    pub detail: PullRequestDetail,
    pub diff: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffstatEntry {
    status: String,
    lines_added: u32,
    lines_removed: u32,
    old_path: Option<String>,
    new_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrFilePreview {
    path: String,
    mime_type: String,
    data_url: String,
    size: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPrAnalyticsSnapshot {
    metrics: Vec<ClosedPrMetric>,
    synced_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineAnchor {
    pub path: String,
    pub to: Option<u32>,
    pub from: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrComment {
    pub id: String,
    pub parent_id: Option<String>,
    pub content_raw: String,
    pub content_html: Option<String>,
    pub user_display_name: String,
    pub created_on: String,
    pub deleted: bool,
    pub inline: Option<InlineAnchor>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUser {
    display_name: String,
    account_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchStatus {
    /// Commits on the destination branch not in the source (how far behind).
    behind: u32,
    /// Commits on the source branch not in the destination (the PR's own work).
    ahead: u32,
    behind_capped: bool,
    ahead_capped: bool,
}

// ---------------------------------------------------------------------------
// Input structs
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListPrOptions {
    pub state: Option<String>,
    pub page: Option<u32>,
    pub pagelen: Option<u32>,
    pub query: Option<String>,
    pub updated_after: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPrAnalyticsOptions {
    limit_per_state: Option<u32>,
    updated_after: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInlineComment {
    path: String,
    to: Option<u32>,
    from: Option<u32>,
    raw: String,
    parent_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Mappers
// ---------------------------------------------------------------------------

fn branch_name(r: Option<BbBranchRef>) -> String {
    r.and_then(|r| r.branch).map(|b| b.name).unwrap_or_default()
}

fn commit_hash(r: Option<BbBranchRef>) -> Option<String> {
    r.and_then(|r| r.commit)
        .map(|commit| commit.hash)
        .filter(|hash| !hash.is_empty())
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn map_reviewers(participants: Vec<BbParticipant>) -> Vec<Participant> {
    participants
        .into_iter()
        .filter(|p| p.role.eq_ignore_ascii_case("REVIEWER"))
        .map(|p| {
            let (display_name, account_id) = match p.user {
                Some(user) => (user.display_name, user.account_id),
                None => (String::new(), None),
            };
            Participant {
                display_name,
                account_id,
                role: p.role,
                approved: p.approved,
            }
        })
        .collect()
}

fn map_pr_summary(p: BbPrSummary) -> PullRequestSummary {
    let (author_display_name, author_account_id) = match p.author {
        Some(a) => (a.display_name, a.account_id),
        None => (String::new(), None),
    };
    PullRequestSummary {
        id: p.id,
        title: p.title,
        author_display_name,
        author_account_id,
        source_branch: branch_name(p.source),
        destination_branch: branch_name(p.destination),
        state: p.state,
        draft: p.draft,
        comment_count: p.comment_count,
        created_on: p.created_on,
        updated_on: p.updated_on,
        reviewers: map_reviewers(p.participants),
    }
}

fn map_diffstat(d: BbDiffstat) -> DiffstatEntry {
    DiffstatEntry {
        status: d.status,
        lines_added: d.lines_added,
        lines_removed: d.lines_removed,
        old_path: d.old.map(|f| f.path),
        new_path: d.new.map(|f| f.path),
    }
}

fn map_comment(c: BbComment) -> PrComment {
    let (content_raw, content_html) = match c.content {
        Some(content) => (content.raw, content.html),
        None => (String::new(), None),
    };
    PrComment {
        id: c.id,
        parent_id: c.parent.map(|p| p.id),
        content_raw,
        content_html,
        user_display_name: c.user.map(|u| u.display_name).unwrap_or_default(),
        created_on: c.created_on,
        deleted: c.deleted,
        inline: c.inline.map(|i| InlineAnchor {
            path: i.path,
            to: i.to,
            from: i.from,
        }),
    }
}

fn map_pr_detail(bb: BbPrDetail) -> PullRequestDetail {
    let reviewers = map_reviewers(bb.participants);
    PullRequestDetail {
        id: bb.id,
        title: bb.title,
        description_raw: bb.description,
        state: bb.state,
        draft: bb.draft,
        author_display_name: bb.author.map(|a| a.display_name).unwrap_or_default(),
        reviewers,
        source_branch: branch_name(bb.source.clone()),
        destination_branch: branch_name(bb.destination.clone()),
        source_commit_hash: commit_hash(bb.source),
        destination_commit_hash: commit_hash(bb.destination),
        created_on: bb.created_on,
        updated_on: bb.updated_on,
    }
}

fn provider_for(provider: Option<ReviewProvider>, workspace: &str, repo: &str) -> ReviewProvider {
    if let Some(provider) = provider {
        return provider;
    }
    let cfg = config::load();
    cfg.repos
        .iter()
        .find(|candidate| candidate.workspace == workspace && candidate.repo == repo)
        .map(|candidate| candidate.provider)
        .unwrap_or(cfg.review_provider)
}

fn gh_user_label(user: Option<GhUser>) -> (String, Option<String>) {
    match user {
        Some(user) => {
            let login = user.login;
            let label = user
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| login.clone());
            (label, Some(login))
        }
        None => (String::new(), None),
    }
}

fn gh_state(pr: &GhPullRequest) -> String {
    if pr.state.eq_ignore_ascii_case("open") {
        "OPEN".to_string()
    } else if pr.merged_at.is_some() {
        "MERGED".to_string()
    } else {
        "DECLINED".to_string()
    }
}

fn gh_reviewers(users: Vec<GhUser>) -> Vec<Participant> {
    users
        .into_iter()
        .map(|user| {
            let (display_name, account_id) = gh_user_label(Some(user));
            Participant {
                display_name,
                account_id,
                role: "REVIEWER".to_string(),
                approved: false,
            }
        })
        .collect()
}

fn map_gh_pr_summary(pr: GhPullRequest) -> PullRequestSummary {
    let state = gh_state(&pr);
    let (author_display_name, author_account_id) = gh_user_label(pr.user);
    let comment_count = pr.comments.unwrap_or(0) + pr.review_comments.unwrap_or(0);
    PullRequestSummary {
        id: pr.number,
        title: pr.title,
        author_display_name,
        author_account_id,
        source_branch: pr.head.r#ref,
        destination_branch: pr.base.r#ref,
        state,
        draft: pr.draft,
        comment_count,
        created_on: pr.created_at,
        updated_on: pr.updated_at,
        reviewers: gh_reviewers(pr.requested_reviewers),
    }
}

fn map_gh_pr_detail(pr: GhPullRequest) -> PullRequestDetail {
    let state = gh_state(&pr);
    let (author_display_name, _) = gh_user_label(pr.user);
    PullRequestDetail {
        id: pr.number,
        title: pr.title,
        description_raw: pr.body.unwrap_or_default(),
        state,
        draft: pr.draft,
        author_display_name,
        reviewers: gh_reviewers(pr.requested_reviewers),
        source_branch: pr.head.r#ref,
        destination_branch: pr.base.r#ref,
        source_commit_hash: non_empty(pr.head.sha),
        destination_commit_hash: non_empty(pr.base.sha),
        created_on: pr.created_at,
        updated_on: pr.updated_at,
    }
}

fn map_gh_file(file: GhFile) -> DiffstatEntry {
    let status = match file.status.as_str() {
        "removed" => "removed",
        "renamed" => "renamed",
        "added" => "added",
        _ => "modified",
    };
    DiffstatEntry {
        status: status.to_string(),
        lines_added: file.additions,
        lines_removed: file.deletions,
        old_path: file.previous_filename,
        new_path: Some(file.filename),
    }
}

fn map_gh_review_comment(comment: GhReviewComment) -> PrComment {
    let (user_display_name, _) = gh_user_label(comment.user);
    let is_left = comment
        .side
        .as_deref()
        .map(|side| side.eq_ignore_ascii_case("LEFT"))
        .unwrap_or(false);
    PrComment {
        id: comment.id,
        parent_id: comment.in_reply_to_id,
        content_raw: comment.body,
        content_html: comment.body_html,
        user_display_name,
        created_on: comment.created_at,
        deleted: false,
        inline: Some(InlineAnchor {
            path: comment.path,
            to: if is_left { None } else { comment.line },
            from: if is_left {
                comment.original_line.or(comment.line)
            } else {
                None
            },
        }),
    }
}

fn map_gh_issue_comment(comment: GhIssueComment) -> PrComment {
    let (user_display_name, _) = gh_user_label(comment.user);
    PrComment {
        id: comment.id,
        parent_id: None,
        content_raw: comment.body,
        content_html: comment.body_html,
        user_display_name,
        created_on: comment.created_at,
        deleted: false,
        inline: None,
    }
}

fn github_paginated_get<T: DeserializeOwned>(
    client: &GithubClient,
    mut url: String,
) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    loop {
        let resp = github_send_checked(client.get(&url))?;
        let next = resp
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|value| value.to_str().ok())
            .map(github_next_link)
            .transpose()?
            .flatten();
        out.extend(resp.json::<Vec<T>>().map_err(|e| e.to_string())?);
        match next {
            Some(next) => url = next,
            None => break,
        }
    }
    Ok(out)
}

fn github_next_link(value: &str) -> Result<Option<String>, String> {
    for entry in value.split(',') {
        let mut parts = entry.split(';');
        let Some(target) = parts.next().map(str::trim) else {
            continue;
        };
        let is_next = parts.any(|part| part.trim() == r#"rel="next""#);
        if !is_next {
            continue;
        }
        let Some(url) = target
            .strip_prefix('<')
            .and_then(|target| target.strip_suffix('>'))
        else {
            return Err("GitHub returned an invalid pagination link.".to_string());
        };
        if !url.starts_with("https://api.github.com/") {
            return Err("GitHub returned an unsafe pagination link.".to_string());
        }
        return Ok(Some(url.to_string()));
    }
    Ok(None)
}

fn fetch_github_pull_request_detail(
    client: &GithubClient,
    owner: &str,
    repo: &str,
    id: u32,
) -> Result<PullRequestDetail, String> {
    let url = format!("{}/pulls/{id}", github_repo_base(owner, repo)?);
    let pr: GhPullRequest = github_get_json(client.get(&url))?;
    Ok(map_gh_pr_detail(pr))
}

fn fetch_pull_request_detail(
    client: &BitbucketClient,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<PullRequestDetail, String> {
    let url = format!("{}/pullrequests/{id}", repo_base(workspace, repo)?);
    let bb: BbPrDetail = get_json(client.get(&url))?;
    Ok(map_pr_detail(bb))
}

fn now_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn query_literal(value: &str) -> String {
    value.replace(['\\', '"'], "")
}

fn pr_query_filter(opts: &ListPrOptions) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(q) = opts.query.as_ref().filter(|q| !q.is_empty()) {
        parts.push(format!("title ~ \"{}\"", query_literal(q)));
    }
    if let Some(updated_after) = opts
        .updated_after
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!(
            "updated_on >= \"{}\"",
            query_literal(updated_after)
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}

fn fetch_pull_requests_page(
    client: &BitbucketClient,
    workspace: &str,
    repo: &str,
    opts: &ListPrOptions,
) -> Result<PullRequestPage, String> {
    let url = format!("{}/pullrequests", repo_base(workspace, repo)?);
    let page = opts.page.unwrap_or(1);
    let pagelen = opts.pagelen.unwrap_or(30);
    let mut query: Vec<(String, String)> = vec![
        ("page".into(), page.to_string()),
        ("pagelen".into(), pagelen.to_string()),
        (
            "fields".into(),
            "size,page,next,values.id,values.title,values.state,values.draft,values.comment_count,values.created_on,values.updated_on,values.author.display_name,values.author.account_id,values.source.branch.name,values.destination.branch.name,values.participants.role,values.participants.approved,values.participants.user.display_name,values.participants.user.account_id".into(),
        ),
    ];
    match opts.state.as_deref() {
        Some("ALL") => {
            for s in ["OPEN", "MERGED", "DECLINED", "SUPERSEDED"] {
                query.push(("state".into(), s.into()));
            }
        }
        Some(s) => query.push(("state".into(), s.to_string())),
        None => query.push(("state".into(), "OPEN".into())),
    }
    if let Some(filter) = pr_query_filter(opts) {
        query.push(("q".into(), filter));
    }
    let bb: BbPrPage = get_json(client.get(&url).query(&query))?;
    Ok(PullRequestPage {
        values: bb.values.into_iter().map(map_pr_summary).collect(),
        size: bb.size,
        page: bb.page.max(1),
        has_next: bb.next.is_some(),
    })
}

fn fetch_diffstat_entries(
    client: &BitbucketClient,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<Vec<DiffstatEntry>, String> {
    let mut url = format!(
        "{}/pullrequests/{id}/diffstat?pagelen=100",
        repo_base(workspace, repo)?
    );
    let mut out = Vec::new();
    loop {
        let page: BbDiffstatPage = get_json(client.get(&url))?;
        out.extend(page.values.into_iter().map(map_diffstat));
        match page.next {
            Some(next) => url = next,
            None => break,
        }
    }
    Ok(out)
}

fn preview_from_bytes(path: String, mime_type: &str, bytes: Vec<u8>) -> PrFilePreview {
    let encoded = base64_encode(&bytes);
    PrFilePreview {
        path,
        mime_type: mime_type.to_string(),
        data_url: format!("data:{mime_type};base64,{encoded}"),
        size: bytes.len(),
    }
}

fn fetch_bitbucket_file_preview(
    client: &BitbucketClient,
    workspace: &str,
    repo: &str,
    branch: &str,
    path: &str,
) -> Result<PrFilePreview, String> {
    let mime_type = image_mime_type(path)
        .ok_or_else(|| "Only PNG, JPEG, SVG, WebP, and GIF previews are supported.".to_string())?;
    let url = format!(
        "{}/src/{}/{}",
        repo_base(workspace, repo)?,
        encode_path_segment(branch),
        encode_path(path)
    );
    let bytes = send_checked(client.get(&url))?
        .bytes()
        .map_err(|e| e.to_string())?
        .to_vec();
    Ok(preview_from_bytes(path.to_string(), mime_type, bytes))
}

fn fetch_github_file_preview(
    client: &GithubClient,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
) -> Result<PrFilePreview, String> {
    let mime_type = image_mime_type(path)
        .ok_or_else(|| "Only PNG, JPEG, SVG, WebP, and GIF previews are supported.".to_string())?;
    let url = format!(
        "{}/contents/{}?ref={}",
        github_repo_base(owner, repo)?,
        encode_path(path),
        encode_path_segment(branch)
    );
    let content: GhContentFile = github_get_json(client.get(&url))?;
    if !content.encoding.eq_ignore_ascii_case("base64") {
        return Err("GitHub returned an unsupported file encoding.".to_string());
    }
    let encoded: String = content
        .content
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    Ok(PrFilePreview {
        path: path.to_string(),
        mime_type: mime_type.to_string(),
        data_url: format!("data:{mime_type};base64,{encoded}"),
        size: content.size,
    })
}

fn fetch_github_pull_requests_page(
    client: &GithubClient,
    owner: &str,
    repo: &str,
    opts: &ListPrOptions,
) -> Result<PullRequestPage, String> {
    let page = opts.page.unwrap_or(1);
    let per_page = opts.pagelen.unwrap_or(30).min(100);
    let state = match opts.state.as_deref() {
        Some("MERGED") | Some("DECLINED") | Some("SUPERSEDED") => "closed",
        Some("ALL") => "all",
        _ => "open",
    };
    let url = format!(
        "{}/pulls?state={state}&per_page={per_page}&page={page}",
        github_repo_base(owner, repo)?
    );
    let mut values: Vec<PullRequestSummary> =
        github_get_json::<Vec<GhPullRequest>>(client.get(&url))?
            .into_iter()
            .filter(|pr| {
                let mapped = gh_state(pr);
                match opts.state.as_deref() {
                    Some("MERGED") => mapped == "MERGED",
                    Some("DECLINED") | Some("SUPERSEDED") => mapped == "DECLINED",
                    _ => true,
                }
            })
            .map(map_gh_pr_summary)
            .collect();
    if let Some(query) = opts.query.as_ref().filter(|query| !query.is_empty()) {
        let query = query.to_lowercase();
        values.retain(|pr| {
            pr.title.to_lowercase().contains(&query)
                || pr.source_branch.to_lowercase().contains(&query)
                || pr.author_display_name.to_lowercase().contains(&query)
                || pr.id.to_string().contains(&query)
        });
    }
    if let Some(updated_after) = opts
        .updated_after
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        values.retain(|pr| pr.updated_on.as_str() >= updated_after.as_str());
    }
    Ok(PullRequestPage {
        size: values.len() as u32,
        page,
        has_next: values.len() as u32 == per_page,
        values,
    })
}

fn fetch_github_diffstat_entries(
    client: &GithubClient,
    owner: &str,
    repo: &str,
    id: u32,
) -> Result<Vec<DiffstatEntry>, String> {
    let url = format!(
        "{}/pulls/{id}/files?per_page=100&page=1",
        github_repo_base(owner, repo)?
    );
    Ok(github_paginated_get::<GhFile>(client, url)?
        .into_iter()
        .map(map_gh_file)
        .collect())
}

fn fetch_github_comments(
    client: &GithubClient,
    owner: &str,
    repo: &str,
    id: u32,
) -> Result<Vec<PrComment>, String> {
    let base = github_repo_base(owner, repo)?;
    let review_comments = github_paginated_get::<GhReviewComment>(
        client,
        format!("{base}/pulls/{id}/comments?per_page=100&page=1"),
    )?;
    let issue_comments = github_paginated_get::<GhIssueComment>(
        client,
        format!("{base}/issues/{id}/comments?per_page=100&page=1"),
    )?;
    let mut out: Vec<PrComment> = review_comments
        .into_iter()
        .map(map_gh_review_comment)
        .chain(issue_comments.into_iter().map(map_gh_issue_comment))
        .collect();
    out.sort_by(|a, b| a.created_on.cmp(&b.created_on));
    Ok(out)
}

fn fetch_github_head_sha(
    client: &GithubClient,
    owner: &str,
    repo: &str,
    id: u32,
) -> Result<String, String> {
    let url = format!("{}/pulls/{id}", github_repo_base(owner, repo)?);
    let pr: GhPullRequest = github_get_json(client.get(&url))?;
    Ok(pr.head.sha)
}

fn cached_closed_metrics_for_repos(repos: &[RepoRef]) -> Result<Vec<ClosedPrMetric>, String> {
    if repos.is_empty() {
        return Ok(Vec::new());
    }
    let metrics = review_storage::list_closed_pr_metrics()?;
    Ok(metrics
        .into_iter()
        .filter(|metric| {
            repos
                .iter()
                .any(|repo| repo.workspace == metric.workspace && repo.repo == metric.repo)
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Commands — connection / config / credentials
// ---------------------------------------------------------------------------

pub fn load_config_native() -> Result<AppConfig, String> {
    let mut cfg = config::load();
    cfg.configured = !cfg.repos.is_empty();
    cfg.has_credentials = credentials::has();
    cfg.has_github_credentials = credentials::has_github();
    cfg.has_jira = credentials::has_jira();
    cfg.has_notion = credentials::has_notion();
    Ok(cfg)
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    load_config_native()
}

pub fn validate_repo_review_config_native(
    repo_path: &Path,
    review_profile: Option<&str>,
) -> Result<RepoReviewConfigLoadResult, String> {
    if review_profile
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .is_some()
    {
        repo_config::load_from_repo_path_with_profile(repo_path, review_profile)
    } else {
        repo_config::load_from_repo_path(repo_path)
    }
}

#[tauri::command]
pub fn validate_repo_review_config(
    repo_path: String,
    review_profile: Option<String>,
) -> Result<RepoReviewConfigLoadResult, String> {
    validate_repo_review_config_native(Path::new(&repo_path), review_profile.as_deref())
}

#[tauri::command]
pub fn save_config(
    repos: Vec<RepoRef>,
    review_provider: ReviewProvider,
    default_diff_view: String,
    theme: String,
    review_terminal: Option<ReviewTerminal>,
    ai_provider: AiProvider,
    claude_model: Option<String>,
    claude_effort: Option<String>,
    codex_model: Option<String>,
    codex_effort: Option<String>,
    jira_base_url: Option<String>,
    automatic_sync_interval_seconds: Option<u64>,
    menu_bar_sync_enabled: bool,
    notifications_enabled: bool,
) -> Result<(), String> {
    config::save(&AppConfig {
        repos,
        review_provider,
        default_diff_view,
        theme,
        review_terminal,
        ai_provider,
        claude_model,
        claude_effort,
        codex_model,
        codex_effort,
        jira_base_url,
        automatic_sync_interval_seconds,
        menu_bar_sync_enabled,
        notifications_enabled,
        configured: false,
        has_credentials: false,
        has_github_credentials: false,
        has_jira: false,
        has_notion: false,
        workspace: None,
        repo: None,
    })
}

#[tauri::command]
pub fn save_credentials(username: String, token: String) -> Result<(), String> {
    credentials::store(&Credentials { username, token })
}

#[tauri::command]
pub fn save_github_token(token: String) -> Result<(), String> {
    if token.trim().is_empty() {
        credentials::clear_github_token()
    } else {
        credentials::store_github_token(token.trim())
    }
}

#[tauri::command]
pub fn has_credentials() -> Result<bool, String> {
    Ok(credentials::has())
}

#[tauri::command]
pub fn clear_credentials() -> Result<(), String> {
    credentials::clear()
}

#[tauri::command]
pub fn save_jira_token(token: String) -> Result<(), String> {
    if token.trim().is_empty() {
        credentials::clear_jira_token()
    } else {
        credentials::store_jira_token(token.trim())
    }
}

#[tauri::command]
pub fn save_notion_token(token: String) -> Result<(), String> {
    if token.trim().is_empty() {
        credentials::clear_notion_token()
    } else {
        credentials::store_notion_token(token.trim())
    }
}

#[tauri::command]
pub async fn test_connection(
    provider: Option<ReviewProvider>,
    username: String,
    token: String,
) -> Result<WorkspaceUser, String> {
    run(move || match provider.unwrap_or_default() {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::new(Credentials { username, token })?;
            let user: BbUser = get_json(client.get(&format!("{BASE}/user")))?;
            Ok(WorkspaceUser {
                display_name: user.display_name,
                account_id: user.account_id,
            })
        }
        ReviewProvider::Github => {
            let client = GithubClient::new(token)?;
            let user: GhUser = github_get_json(client.get("https://api.github.com/user"))?;
            let (display_name, account_id) = gh_user_label(Some(user));
            Ok(WorkspaceUser {
                display_name,
                account_id,
            })
        }
    })
    .await
}

#[tauri::command]
pub async fn get_current_user(provider: Option<ReviewProvider>) -> Result<WorkspaceUser, String> {
    run(
        move || match provider.unwrap_or_else(|| config::load().review_provider) {
            ReviewProvider::Bitbucket => {
                let client = BitbucketClient::from_stored()?;
                let user: BbUser = get_json(client.get(&format!("{BASE}/user")))?;
                Ok(WorkspaceUser {
                    display_name: user.display_name,
                    account_id: user.account_id,
                })
            }
            ReviewProvider::Github => {
                let client = GithubClient::from_stored()?;
                let user: GhUser = github_get_json(client.get("https://api.github.com/user"))?;
                let (display_name, account_id) = gh_user_label(Some(user));
                Ok(WorkspaceUser {
                    display_name,
                    account_id,
                })
            }
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Commands — pull requests
// ---------------------------------------------------------------------------

pub fn list_pull_requests_native(
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
    opts: &ListPrOptions,
) -> Result<PullRequestPage, String> {
    match provider_for(provider, workspace, repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            fetch_pull_requests_page(&client, workspace, repo, opts)
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            fetch_github_pull_requests_page(&client, workspace, repo, opts)
        }
    }
}

#[tauri::command]
pub async fn list_pull_requests(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    opts: ListPrOptions,
) -> Result<PullRequestPage, String> {
    run(move || list_pull_requests_native(provider, &workspace, &repo, &opts)).await
}

#[tauri::command]
pub async fn list_closed_pr_metrics(
    repos: Vec<RepoRef>,
) -> Result<ClosedPrAnalyticsSnapshot, String> {
    run(move || {
        Ok(ClosedPrAnalyticsSnapshot {
            metrics: cached_closed_metrics_for_repos(&repos)?,
            synced_count: 0,
        })
    })
    .await
}

#[tauri::command]
pub async fn sync_closed_pr_metrics(
    repos: Vec<RepoRef>,
    options: ClosedPrAnalyticsOptions,
) -> Result<ClosedPrAnalyticsSnapshot, String> {
    run(move || {
        if repos.is_empty() {
            return Ok(ClosedPrAnalyticsSnapshot {
                metrics: Vec::new(),
                synced_count: 0,
            });
        }

        let limit = options.limit_per_state.unwrap_or(25).clamp(1, 100);
        let states = ["MERGED", "DECLINED", "SUPERSEDED"];
        let mut synced_count = 0;

        for repo_ref in &repos {
            for state in states {
                let opts = ListPrOptions {
                    state: Some(state.to_string()),
                    page: Some(1),
                    pagelen: Some(limit),
                    query: None,
                    updated_after: options.updated_after.clone(),
                };
                let page = match repo_ref.provider {
                    ReviewProvider::Bitbucket => {
                        let client = BitbucketClient::from_stored()?;
                        fetch_pull_requests_page(
                            &client,
                            &repo_ref.workspace,
                            &repo_ref.repo,
                            &opts,
                        )?
                    }
                    ReviewProvider::Github => {
                        let client = GithubClient::from_stored()?;
                        fetch_github_pull_requests_page(
                            &client,
                            &repo_ref.workspace,
                            &repo_ref.repo,
                            &opts,
                        )?
                    }
                };

                for pr in page.values {
                    let diffstat = match repo_ref.provider {
                        ReviewProvider::Bitbucket => {
                            let client = BitbucketClient::from_stored()?;
                            fetch_diffstat_entries(
                                &client,
                                &repo_ref.workspace,
                                &repo_ref.repo,
                                pr.id,
                            )
                        }
                        ReviewProvider::Github => {
                            let client = GithubClient::from_stored()?;
                            fetch_github_diffstat_entries(
                                &client,
                                &repo_ref.workspace,
                                &repo_ref.repo,
                                pr.id,
                            )
                        }
                    };
                    let (additions, deletions, files_changed, diffstat_cached) = match diffstat {
                        Ok(entries) => {
                            let additions = entries.iter().map(|entry| entry.lines_added).sum();
                            let deletions = entries.iter().map(|entry| entry.lines_removed).sum();
                            (additions, deletions, entries.len() as u32, true)
                        }
                        Err(error) => {
                            eprintln!(
                                "Failed to sync diffstat for {}/{} #{}: {}",
                                repo_ref.workspace, repo_ref.repo, pr.id, error
                            );
                            (0, 0, 0, false)
                        }
                    };
                    let risk = review_storage::review_risk_summary(
                        &repo_ref.workspace,
                        &repo_ref.repo,
                        pr.id,
                        additions,
                        deletions,
                        files_changed,
                    );
                    review_storage::upsert_closed_pr_metric(&ClosedPrMetric {
                        workspace: repo_ref.workspace.clone(),
                        repo: repo_ref.repo.clone(),
                        pr_id: pr.id,
                        title: pr.title,
                        author_display_name: pr.author_display_name,
                        author_account_id: pr.author_account_id,
                        state: pr.state,
                        source_branch: pr.source_branch,
                        destination_branch: pr.destination_branch,
                        created_on: pr.created_on,
                        updated_on: pr.updated_on,
                        additions,
                        deletions,
                        files_changed,
                        diffstat_cached,
                        risk,
                        synced_at: now_ms(),
                    })?;
                    synced_count += 1;
                }
            }
        }

        Ok(ClosedPrAnalyticsSnapshot {
            metrics: cached_closed_metrics_for_repos(&repos)?,
            synced_count,
        })
    })
    .await
}

#[tauri::command]
pub async fn get_pull_request(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
) -> Result<PullRequestDetail, String> {
    run(move || get_pull_request_native(provider, &workspace, &repo, id)).await
}

pub fn get_pull_request_native(
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<PullRequestDetail, String> {
    match provider_for(provider, workspace, repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            fetch_pull_request_detail(&client, workspace, repo, id)
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            fetch_github_pull_request_detail(&client, workspace, repo, id)
        }
    }
}

#[tauri::command]
pub async fn approve_pull_request(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
) -> Result<PullRequestDetail, String> {
    run(move || match provider_for(provider, &workspace, &repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            if !dry_run() {
                let url = format!(
                    "{}/pullrequests/{id}/approve",
                    repo_base(&workspace, &repo)?
                );
                send_checked(client.post(&url))?;
            } else {
                eprintln!("[dry-run] approve PR #{id}");
            }
            fetch_pull_request_detail(&client, &workspace, &repo, id)
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            if !dry_run() {
                let url = format!(
                    "{}/pulls/{id}/reviews",
                    github_repo_base(&workspace, &repo)?
                );
                github_send_checked(client.post(&url).json(&json!({ "event": "APPROVE" })))?;
            } else {
                eprintln!("[dry-run] approve GitHub PR #{id}");
            }
            fetch_github_pull_request_detail(&client, &workspace, &repo, id)
        }
    })
    .await
}

#[tauri::command]
pub async fn get_branch_status(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    source: String,
    destination: String,
) -> Result<BranchStatus, String> {
    run(move || match provider_for(provider, &workspace, &repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            let base = repo_base(&workspace, &repo)?;
            let (behind, behind_capped) =
                count_commits(&client, &base, &destination, &source, 100)?;
            let (ahead, ahead_capped) = count_commits(&client, &base, &source, &destination, 100)?;
            Ok(BranchStatus {
                behind,
                ahead,
                behind_capped,
                ahead_capped,
            })
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            let base = github_repo_base(&workspace, &repo)?;
            let compare = format!(
                "{base}/compare/{}...{}",
                encode_path_segment(&destination),
                encode_path_segment(&source)
            );
            let gh: GhCompare = github_get_json(client.get(&compare))?;
            Ok(BranchStatus {
                behind: gh.behind_by,
                ahead: gh.ahead_by,
                behind_capped: false,
                ahead_capped: false,
            })
        }
    })
    .await
}

#[tauri::command]
pub async fn get_diffstat(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
) -> Result<Vec<DiffstatEntry>, String> {
    run(move || match provider_for(provider, &workspace, &repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            fetch_diffstat_entries(&client, &workspace, &repo, id)
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            fetch_github_diffstat_entries(&client, &workspace, &repo, id)
        }
    })
    .await
}

#[tauri::command]
pub async fn get_pr_diff(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
) -> Result<String, String> {
    run(move || get_pr_diff_native(provider, &workspace, &repo, id)).await
}

pub fn get_pr_diff_native(
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<String, String> {
    match provider_for(provider, workspace, repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            let url = format!("{}/pullrequests/{id}/diff", repo_base(workspace, repo)?);
            let resp = send_checked(client.get(&url))?;
            resp.text().map_err(|e| e.to_string())
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            let url = format!("{}/pulls/{id}", github_repo_base(workspace, repo)?);
            let resp = github_send_checked(client.get_diff(&url))?;
            resp.text().map_err(|e| e.to_string())
        }
    }
}

fn required_commit_matches(left: &Option<String>, right: &Option<String>) -> bool {
    match (
        left.as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        right
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

fn validate_stable_pull_request_snapshot(
    before: &PullRequestDetail,
    after: &PullRequestDetail,
) -> Result<(), String> {
    let stable = before.id == after.id
        && before.source_branch == after.source_branch
        && before.destination_branch == after.destination_branch
        && required_commit_matches(&before.source_commit_hash, &after.source_commit_hash)
        && required_commit_matches(
            &before.destination_commit_hash,
            &after.destination_commit_hash,
        );
    if stable {
        Ok(())
    } else {
        Err(
            "The pull request changed while its review snapshot was loading; rerun the review."
                .to_string(),
        )
    }
}

pub fn get_stable_pull_request_review_snapshot_native(
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<PullRequestReviewSnapshot, String> {
    let before = get_pull_request_native(provider, workspace, repo, id)?;
    let diff = get_pr_diff_native(provider, workspace, repo, id)?;
    let after = get_pull_request_native(provider, workspace, repo, id)?;
    validate_stable_pull_request_snapshot(&before, &after)?;
    Ok(PullRequestReviewSnapshot {
        detail: after,
        diff,
    })
}

#[tauri::command]
pub async fn get_pr_file_preview(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
    path: String,
    side: String,
) -> Result<PrFilePreview, String> {
    run(move || match provider_for(provider, &workspace, &repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            let pr = fetch_pull_request_detail(&client, &workspace, &repo, id)?;
            let reference = if side == "old" {
                pr.destination_commit_hash.unwrap_or(pr.destination_branch)
            } else {
                pr.source_commit_hash.unwrap_or(pr.source_branch)
            };
            fetch_bitbucket_file_preview(&client, &workspace, &repo, &reference, &path)
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            let pr = fetch_github_pull_request_detail(&client, &workspace, &repo, id)?;
            let reference = if side == "old" {
                pr.destination_commit_hash.unwrap_or(pr.destination_branch)
            } else {
                pr.source_commit_hash.unwrap_or(pr.source_branch)
            };
            fetch_github_file_preview(&client, &workspace, &repo, &reference, &path)
        }
    })
    .await
}

// ---------------------------------------------------------------------------
// Commands — comments
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy)]
pub struct StoredProviderInlineCommentApi;

impl ProviderInlineCommentApi for StoredProviderInlineCommentApi {
    fn current_revision(
        &self,
        target: &ProviderPublicationTarget,
    ) -> Result<ProviderPullRequestRevision, ProviderPublicationApiError> {
        let detail = match target.provider {
            PullRequestReviewEventProvider::Bitbucket => {
                let client = BitbucketClient::from_stored().map_err(map_publication_auth_error)?;
                fetch_pull_request_detail(
                    &client,
                    &target.workspace,
                    &target.repository,
                    publication_pr_id(target.pull_request_id)?,
                )
                .map_err(map_publication_read_error)?
            }
            PullRequestReviewEventProvider::Github => {
                let client = GithubClient::from_stored().map_err(map_publication_auth_error)?;
                fetch_github_pull_request_detail(
                    &client,
                    &target.workspace,
                    &target.repository,
                    publication_pr_id(target.pull_request_id)?,
                )
                .map_err(map_publication_read_error)?
            }
        };
        let head_sha = detail.source_commit_hash.ok_or_else(|| {
            ProviderPublicationApiError::unavailable(
                "The provider did not return the pull request head commit.",
            )
        })?;
        let base_sha = detail.destination_commit_hash.ok_or_else(|| {
            ProviderPublicationApiError::unavailable(
                "The provider did not return the pull request destination commit.",
            )
        })?;
        Ok(ProviderPullRequestRevision { head_sha, base_sha })
    }

    fn find_inline_comment(
        &self,
        target: &ProviderPublicationTarget,
        marker: &str,
        expected: &ProviderInlineCommentPayload,
    ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
        find_published_finding_comment(target, marker, Some(expected))
    }

    fn find_comment_by_marker(
        &self,
        target: &ProviderPublicationTarget,
        marker: &str,
    ) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
        find_published_finding_comment(target, marker, None)
    }

    fn create_inline_comment(
        &self,
        target: &ProviderPublicationTarget,
        payload: &ProviderInlineCommentPayload,
    ) -> Result<ProviderCommentIdentity, ProviderPublicationApiError> {
        let pr_id = publication_pr_id(target.pull_request_id)?;
        let (comment_id, inline) = match target.provider {
            PullRequestReviewEventProvider::Bitbucket => {
                let client = BitbucketClient::from_stored().map_err(map_publication_auth_error)?;
                let url = format!(
                    "{}/pullrequests/{pr_id}/comments",
                    repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?
                );
                let body = bitbucket_publication_body(payload);
                let response = send_once_checked(client.post(&url).json(&body))
                    .map_err(map_publication_write_error)?;
                let comment: BbPublicationComment = response
                    .json()
                    .map_err(|error| ProviderPublicationApiError::unavailable(error.to_string()))?;
                (
                    publication_comment_id(comment.id)?,
                    comment
                        .inline
                        .as_ref()
                        .is_some_and(|anchor| bitbucket_anchor_matches(anchor, payload)),
                )
            }
            PullRequestReviewEventProvider::Github => {
                let client = GithubClient::from_stored().map_err(map_publication_auth_error)?;
                let base = github_repo_base(&target.workspace, &target.repository)
                    .map_err(ProviderPublicationApiError::unavailable)?;
                let body = github_publication_body(payload);
                let url = format!("{base}/pulls/{pr_id}/comments");
                let response = github_send_checked(client.post(&url).json(&body))
                    .map_err(map_publication_write_error)?;
                let comment: GhPublicationComment = response
                    .json()
                    .map_err(|error| ProviderPublicationApiError::unavailable(error.to_string()))?;
                let inline = github_anchor_matches(&comment, payload);
                (publication_comment_id(comment.id)?, inline)
            }
        };
        let identity = ProviderCommentIdentity { comment_id };
        if !inline {
            self.delete_comment(target, &identity)?;
            return Err(ProviderPublicationApiError::invalid_anchor(
                "The provider did not preserve the requested inline anchor; the comment was removed.",
            ));
        }
        Ok(identity)
    }

    fn delete_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
    ) -> Result<(), ProviderPublicationApiError> {
        let comment_id = encode_path_segment(&identity.comment_id);
        let result = match target.provider {
            PullRequestReviewEventProvider::Bitbucket => {
                let client = BitbucketClient::from_stored().map_err(map_publication_auth_error)?;
                let url = format!(
                    "{}/pullrequests/{}/comments/{comment_id}",
                    repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?,
                    publication_pr_id(target.pull_request_id)?
                );
                send_checked(client.delete(&url)).map(|_| ())
            }
            PullRequestReviewEventProvider::Github => {
                let client = GithubClient::from_stored().map_err(map_publication_auth_error)?;
                let url = format!(
                    "{}/pulls/comments/{comment_id}",
                    github_repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?
                );
                github_send_checked(client.delete(&url)).map(|_| ())
            }
        };
        match result {
            Ok(()) => Ok(()),
            Err(error) if error.contains("404 Not Found") => Ok(()),
            Err(error) => Err(map_publication_read_error(error)),
        }
    }
}

impl ProviderFindingReconciliationApi for StoredProviderInlineCommentApi {
    fn get_finding_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
    ) -> Result<Option<ProviderFindingComment>, ProviderPublicationApiError> {
        let pr_id = publication_pr_id(target.pull_request_id)?;
        let comment_id = encode_path_segment(&identity.comment_id);
        match target.provider {
            PullRequestReviewEventProvider::Bitbucket => {
                let client = BitbucketClient::from_stored().map_err(map_publication_auth_error)?;
                let user: BbUser = get_json(client.get(&format!("{BASE}/user")))
                    .map_err(map_publication_read_error)?;
                let author_account_id = user
                    .account_id
                    .filter(|account_id| !account_id.trim().is_empty())
                    .ok_or_else(|| {
                        ProviderPublicationApiError::unavailable(
                            "Bitbucket did not return the authenticated account identifier.",
                        )
                    })?;
                let url = format!(
                    "{}/pullrequests/{pr_id}/comments/{comment_id}",
                    repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?
                );
                let comment: BbPublicationComment = match get_json(client.get(&url)) {
                    Ok(comment) => comment,
                    Err(error) if error.contains("404 Not Found") => return Ok(None),
                    Err(error) => return Err(map_publication_read_error(error)),
                };
                if comment.deleted
                    || comment
                        .user
                        .as_ref()
                        .and_then(|user| user.account_id.as_deref())
                        != Some(author_account_id.as_str())
                {
                    return Ok(None);
                }
                provider_finding_comment_from_bitbucket(comment).map(Some)
            }
            PullRequestReviewEventProvider::Github => {
                let client = GithubClient::from_stored().map_err(map_publication_auth_error)?;
                let user: GhUser = github_get_json(client.get("https://api.github.com/user"))
                    .map_err(map_publication_read_error)?;
                let author_login = user.login.trim();
                if author_login.is_empty() {
                    return Err(ProviderPublicationApiError::unavailable(
                        "GitHub did not return the authenticated account login.",
                    ));
                }
                let url = format!(
                    "{}/pulls/comments/{comment_id}",
                    github_repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?
                );
                let comment: GhPublicationComment = match github_get_json(client.get(&url)) {
                    Ok(comment) => comment,
                    Err(error) if error.contains("404 Not Found") => return Ok(None),
                    Err(error) => return Err(map_publication_read_error(error)),
                };
                if !comment
                    .user
                    .as_ref()
                    .is_some_and(|user| user.login.eq_ignore_ascii_case(author_login))
                {
                    return Ok(None);
                }
                provider_finding_comment_from_github(comment).map(Some)
            }
        }
    }

    fn update_finding_comment(
        &self,
        target: &ProviderPublicationTarget,
        identity: &ProviderCommentIdentity,
        markdown: &str,
    ) -> Result<(), ProviderPublicationApiError> {
        let existing = self.get_finding_comment(target, identity)?.ok_or_else(|| {
            ProviderPublicationApiError::permission_denied(
                "The tracked finding comment is missing or belongs to another author.",
            )
        })?;
        if !comment_has_lachesi_finding_marker(&existing.markdown) {
            return Err(ProviderPublicationApiError::permission_denied(
                "The tracked provider comment is not a Lachesi finding.",
            ));
        }

        let pr_id = publication_pr_id(target.pull_request_id)?;
        let comment_id = encode_path_segment(&identity.comment_id);
        let (updated_id, updated_markdown) = match target.provider {
            PullRequestReviewEventProvider::Bitbucket => {
                let client = BitbucketClient::from_stored().map_err(map_publication_auth_error)?;
                let url = format!(
                    "{}/pullrequests/{pr_id}/comments/{comment_id}",
                    repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?
                );
                let response = send_checked(
                    client
                        .put(&url)
                        .json(&bitbucket_reconciliation_update_body(markdown)),
                )
                .map_err(map_publication_write_error)?;
                let comment: BbPublicationComment = response
                    .json()
                    .map_err(|error| ProviderPublicationApiError::unavailable(error.to_string()))?;
                (
                    publication_comment_id(comment.id)?,
                    comment
                        .content
                        .map(|content| content.raw)
                        .unwrap_or_default(),
                )
            }
            PullRequestReviewEventProvider::Github => {
                let client = GithubClient::from_stored().map_err(map_publication_auth_error)?;
                let url = format!(
                    "{}/pulls/comments/{comment_id}",
                    github_repo_base(&target.workspace, &target.repository)
                        .map_err(ProviderPublicationApiError::unavailable)?
                );
                let response = github_send_checked(
                    client
                        .patch(&url)
                        .json(&github_reconciliation_update_body(markdown)),
                )
                .map_err(map_publication_write_error)?;
                let comment: GhPublicationComment = response
                    .json()
                    .map_err(|error| ProviderPublicationApiError::unavailable(error.to_string()))?;
                (publication_comment_id(comment.id)?, comment.body)
            }
        };
        if updated_id != identity.comment_id || updated_markdown != markdown {
            return Err(ProviderPublicationApiError::unavailable(
                "The provider did not preserve the reconciled finding comment.",
            ));
        }
        Ok(())
    }
}

fn provider_finding_comment_from_bitbucket(
    comment: BbPublicationComment,
) -> Result<ProviderFindingComment, ProviderPublicationApiError> {
    let identity = ProviderCommentIdentity {
        comment_id: publication_comment_id(comment.id)?,
    };
    let markdown = comment.content.map(|content| content.raw).ok_or_else(|| {
        ProviderPublicationApiError::invalid_anchor(
            "The tracked Bitbucket finding has no comment body.",
        )
    })?;
    let anchor = comment
        .inline
        .as_ref()
        .and_then(bitbucket_finding_anchor)
        .ok_or_else(|| {
            ProviderPublicationApiError::invalid_anchor(
                "The tracked Bitbucket finding no longer has a valid inline anchor.",
            )
        })?;
    Ok(ProviderFindingComment {
        identity,
        markdown,
        anchor,
    })
}

fn provider_finding_comment_from_github(
    comment: GhPublicationComment,
) -> Result<ProviderFindingComment, ProviderPublicationApiError> {
    let anchor = github_finding_anchor(&comment).ok_or_else(|| {
        ProviderPublicationApiError::invalid_anchor(
            "The tracked GitHub finding no longer has a valid inline anchor.",
        )
    })?;
    let identity = ProviderCommentIdentity {
        comment_id: publication_comment_id(comment.id)?,
    };
    Ok(ProviderFindingComment {
        identity,
        markdown: comment.body,
        anchor,
    })
}

fn bitbucket_finding_anchor(anchor: &BbPublicationInline) -> Option<FindingLineRange> {
    if let Some(end_line) = anchor.to {
        let start_line = anchor.start_to.unwrap_or(end_line);
        return (start_line > 0 && start_line <= end_line).then(|| FindingLineRange {
            path: anchor.path.clone(),
            start_line,
            end_line,
            side: FindingAnchorSide::New,
        });
    }
    let end_line = anchor.from?;
    let start_line = anchor.start_from.unwrap_or(end_line);
    (start_line > 0 && start_line <= end_line).then(|| FindingLineRange {
        path: anchor.path.clone(),
        start_line,
        end_line,
        side: FindingAnchorSide::Old,
    })
}

fn github_finding_anchor(comment: &GhPublicationComment) -> Option<FindingLineRange> {
    let path = comment
        .path
        .clone()
        .filter(|path| !path.trim().is_empty())?;
    let old_side = comment
        .side
        .as_deref()
        .is_some_and(|side| side.eq_ignore_ascii_case("LEFT"));
    let (end_line, start_line, side) = if old_side {
        let end_line = comment.original_line.or(comment.line)?;
        (
            end_line,
            comment
                .original_start_line
                .or(comment.start_line)
                .unwrap_or(end_line),
            FindingAnchorSide::Old,
        )
    } else {
        let end_line = comment.line.or(comment.original_line)?;
        (
            end_line,
            comment
                .start_line
                .or(comment.original_start_line)
                .unwrap_or(end_line),
            FindingAnchorSide::New,
        )
    };
    (start_line > 0 && start_line <= end_line).then_some(FindingLineRange {
        path,
        start_line,
        end_line,
        side,
    })
}

fn comment_has_lachesi_finding_marker(markdown: &str) -> bool {
    let controls = markdown
        .lines()
        .rev()
        .skip_while(|line| line.is_empty())
        .take_while(|line| line.starts_with("<!-- lachesi:") && line.ends_with(" -->"))
        .collect::<Vec<_>>();
    controls.iter().any(|line| {
        line.strip_prefix("<!-- lachesi:finding:")
            .and_then(|value| value.strip_suffix(" -->"))
            .is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    })
}

fn bitbucket_reconciliation_update_body(markdown: &str) -> serde_json::Value {
    json!({ "content": { "raw": markdown } })
}

fn github_reconciliation_update_body(markdown: &str) -> serde_json::Value {
    json!({ "body": markdown })
}

pub fn reconcile_review_findings_native(
    request: &FindingReconciliationRequest,
) -> Result<FindingReconciliationSummary, FindingPublicationError> {
    FindingReconciler::new(
        StoredProviderInlineCommentApi,
        SqliteFindingPublicationStore,
    )
    .reconcile(request)
}

pub fn publish_review_finding_native(
    request: &FindingPublicationRequest,
) -> Result<PublishedCommentIdentity, FindingPublicationError> {
    if dry_run() {
        eprintln!(
            "[dry-run] structured inline finding on PR #{} {}:{}-{}",
            request.pull_request_id,
            request.anchor.path,
            request.anchor.start_line,
            request.anchor.end_line
        );
        return dry_run_publication_identity(request);
    }
    FindingPublisher::new(
        StoredProviderInlineCommentApi,
        SqliteFindingPublicationStore,
    )
    .publish(request)
}

#[tauri::command]
pub async fn publish_review_finding(
    request: FindingPublicationRequest,
) -> Result<PublishedCommentIdentity, String> {
    tauri::async_runtime::spawn_blocking(move || publish_review_finding_native(&request))
        .await
        .map_err(|_| {
            publication_ipc_error(FindingPublicationError {
                code: FindingPublicationErrorCode::ProviderUnavailable,
                retryable: true,
                message: "The finding publication worker stopped unexpectedly.".to_string(),
            })
        })?
        .map_err(publication_ipc_error)
}

fn publication_ipc_error(error: FindingPublicationError) -> String {
    serde_json::to_string(&error).unwrap_or(error.message)
}

fn find_published_finding_comment(
    target: &ProviderPublicationTarget,
    marker: &str,
    expected: Option<&ProviderInlineCommentPayload>,
) -> Result<Option<ProviderCommentIdentity>, ProviderPublicationApiError> {
    let pr_id = publication_pr_id(target.pull_request_id)?;
    match target.provider {
        PullRequestReviewEventProvider::Bitbucket => {
            let client = BitbucketClient::from_stored().map_err(map_publication_auth_error)?;
            let user: BbUser = get_json(client.get(&format!("{BASE}/user")))
                .map_err(map_publication_read_error)?;
            let author_account_id = user
                .account_id
                .filter(|account_id| !account_id.trim().is_empty())
                .ok_or_else(|| {
                    ProviderPublicationApiError::unavailable(
                        "Bitbucket did not return the authenticated account identifier.",
                    )
                })?;
            let comments_endpoint = format!(
                "{}/pullrequests/{pr_id}/comments",
                repo_base(&target.workspace, &target.repository)
                    .map_err(ProviderPublicationApiError::unavailable)?
            );
            let mut url = format!(
                "{comments_endpoint}?pagelen=100&fields={BITBUCKET_PUBLICATION_COMMENT_FIELDS}"
            );
            loop {
                let page: BbPublicationCommentPage =
                    get_json(client.get(&url)).map_err(map_publication_read_error)?;
                if let Some(comment) = page.values.into_iter().find(|comment| {
                    bitbucket_publication_comment_matches(
                        comment,
                        marker,
                        expected,
                        &author_account_id,
                    )
                }) {
                    return Ok(Some(ProviderCommentIdentity {
                        comment_id: publication_comment_id(comment.id)?,
                    }));
                }
                match page.next {
                    Some(next) => {
                        url = safe_bitbucket_publication_next_url(&next, &comments_endpoint)
                            .map_err(map_publication_read_error)?;
                    }
                    None => return Ok(None),
                }
            }
        }
        PullRequestReviewEventProvider::Github => {
            let client = GithubClient::from_stored().map_err(map_publication_auth_error)?;
            let user: GhUser = github_get_json(client.get("https://api.github.com/user"))
                .map_err(map_publication_read_error)?;
            let author_login = user.login.trim();
            if author_login.is_empty() {
                return Err(ProviderPublicationApiError::unavailable(
                    "GitHub did not return the authenticated account login.",
                ));
            }
            let url = format!(
                "{}/pulls/{pr_id}/comments?per_page=100&page=1",
                github_repo_base(&target.workspace, &target.repository)
                    .map_err(ProviderPublicationApiError::unavailable)?
            );
            let comments: Vec<GhPublicationComment> =
                github_paginated_get(&client, url).map_err(map_publication_read_error)?;
            comments
                .into_iter()
                .find(|comment| {
                    github_publication_comment_matches(comment, marker, expected, author_login)
                })
                .map(|comment| {
                    Ok(ProviderCommentIdentity {
                        comment_id: publication_comment_id(comment.id)?,
                    })
                })
                .transpose()
        }
    }
}

fn safe_bitbucket_publication_next_url(
    value: &str,
    comments_endpoint: &str,
) -> Result<String, String> {
    let next = reqwest::Url::parse(value)
        .map_err(|_| "Bitbucket returned an invalid pagination URL.".to_string())?;
    let expected = reqwest::Url::parse(comments_endpoint)
        .map_err(|_| "The Bitbucket comments endpoint is invalid.".to_string())?;
    let same_endpoint = next.scheme() == "https"
        && next.scheme() == expected.scheme()
        && next.host_str() == expected.host_str()
        && next.port_or_known_default() == expected.port_or_known_default()
        && next.path() == expected.path()
        && next.username().is_empty()
        && next.password().is_none()
        && next.fragment().is_none()
        && next.query().is_some();
    if !same_endpoint {
        return Err("Bitbucket returned an unsafe pagination URL.".to_string());
    }
    Ok(value.to_string())
}

fn publication_comment_id(value: serde_json::Value) -> Result<String, ProviderPublicationApiError> {
    comment_id_from_value(value).map_err(ProviderPublicationApiError::unavailable)
}

fn publication_pr_id(pull_request_id: u64) -> Result<u32, ProviderPublicationApiError> {
    u32::try_from(pull_request_id).map_err(|_| {
        ProviderPublicationApiError::invalid_anchor(
            "The pull request identifier is not supported by this provider adapter.",
        )
    })
}

fn bitbucket_publication_body(payload: &ProviderInlineCommentPayload) -> serde_json::Value {
    let mut inline = serde_json::Map::new();
    inline.insert("path".into(), json!(payload.path));
    match payload.side {
        FindingAnchorSide::New => {
            inline.insert("to".into(), json!(payload.end_line));
            if payload.start_line < payload.end_line {
                inline.insert("start_to".into(), json!(payload.start_line));
            }
        }
        FindingAnchorSide::Old => {
            inline.insert("from".into(), json!(payload.end_line));
            if payload.start_line < payload.end_line {
                inline.insert("start_from".into(), json!(payload.start_line));
            }
        }
    }
    json!({
        "content": { "raw": payload.markdown },
        "inline": inline,
    })
}

fn github_publication_body(payload: &ProviderInlineCommentPayload) -> serde_json::Value {
    let side = match payload.side {
        FindingAnchorSide::New => "RIGHT",
        FindingAnchorSide::Old => "LEFT",
    };
    let mut body = json!({
        "body": payload.markdown,
        "commit_id": payload.head_sha,
        "path": payload.path,
        "line": payload.end_line,
        "side": side,
    });
    if payload.start_line < payload.end_line {
        body["start_line"] = json!(payload.start_line);
        body["start_side"] = json!(side);
    }
    body
}

fn bitbucket_anchor_matches(
    anchor: &BbPublicationInline,
    payload: &ProviderInlineCommentPayload,
) -> bool {
    if anchor.path != payload.path {
        return false;
    }
    match payload.side {
        FindingAnchorSide::New => {
            anchor.to == Some(payload.end_line)
                && if payload.start_line < payload.end_line {
                    anchor.start_to == Some(payload.start_line)
                } else {
                    anchor.start_to.is_none()
                }
        }
        FindingAnchorSide::Old => {
            anchor.from == Some(payload.end_line)
                && if payload.start_line < payload.end_line {
                    anchor.start_from == Some(payload.start_line)
                } else {
                    anchor.start_from.is_none()
                }
        }
    }
}

fn github_anchor_matches(
    comment: &GhPublicationComment,
    payload: &ProviderInlineCommentPayload,
) -> bool {
    let side = match payload.side {
        FindingAnchorSide::New => "RIGHT",
        FindingAnchorSide::Old => "LEFT",
    };
    let (line, start_line) = match payload.side {
        FindingAnchorSide::New => (comment.line, comment.start_line),
        FindingAnchorSide::Old => (
            comment.original_line.or(comment.line),
            comment.original_start_line.or(comment.start_line),
        ),
    };
    comment.path.as_deref() == Some(payload.path.as_str())
        && line == Some(payload.end_line)
        && comment
            .side
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(side))
        && if payload.start_line < payload.end_line {
            start_line == Some(payload.start_line)
                && comment
                    .start_side
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(side))
        } else {
            comment.start_line.is_none()
        }
}

fn bitbucket_publication_comment_matches(
    comment: &BbPublicationComment,
    marker: &str,
    expected: Option<&ProviderInlineCommentPayload>,
    author_account_id: &str,
) -> bool {
    !comment.deleted
        && comment
            .user
            .as_ref()
            .and_then(|user| user.account_id.as_deref())
            == Some(author_account_id)
        && expected
            .map(|expected| {
                comment
                    .inline
                    .as_ref()
                    .is_some_and(|anchor| bitbucket_anchor_matches(anchor, expected))
            })
            .unwrap_or(true)
        && comment
            .content
            .as_ref()
            .is_some_and(|content| comment_has_marker(&content.raw, marker))
}

fn github_publication_comment_matches(
    comment: &GhPublicationComment,
    marker: &str,
    expected: Option<&ProviderInlineCommentPayload>,
    author_login: &str,
) -> bool {
    comment
        .user
        .as_ref()
        .is_some_and(|user| user.login.eq_ignore_ascii_case(author_login))
        && expected
            .map(|expected| github_anchor_matches(comment, expected))
            .unwrap_or(true)
        && comment_has_marker(&comment.body, marker)
}

fn comment_has_marker(body: &str, marker: &str) -> bool {
    body.lines().next_back() == Some(marker)
}

fn map_publication_auth_error(error: String) -> ProviderPublicationApiError {
    ProviderPublicationApiError::permission_denied(error)
}

fn map_publication_read_error(error: String) -> ProviderPublicationApiError {
    if publication_rate_limit_error(&error) {
        ProviderPublicationApiError::unavailable(error)
    } else if publication_permission_error(&error) {
        ProviderPublicationApiError::permission_denied(error)
    } else if error.contains("404 Not Found") {
        ProviderPublicationApiError::invalid_anchor(error)
    } else {
        ProviderPublicationApiError::unavailable(error)
    }
}

fn map_publication_write_error(error: String) -> ProviderPublicationApiError {
    if publication_rate_limit_error(&error) {
        ProviderPublicationApiError::unavailable(error)
    } else if publication_permission_error(&error) {
        ProviderPublicationApiError::permission_denied(error)
    } else if error.contains("400 Bad Request")
        || error.contains("404 Not Found")
        || error.contains("409 Conflict")
        || error.contains("422 Unprocessable Entity")
    {
        ProviderPublicationApiError::invalid_anchor(error)
    } else {
        ProviderPublicationApiError::unavailable(error)
    }
}

fn publication_permission_error(error: &str) -> bool {
    error.contains("401 Unauthorized") || error.contains("403 Forbidden")
}

fn publication_rate_limit_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("rate limit")
        || lower.contains("429 too many requests")
        || lower.contains("spam")
        || lower.contains("abuse")
}

#[tauri::command]
pub async fn list_comments(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
) -> Result<Vec<PrComment>, String> {
    run(move || list_comments_native(provider, &workspace, &repo, id)).await
}

pub fn list_comments_native(
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
    id: u32,
) -> Result<Vec<PrComment>, String> {
    match provider_for(provider, workspace, repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            let mut url = format!(
                "{}/pullrequests/{id}/comments?pagelen=100&fields=next,values.id,values.created_on,values.deleted,values.content.raw,values.content.html,values.user.display_name,values.inline.path,values.inline.to,values.inline.from,values.parent.id",
                repo_base(workspace, repo)?
            );
            let mut out = Vec::new();
            loop {
                let page: BbCommentPage = get_json(client.get(&url))?;
                out.extend(page.values.into_iter().map(map_comment));
                match page.next {
                    Some(next) => url = next,
                    None => break,
                }
            }
            Ok(out)
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            fetch_github_comments(&client, workspace, repo, id)
        }
    }
}

#[tauri::command]
pub async fn create_inline_comment(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
    req: NewInlineComment,
) -> Result<PrComment, String> {
    run(move || {
        if dry_run() {
            eprintln!(
                "[dry-run] inline comment on PR #{id} {}: {}",
                req.path, req.raw
            );
            return Ok(PrComment {
                id: "dry-run".to_string(),
                parent_id: req.parent_id,
                content_raw: req.raw,
                content_html: None,
                user_display_name: "dry-run".to_string(),
                created_on: String::new(),
                deleted: false,
                inline: Some(InlineAnchor {
                    path: req.path,
                    to: req.to,
                    from: req.from,
                }),
            });
        }
        match provider_for(provider, &workspace, &repo) {
            ReviewProvider::Bitbucket => {
                let client = BitbucketClient::from_stored()?;
                let url = format!(
                    "{}/pullrequests/{id}/comments",
                    repo_base(&workspace, &repo)?
                );
                let mut inline = serde_json::Map::new();
                inline.insert("path".into(), json!(req.path));
                if let Some(to) = req.to {
                    inline.insert("to".into(), json!(to));
                }
                if let Some(from) = req.from {
                    inline.insert("from".into(), json!(from));
                }
                let mut body = json!({ "content": { "raw": req.raw }, "inline": inline });
                if let Some(parent_id) = req.parent_id.as_deref() {
                    body["parent"] = json!({ "id": bitbucket_comment_id_number(parent_id)? });
                }
                let bb: BbComment = get_json(client.post(&url).json(&body))?;
                Ok(map_comment(bb))
            }
            ReviewProvider::Github => {
                let client = GithubClient::from_stored()?;
                let base = github_repo_base(&workspace, &repo)?;
                if let Some(parent_id) = req.parent_id {
                    let parent_id = encode_path_segment(&parent_id);
                    let url = format!("{base}/pulls/{id}/comments/{parent_id}/replies");
                    let comment: GhReviewComment =
                        github_get_json(client.post(&url).json(&json!({ "body": req.raw })))?;
                    return Ok(map_gh_review_comment(comment));
                }
                let line = req
                    .to
                    .or(req.from)
                    .ok_or_else(|| "GitHub inline comments require a target line.".to_string())?;
                let side = if req.to.is_some() { "RIGHT" } else { "LEFT" };
                let commit_id = fetch_github_head_sha(&client, &workspace, &repo, id)?;
                let url = format!("{base}/pulls/{id}/comments");
                let body = json!({
                    "body": req.raw,
                    "commit_id": commit_id,
                    "path": req.path,
                    "line": line,
                    "side": side,
                });
                let comment: GhReviewComment = github_get_json(client.post(&url).json(&body))?;
                Ok(map_gh_review_comment(comment))
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn create_general_comment(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
    raw: String,
    parent_id: Option<String>,
) -> Result<PrComment, String> {
    run(move || create_general_comment_native(provider, &workspace, &repo, id, raw, parent_id))
        .await
}

pub fn create_general_comment_native(
    provider: Option<ReviewProvider>,
    workspace: &str,
    repo: &str,
    id: u32,
    raw: String,
    parent_id: Option<String>,
) -> Result<PrComment, String> {
    if dry_run() {
        eprintln!("[dry-run] general comment on PR #{id}: {raw}");
        return Ok(PrComment {
            id: "dry-run".to_string(),
            parent_id,
            content_raw: raw,
            content_html: None,
            user_display_name: "dry-run".to_string(),
            created_on: String::new(),
            deleted: false,
            inline: None,
        });
    }
    match provider_for(provider, workspace, repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            let url = format!("{}/pullrequests/{id}/comments", repo_base(workspace, repo)?);
            let mut body = json!({ "content": { "raw": raw } });
            if let Some(parent_id) = parent_id.as_deref() {
                body["parent"] = json!({ "id": bitbucket_comment_id_number(parent_id)? });
            }
            let bb: BbComment = get_json(client.post(&url).json(&body))?;
            Ok(map_comment(bb))
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            let base = github_repo_base(workspace, repo)?;
            if let Some(parent_id) = parent_id {
                let parent_id = encode_path_segment(&parent_id);
                let url = format!("{base}/pulls/{id}/comments/{parent_id}/replies");
                if let Ok(comment) = github_get_json::<GhReviewComment>(
                    client.post(&url).json(&json!({ "body": raw })),
                ) {
                    return Ok(map_gh_review_comment(comment));
                }
            }
            let url = format!("{base}/issues/{id}/comments");
            let comment: GhIssueComment =
                github_get_json(client.post(&url).json(&json!({ "body": raw })))?;
            Ok(map_gh_issue_comment(comment))
        }
    }
}

#[tauri::command]
pub async fn delete_comment(
    provider: Option<ReviewProvider>,
    workspace: String,
    repo: String,
    id: u32,
    comment_id: String,
) -> Result<(), String> {
    run(move || match provider_for(provider, &workspace, &repo) {
        ReviewProvider::Bitbucket => {
            let client = BitbucketClient::from_stored()?;
            let comment_id = encode_path_segment(&comment_id);
            let url = format!(
                "{}/pullrequests/{id}/comments/{comment_id}",
                repo_base(&workspace, &repo)?
            );
            send_checked(client.delete(&url))?;
            Ok(())
        }
        ReviewProvider::Github => {
            let client = GithubClient::from_stored()?;
            let base = github_repo_base(&workspace, &repo)?;
            let comment_id = encode_path_segment(&comment_id);
            let review_url = format!("{base}/pulls/comments/{comment_id}");
            match github_send_checked(client.delete(&review_url)) {
                Ok(_) => Ok(()),
                Err(_) => {
                    let issue_url = format!("{base}/issues/comments/{comment_id}");
                    github_send_checked(client.delete(&issue_url))?;
                    Ok(())
                }
            }
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn publication_payload(side: FindingAnchorSide) -> ProviderInlineCommentPayload {
        ProviderInlineCommentPayload {
            head_sha: "2222222222222222222222222222222222222222".to_string(),
            path: "src/lib.rs".to_string(),
            start_line: 12,
            end_line: 14,
            side,
            markdown: "finding\n\n<!-- lachesi:finding:abc -->".to_string(),
        }
    }

    #[test]
    fn provider_publication_bodies_keep_inline_anchor_semantics() {
        for field in [
            "values.user.account_id",
            "values.inline.path",
            "values.inline.to",
            "values.inline.from",
            "values.inline.start_to",
            "values.inline.start_from",
        ] {
            assert!(BITBUCKET_PUBLICATION_COMMENT_FIELDS
                .split(',')
                .any(|current| current == field));
        }
        let github = github_publication_body(&publication_payload(FindingAnchorSide::New));
        assert_eq!(
            github["commit_id"],
            "2222222222222222222222222222222222222222"
        );
        assert_eq!(github["path"], "src/lib.rs");
        assert_eq!(github["start_line"], 12);
        assert_eq!(github["start_side"], "RIGHT");
        assert_eq!(github["line"], 14);
        assert_eq!(github["side"], "RIGHT");

        let bitbucket = bitbucket_publication_body(&publication_payload(FindingAnchorSide::Old));
        assert_eq!(bitbucket["inline"]["path"], "src/lib.rs");
        assert_eq!(bitbucket["inline"]["start_from"], 12);
        assert_eq!(bitbucket["inline"]["from"], 14);
        assert!(bitbucket["inline"].get("to").is_none());
        assert!(bitbucket.get("content").is_some());

        let bitbucket_page: BbPublicationCommentPage = serde_json::from_value(json!({
            "values": [{
                "id": 99,
                "deleted": false,
                "user": { "account_id": "reviewer-1" },
                "content": {
                    "raw": "finding\n\n<!-- lachesi:finding:abc -->"
                },
                "inline": {
                    "path": "src/lib.rs",
                    "from": 14,
                    "start_from": 12
                }
            }]
        }))
        .expect("filtered Bitbucket publication page");
        let bitbucket_response = bitbucket_page.values[0]
            .inline
            .as_ref()
            .expect("inline anchor");
        assert!(bitbucket_anchor_matches(
            bitbucket_response,
            &publication_payload(FindingAnchorSide::Old)
        ));
        let orphan: BbPublicationComment = serde_json::from_value(json!({
            "id": 100,
            "deleted": false,
            "user": { "account_id": "reviewer-1" },
            "content": {
                "raw": "finding\n\n<!-- lachesi:finding:abc -->"
            }
        }))
        .expect("marker-only Bitbucket comment");
        assert!(bitbucket_publication_comment_matches(
            &orphan,
            "<!-- lachesi:finding:abc -->",
            None,
            "reviewer-1"
        ));
        assert!(!bitbucket_publication_comment_matches(
            &orphan,
            "<!-- lachesi:finding:abc -->",
            Some(&publication_payload(FindingAnchorSide::Old)),
            "reviewer-1"
        ));
        assert!(!bitbucket_publication_comment_matches(
            &orphan,
            "<!-- lachesi:finding:abc -->",
            None,
            "another-reviewer"
        ));
        let github_response = GhPublicationComment {
            id: json!(99),
            body: "finding\n\n<!-- lachesi:finding:abc -->".to_string(),
            user: Some(GhUser {
                login: "reviewer-1".to_string(),
                name: None,
            }),
            path: Some("src/lib.rs".to_string()),
            line: Some(14),
            original_line: None,
            start_line: Some(12),
            original_start_line: None,
            side: Some("RIGHT".to_string()),
            start_side: Some("RIGHT".to_string()),
        };
        assert!(github_anchor_matches(
            &github_response,
            &publication_payload(FindingAnchorSide::New)
        ));
        assert!(github_publication_comment_matches(
            &github_response,
            "<!-- lachesi:finding:abc -->",
            Some(&publication_payload(FindingAnchorSide::New)),
            "REVIEWER-1"
        ));
        let github_old_response = GhPublicationComment {
            id: json!(100),
            body: "finding\n\n<!-- lachesi:finding:def -->".to_string(),
            user: Some(GhUser {
                login: "reviewer-1".to_string(),
                name: None,
            }),
            path: Some("src/lib.rs".to_string()),
            line: None,
            original_line: Some(14),
            start_line: None,
            original_start_line: Some(12),
            side: Some("LEFT".to_string()),
            start_side: Some("LEFT".to_string()),
        };
        assert!(github_anchor_matches(
            &github_old_response,
            &publication_payload(FindingAnchorSide::Old)
        ));
        assert!(!github_publication_comment_matches(
            &github_response,
            "<!-- lachesi:finding:abc -->",
            None,
            "another-reviewer"
        ));
        let mut mismatched = publication_payload(FindingAnchorSide::New);
        mismatched.end_line = 15;
        assert!(!github_anchor_matches(&github_response, &mismatched));
        assert!(comment_has_marker(
            "body\n\n<!-- lachesi:finding:abc -->",
            "<!-- lachesi:finding:abc -->"
        ));
        assert!(!comment_has_marker(
            "<!-- lachesi:finding:abc -->\nextra",
            "<!-- lachesi:finding:abc -->"
        ));
    }

    #[test]
    fn provider_publication_comment_ids_are_not_limited_to_u32() {
        let id = publication_comment_id(json!(9_223_372_036_854_775_000_u64))
            .expect("large provider comment id");
        assert_eq!(id, "9223372036854775000");
        assert_eq!(
            publication_comment_id(json!("opaque-comment-id")).expect("opaque id"),
            "opaque-comment-id"
        );
    }

    #[test]
    fn reconciliation_comment_mappers_preserve_provider_ranges_and_bodies() {
        let markdown =
            "finding\n\n<!-- lachesi:finding-lineage:abc -->\n<!-- lachesi:finding:def -->";
        let bitbucket: BbPublicationComment = serde_json::from_value(json!({
            "id": 9_223_372_036_854_775_000_u64,
            "content": { "raw": markdown },
            "inline": {
                "path": "src/lib.rs",
                "start_to": 12,
                "to": 14
            }
        }))
        .expect("Bitbucket reconciliation comment");
        let bitbucket =
            provider_finding_comment_from_bitbucket(bitbucket).expect("Bitbucket mapping");
        assert_eq!(bitbucket.identity.comment_id, "9223372036854775000");
        assert_eq!(
            bitbucket.anchor,
            FindingLineRange {
                path: "src/lib.rs".to_string(),
                start_line: 12,
                end_line: 14,
                side: FindingAnchorSide::New,
            }
        );
        assert_eq!(bitbucket.markdown, markdown);

        let github: GhPublicationComment = serde_json::from_value(json!({
            "id": "opaque-comment",
            "body": markdown,
            "path": "src/lib.rs",
            "original_start_line": 20,
            "original_line": 22,
            "side": "LEFT"
        }))
        .expect("GitHub reconciliation comment");
        let github = provider_finding_comment_from_github(github).expect("GitHub mapping");
        assert_eq!(github.identity.comment_id, "opaque-comment");
        assert_eq!(
            github.anchor,
            FindingLineRange {
                path: "src/lib.rs".to_string(),
                start_line: 20,
                end_line: 22,
                side: FindingAnchorSide::Old,
            }
        );
        assert_eq!(github.markdown, markdown);
    }

    #[test]
    fn reconciliation_updates_only_marker_owned_comment_bodies() {
        let lineage = "a".repeat(64);
        let exact = "b".repeat(64);
        let markdown = format!(
            "finding\n\n<!-- lachesi:finding-lineage:{lineage} -->\n<!-- lachesi:finding:{exact} -->"
        );
        assert!(comment_has_lachesi_finding_marker(&markdown));
        assert!(!comment_has_lachesi_finding_marker(
            "<!-- lachesi:finding-state:resolved -->"
        ));
        assert!(!comment_has_lachesi_finding_marker(
            "<!-- lachesi:finding-lineage:abc -->"
        ));
        assert!(comment_has_lachesi_finding_marker(&format!(
            "legacy finding\n\n<!-- lachesi:finding:{exact} -->"
        )));
        assert!(!comment_has_lachesi_finding_marker(&format!(
            "<!-- lachesi:finding:{exact} -->\ncontent"
        )));
        assert_eq!(
            bitbucket_reconciliation_update_body(&markdown),
            json!({ "content": { "raw": markdown } })
        );
        assert_eq!(
            github_reconciliation_update_body(&markdown),
            json!({ "body": markdown })
        );
    }

    #[test]
    fn normal_comment_dtos_preserve_large_and_opaque_provider_ids() {
        let github: GhReviewComment = serde_json::from_value(json!({
            "id": 9_223_372_036_854_775_000_u64,
            "body": "review",
            "path": "src/lib.rs",
            "line": 12,
            "side": "RIGHT",
            "in_reply_to_id": "opaque-parent"
        }))
        .expect("GitHub review comment");
        let mapped = map_gh_review_comment(github);
        assert_eq!(mapped.id, "9223372036854775000");
        assert_eq!(mapped.parent_id.as_deref(), Some("opaque-parent"));

        let bitbucket: BbComment = serde_json::from_value(json!({
            "id": 9_223_372_036_854_775_000_u64,
            "parent": { "id": "9223372036854774999" }
        }))
        .expect("Bitbucket comment");
        let mapped = map_comment(bitbucket);
        assert_eq!(mapped.id, "9223372036854775000");
        assert_eq!(mapped.parent_id.as_deref(), Some("9223372036854774999"));
    }

    #[test]
    fn bitbucket_publication_post_is_not_retried_after_a_server_error() {
        for status in [
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(!should_retry_bitbucket_request(
                BitbucketRetryPolicy::AtMostOnce,
                status,
                0
            ));
            assert!(should_retry_bitbucket_request(
                BitbucketRetryPolicy::RetryTransient,
                status,
                0
            ));
        }
        assert!(!should_retry_bitbucket_request(
            BitbucketRetryPolicy::RetryTransient,
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            3
        ));
    }

    #[test]
    fn provider_repository_segments_are_percent_encoded() {
        assert_eq!(
            repo_base("team/name", "payments?#").expect("encoded Bitbucket repository path"),
            "https://api.bitbucket.org/2.0/repositories/team%2Fname/payments%3F%23"
        );
    }

    #[test]
    fn github_rate_limits_are_retryable_publication_failures() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "7".parse().unwrap());
        assert_eq!(
            github_rate_limit_wait(reqwest::StatusCode::FORBIDDEN, &headers),
            Some(7)
        );
        assert_eq!(
            github_rate_limit_wait(reqwest::StatusCode::UNAUTHORIZED, &headers),
            None
        );

        let error = map_publication_write_error(
            "GitHub API rate limit error 403 Forbidden: slow down".to_string(),
        );
        assert_eq!(
            error.kind,
            crate::finding_publication::ProviderPublicationApiErrorKind::Unavailable
        );
        for message in [
            "GitHub API error 422 Unprocessable Entity: the endpoint has been spammed",
            "GitHub API error 422 Unprocessable Entity: abuse detection mechanism",
            "GitHub API error 422 Unprocessable Entity: secondary rate limit",
        ] {
            assert_eq!(
                map_publication_write_error(message.to_string()).kind,
                crate::finding_publication::ProviderPublicationApiErrorKind::Unavailable
            );
        }
        assert_eq!(
            map_publication_write_error(
                "GitHub API error 422 Unprocessable Entity: validation failed".to_string()
            )
            .kind,
            crate::finding_publication::ProviderPublicationApiErrorKind::InvalidAnchor
        );
    }

    #[test]
    fn github_pagination_follows_the_exact_safe_next_link() {
        let link = concat!(
            "<https://api.github.com/repos/acme/payments/pulls/42/comments?",
            "per_page=100&page=3>; rel=\"next\", ",
            "<https://api.github.com/repos/acme/payments/pulls/42/comments?",
            "per_page=100&page=9>; rel=\"last\""
        );
        assert_eq!(
            github_next_link(link).expect("valid pagination link"),
            Some(
                "https://api.github.com/repos/acme/payments/pulls/42/comments?per_page=100&page=3"
                    .to_string()
            )
        );
        assert!(
            github_next_link("<https://attacker.invalid/comments?page=2>; rel=\"next\"").is_err()
        );
        assert_eq!(
            github_next_link(
                "<https://api.github.com/repos/acme/payments/pulls/42/comments?page=9>; rel=\"last\""
            )
            .expect("last-only link"),
            None
        );
    }

    #[test]
    fn bitbucket_publication_pagination_stays_on_the_expected_comment_endpoint() {
        let endpoint =
            "https://api.bitbucket.org/2.0/repositories/acme/payments/pullrequests/42/comments";
        let next = format!("{endpoint}?pagelen=100&page=3");
        assert_eq!(
            safe_bitbucket_publication_next_url(&next, endpoint)
                .expect("same-endpoint pagination URL"),
            next
        );

        for unsafe_url in [
            "https://attacker.invalid/comments?page=2",
            "https://api.bitbucket.org.attacker.invalid/2.0/repositories/acme/payments/pullrequests/42/comments?page=2",
            "https://api.bitbucket.org/2.0/repositories/other/payments/pullrequests/42/comments?page=2",
            "https://user@api.bitbucket.org/2.0/repositories/acme/payments/pullrequests/42/comments?page=2",
            "https://api.bitbucket.org/2.0/repositories/acme/payments/pullrequests/42/comments?page=2#redirect",
        ] {
            assert!(
                safe_bitbucket_publication_next_url(unsafe_url, endpoint).is_err(),
                "accepted unsafe pagination URL: {unsafe_url}"
            );
        }
    }

    #[test]
    fn provider_anchor_rejections_are_terminal_publication_errors() {
        let error =
            map_publication_write_error("GitHub API error 422 Unprocessable Entity".to_string());
        assert_eq!(
            error.kind,
            crate::finding_publication::ProviderPublicationApiErrorKind::InvalidAnchor
        );

        let error =
            map_publication_write_error("GitHub API error 503 Service Unavailable".to_string());
        assert_eq!(
            error.kind,
            crate::finding_publication::ProviderPublicationApiErrorKind::Unavailable
        );

        let error =
            map_publication_write_error("GitHub API error 403 Forbidden: denied".to_string());
        assert_eq!(
            error.kind,
            crate::finding_publication::ProviderPublicationApiErrorKind::PermissionDenied
        );
    }

    #[test]
    fn pr_query_filter_combines_title_and_updated_window() {
        let opts = ListPrOptions {
            state: Some("MERGED".to_string()),
            page: Some(1),
            pagelen: Some(10),
            query: Some("payment".to_string()),
            updated_after: Some("2026-06-01T00:00:00.000Z".to_string()),
        };

        assert_eq!(
            pr_query_filter(&opts),
            Some("title ~ \"payment\" AND updated_on >= \"2026-06-01T00:00:00.000Z\"".to_string(),),
        );
    }

    #[test]
    fn pr_query_filter_sanitizes_literals() {
        let opts = ListPrOptions {
            state: None,
            page: None,
            pagelen: None,
            query: Some("quote\" slash\\".to_string()),
            updated_after: Some("2026-06-01T00:00:00.000Z\"".to_string()),
        };

        assert_eq!(
            pr_query_filter(&opts),
            Some(
                "title ~ \"quote slash\" AND updated_on >= \"2026-06-01T00:00:00.000Z\""
                    .to_string(),
            ),
        );
    }

    #[test]
    fn native_repo_config_validation_uses_shared_loader() {
        let nonce = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let repo_path = std::env::temp_dir().join(format!(
            "lachesi-native-config-validation-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&repo_path).expect("create temp repo");
        fs::write(repo_path.join(".lachesi.yaml"), "version: 0.1\n").expect("write config");

        let result = validate_repo_review_config_native(&repo_path, None).expect("validate config");

        assert_eq!(result.repo_path, repo_path.to_string_lossy());
        assert_eq!(result.errors.len(), 0);
        fs::remove_dir_all(repo_path).expect("remove temp repo");
    }

    #[test]
    fn pr_detail_keeps_branch_name_and_commit_hash() {
        let detail = map_pr_detail(BbPrDetail {
            id: 42,
            title: "Visual assets".to_string(),
            description: String::new(),
            state: "OPEN".to_string(),
            draft: false,
            author: None,
            source: Some(BbBranchRef {
                branch: Some(BbBranch {
                    name: "feature/assets-update".to_string(),
                }),
                commit: Some(BbCommitRef {
                    hash: "source-sha".to_string(),
                }),
            }),
            destination: Some(BbBranchRef {
                branch: Some(BbBranch {
                    name: "develop".to_string(),
                }),
                commit: Some(BbCommitRef {
                    hash: "destination-sha".to_string(),
                }),
            }),
            created_on: String::new(),
            updated_on: String::new(),
            participants: Vec::new(),
        });

        assert_eq!(detail.source_branch, "feature/assets-update");
        assert_eq!(detail.source_commit_hash.as_deref(), Some("source-sha"));
        assert_eq!(detail.destination_branch, "develop");
        assert_eq!(
            detail.destination_commit_hash.as_deref(),
            Some("destination-sha")
        );
    }

    fn review_snapshot_detail(
        source_branch: &str,
        destination_branch: &str,
        source_commit_hash: Option<&str>,
        destination_commit_hash: Option<&str>,
    ) -> PullRequestDetail {
        PullRequestDetail {
            id: 42,
            title: "Stable review".to_string(),
            description_raw: String::new(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "Reviewer".to_string(),
            reviewers: Vec::new(),
            source_branch: source_branch.to_string(),
            destination_branch: destination_branch.to_string(),
            source_commit_hash: source_commit_hash.map(ToOwned::to_owned),
            destination_commit_hash: destination_commit_hash.map(ToOwned::to_owned),
            created_on: String::new(),
            updated_on: String::new(),
        }
    }

    #[test]
    fn stable_review_snapshot_requires_matching_head_base_and_branches() {
        let before = review_snapshot_detail(
            "feature/review",
            "main",
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"),
        );
        let same = review_snapshot_detail(
            "feature/review",
            "main",
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        );
        assert!(validate_stable_pull_request_snapshot(&before, &same).is_ok());

        for changed in [
            review_snapshot_detail(
                "feature/review",
                "main",
                Some("cccccccccccccccccccccccccccccccccccccccc"),
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            ),
            review_snapshot_detail(
                "feature/review",
                "main",
                Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                Some("dddddddddddddddddddddddddddddddddddddddd"),
            ),
            review_snapshot_detail(
                "feature/review",
                "release",
                Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            ),
        ] {
            assert!(validate_stable_pull_request_snapshot(&before, &changed)
                .unwrap_err()
                .contains("changed while its review snapshot was loading"));
        }
    }

    #[test]
    fn stable_review_snapshot_rejects_missing_provider_commit_ids() {
        let before = review_snapshot_detail("feature/review", "main", None, None);
        let after = review_snapshot_detail("feature/review", "main", None, None);

        assert!(validate_stable_pull_request_snapshot(&before, &after).is_err());
    }
}
