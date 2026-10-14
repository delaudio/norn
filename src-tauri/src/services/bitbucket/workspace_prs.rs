//! Bounded provider reads for the terminal workspace (ADR 0020, #357).
mod approval;
pub(crate) use approval::WriteOutcome;
mod images;
mod snapshots;
use super::*;
use reqwest::{blocking::Client, redirect::Policy, Url};
use serde_json::Value;
pub(crate) use snapshots::{snapshot_sections, snapshot_sections_with_limit};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

const BODY_BYTES: usize = 1024 * 1024;
const RESULT_BYTES: usize = 512 * 1024;
const MAX_PAGE: u32 = 100;
const MAX_REVIEW_PAGES: u32 = 5;
const TOTAL_TIME: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryIdentity {
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
}
impl RepositoryIdentity {
    pub fn validate(&self) -> Result<(), String> {
        for part in [&self.workspace, &self.repo] {
            if part.is_empty()
                || part.len() > 256
                || part == "."
                || part == ".."
                || !(part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                    || (self.provider == ReviewProvider::Bitbucket && braced_uuid(part)))
            {
                return Err("Invalid provider repository identity.".into());
            }
        }
        Ok(())
    }
}
fn braced_uuid(value: &str) -> bool {
    value
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
        .is_some_and(|v| {
            v.len() == 36
                && v.bytes().enumerate().all(|(i, b)| {
                    if matches!(i, 8 | 13 | 18 | 23) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
        })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PrFilter {
    Open,
    Draft,
    Merged,
    All,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListParams {
    pub repository: RepositoryIdentity,
    pub filter: PrFilter,
    pub page: u32,
    pub limit: u32,
    pub query: Option<String>,
    pub author_id: Option<String>,
}
impl ListParams {
    pub fn validate(&self) -> Result<(), String> {
        self.repository.validate()?;
        if self.page == 0
            || self.page > MAX_PAGE
            || self.limit == 0
            || self.limit > 100
            || self
                .query
                .as_ref()
                .is_some_and(|s| s.len() > 256 || s.chars().any(char::is_control))
            || self
                .author_id
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        {
            return Err("Invalid pull-request list filters or page bounds.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailParams {
    pub repository: RepositoryIdentity,
    pub pr_id: u32,
}
impl DetailParams {
    pub fn validate(&self) -> Result<(), String> {
        self.repository.validate()?;
        if self.pr_id == 0 || self.pr_id >= crate::local_review::LOCAL_REVIEW_ID_FLAG {
            return Err("Invalid provider pull-request number.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserParams {
    pub repository: RepositoryIdentity,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrPage {
    pub request: ListParams,
    pub values: Vec<PullRequestSummary>,
    pub next_page: Option<u32>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDetail {
    pub request: DetailParams,
    pub detail: PullRequestDetail,
    pub author_id: Option<String>,
    pub browser_url: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    pub repository: RepositoryIdentity,
    pub account_id: String,
    pub display_name: String,
}
#[derive(Clone)]
enum Auth {
    Github(String),
    Bitbucket(Credentials),
}
pub struct Api {
    http: Client,
    auth: Auth,
    repository: RepositoryIdentity,
    root: String,
}
pub(crate) fn needs_browser_alias(repository: &RepositoryIdentity) -> bool {
    repository.provider == ReviewProvider::Bitbucket
        && (braced_uuid(&repository.workspace) || braced_uuid(&repository.repo))
}

pub(crate) fn browser_url(
    params: &DetailParams,
    resolved: &RepositoryIdentity,
) -> Result<String, String> {
    params.validate()?;
    resolved.validate()?;
    if resolved.provider != params.repository.provider
        || braced_uuid(&resolved.workspace)
        || braced_uuid(&resolved.repo)
        || (!braced_uuid(&params.repository.workspace)
            && !params
                .repository
                .workspace
                .eq_ignore_ascii_case(&resolved.workspace))
        || (!braced_uuid(&params.repository.repo)
            && !params.repository.repo.eq_ignore_ascii_case(&resolved.repo))
    {
        return Err(error());
    }
    Ok(format!(
        "https://{}/{}/{}/{}/{}",
        if resolved.provider == ReviewProvider::Github {
            "github.com"
        } else {
            "bitbucket.org"
        },
        encode_path_segment(&resolved.workspace),
        encode_path_segment(&resolved.repo),
        if resolved.provider == ReviewProvider::Github {
            "pull"
        } else {
            "pull-requests"
        },
        params.pr_id
    ))
}

fn error() -> String {
    "The provider PR response could not be verified. Refresh explicitly after checking provider availability.".into()
}
fn bounded_result<T: Serialize>(value: T) -> Result<T, String> {
    if serde_json::to_vec(&value).map_err(|_| error())?.len() > RESULT_BYTES {
        return Err(
            "Provider PR data exceeds the workspace response limit; use a smaller page.".into(),
        );
    }
    Ok(value)
}
fn safe_text(value: &str, max: usize, multiline: bool) -> Result<(), String> {
    if value.len() > max
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
    {
        Err(error())
    } else {
        Ok(())
    }
}
fn validate_summary(value: &PullRequestSummary) -> Result<(), String> {
    if value.id == 0
        || value.id >= crate::local_review::LOCAL_REVIEW_ID_FLAG
        || !matches!(
            value.state.as_str(),
            "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED"
        )
    {
        return Err(error());
    }
    safe_text(&value.title, 4096, false)?;
    for text in [
        &value.author_display_name,
        &value.source_branch,
        &value.destination_branch,
        &value.created_on,
        &value.updated_on,
    ] {
        safe_text(text, 256, false)?;
    }
    if let Some(id) = &value.author_account_id {
        safe_text(id, 256, false)?;
    }
    validate_reviewers(&value.reviewers)
}
fn validate_reviewers(values: &[Participant]) -> Result<(), String> {
    if values.len() > 100 {
        return Err(error());
    }
    for value in values {
        safe_text(&value.display_name, 256, false)?;
        safe_text(&value.role, 32, false)?;
        if let Some(id) = &value.account_id {
            safe_text(id, 256, false)?;
        }
    }
    Ok(())
}
impl Api {
    pub fn from_stored(repository: RepositoryIdentity) -> Result<Self, String> {
        repository.validate()?;
        let auth = match repository.provider {
            ReviewProvider::Github => Auth::Github(credentials::load_github_token().ok_or(
                "GitHub credentials are unavailable; configure authentication in Settings.",
            )?),
            ReviewProvider::Bitbucket => Auth::Bitbucket(credentials::load().ok_or(
                "Bitbucket credentials are unavailable; configure authentication in Settings.",
            )?),
        };
        Self::with_auth(repository, auth)
    }
    fn with_auth(repository: RepositoryIdentity, auth: Auth) -> Result<Self, String> {
        let builder = Client::builder()
            .user_agent("norn")
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10));
        #[cfg(test)]
        let builder = builder.no_proxy();
        Ok(Self {
            http: builder.build().map_err(|_| error())?,
            root: match repository.provider {
                ReviewProvider::Github => "https://api.github.com",
                ReviewProvider::Bitbucket => "https://api.bitbucket.org/2.0",
            }
            .into(),
            auth,
            repository,
        })
    }
    fn base(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.root,
            if self.repository.provider == ReviewProvider::Github {
                "repos"
            } else {
                "repositories"
            },
            encode_path_segment(&self.repository.workspace),
            encode_path_segment(&self.repository.repo)
        )
    }
    fn collection(&self) -> String {
        format!(
            "{}/{}",
            self.base(),
            if self.repository.provider == ReviewProvider::Github {
                "pulls"
            } else {
                "pullrequests"
            }
        )
    }
    fn get(&self, url: &str, started: Instant) -> Result<(Value, Option<String>), String> {
        let (bytes, link) =
            self.get_bytes(url, started, BODY_BYTES, "application/vnd.github+json")?;
        Ok((serde_json::from_slice(&bytes).map_err(|_| error())?, link))
    }
    fn get_bytes(
        &self,
        url: &str,
        started: Instant,
        maximum: usize,
        accept: &str,
    ) -> Result<(Vec<u8>, Option<String>), String> {
        let remaining = TOTAL_TIME
            .checked_sub(started.elapsed())
            .ok_or_else(error)?;
        let request = self
            .http
            .get(url)
            .timeout(remaining.min(Duration::from_secs(10)));
        let request = match &self.auth {
            Auth::Github(token) => request.bearer_auth(token).header("Accept", accept),
            Auth::Bitbucket(creds) => request.basic_auth(&creds.username, Some(&creds.token)),
        };
        let mut response = request.send().map_err(|_| error())?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(
                "Provider PR reads are rate limited or unavailable for this account.".into(),
            );
        }
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > maximum as u64)
        {
            return Err(error());
        }
        let link = response
            .headers()
            .get(reqwest::header::LINK)
            .map(|v| v.to_str().map(str::to_owned).map_err(|_| error()))
            .transpose()?;
        if link.as_ref().is_some_and(|s| s.len() > 8192) {
            return Err(error());
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take((maximum + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| error())?;
        if bytes.len() > maximum || started.elapsed() >= TOTAL_TIME {
            return Err(error());
        }
        Ok((bytes, link))
    }
    fn next_page(
        &self,
        next: Option<&str>,
        collection: &str,
        page: u32,
        limit: u32,
    ) -> Result<Option<u32>, String> {
        let Some(next) = next else { return Ok(None) };
        let next = Url::parse(next).map_err(|_| error())?;
        let base = Url::parse(collection).map_err(|_| error())?;
        let page_key = "page";
        let limit_key = if self.repository.provider == ReviewProvider::Github {
            "per_page"
        } else {
            "pagelen"
        };
        let query = next.query_pairs().collect::<Vec<_>>();
        let values = |key: &str| {
            query
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.as_ref())
                .collect::<Vec<_>>()
        };
        if next.origin() != base.origin()
            || next.path() != base.path()
            || !next.username().is_empty()
            || next.password().is_some()
            || next.fragment().is_some()
            || values(page_key) != vec![(page + 1).to_string().as_str()]
            || values(limit_key) != vec![limit.to_string().as_str()]
        {
            return Err(error());
        }
        if page >= MAX_PAGE {
            return Err(
                "Provider PR pagination exceeds the scan limit; narrow the filters.".into(),
            );
        }
        Ok(Some(page + 1))
    }
    pub fn list(&self, params: ListParams) -> Result<PrPage, String> {
        params.validate()?;
        if params.repository != self.repository {
            return Err(error());
        }
        let collection = self.collection();
        let mut url = Url::parse(&collection).map_err(|_| error())?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("page", &params.page.to_string());
            q.append_pair(
                if self.repository.provider == ReviewProvider::Github {
                    "per_page"
                } else {
                    "pagelen"
                },
                &params.limit.to_string(),
            );
            if self.repository.provider == ReviewProvider::Github {
                q.append_pair(
                    "state",
                    match params.filter {
                        PrFilter::Merged => "closed",
                        PrFilter::All => "all",
                        _ => "open",
                    },
                );
            } else {
                for state in match params.filter {
                    PrFilter::All => &['O', 'M', 'D', 'S'][..],
                    PrFilter::Merged => &['M'][..],
                    _ => &['O'][..],
                } {
                    q.append_pair(
                        "state",
                        match state {
                            'O' => "OPEN",
                            'M' => "MERGED",
                            'D' => "DECLINED",
                            _ => "SUPERSEDED",
                        },
                    );
                }
            }
        }
        let (json, link) = self.get(url.as_str(), Instant::now())?;
        let (mut values, next) = if self.repository.provider == ReviewProvider::Github {
            let raw = json.as_array().ok_or_else(error)?;
            if raw.len() > params.limit as usize {
                return Err(error());
            }
            let mut values = Vec::new();
            for value in raw {
                let pr: GhPullRequest =
                    serde_json::from_value(value.clone()).map_err(|_| error())?;
                if !matches!(pr.state.as_str(), "open" | "closed") {
                    return Err(error());
                }
                // Inherited mapper adds these counts; reject overflow before mapping.
                if pr
                    .comments
                    .unwrap_or(0)
                    .checked_add(pr.review_comments.unwrap_or(0))
                    .is_none()
                {
                    return Err(error());
                }
                values.push(map_gh_pr_summary(pr));
            }
            let next = github_next(link.as_deref())?;
            (
                values,
                self.next_page(next.as_deref(), &collection, params.page, params.limit)?,
            )
        } else {
            let page: BbPrPage = serde_json::from_value(json).map_err(|_| error())?;
            if page.page != params.page || page.values.len() > params.limit as usize {
                return Err(error());
            }
            let next =
                self.next_page(page.next.as_deref(), &collection, params.page, params.limit)?;
            (page.values.into_iter().map(map_pr_summary).collect(), next)
        };
        let mut ids = HashSet::new();
        for value in &values {
            validate_summary(value)?;
            if !ids.insert(value.id) {
                return Err(error());
            }
        }
        values.retain(|pr| {
            let state = match params.filter {
                PrFilter::Open => pr.state == "OPEN",
                PrFilter::Draft => pr.state == "OPEN" && pr.draft,
                PrFilter::Merged => pr.state == "MERGED",
                PrFilter::All => true,
            };
            let author = params
                .author_id
                .as_ref()
                .is_none_or(|id| pr.author_account_id.as_ref() == Some(id));
            let query = params.query.as_ref().is_none_or(|q| {
                let q = q.to_lowercase();
                [
                    &pr.title,
                    &pr.source_branch,
                    &pr.author_display_name,
                    &pr.id.to_string(),
                ]
                .iter()
                .any(|s| s.to_lowercase().contains(&q))
            });
            state && author && query
        });
        bounded_result(PrPage {
            request: params,
            values,
            next_page: next,
        })
    }
    pub(crate) fn browser_repository(
        &self,
        params: &DetailParams,
    ) -> Result<RepositoryIdentity, String> {
        params.validate()?;
        if params.repository != self.repository {
            return Err(error());
        }
        if !needs_browser_alias(&self.repository) {
            return Ok(self.repository.clone());
        }
        let (metadata, _) = self.get(&self.base(), Instant::now())?;
        if braced_uuid(&self.repository.repo)
            && metadata
                .get("uuid")
                .and_then(Value::as_str)
                .is_none_or(|uuid| !uuid.eq_ignore_ascii_case(&self.repository.repo))
        {
            return Err(error());
        }
        if braced_uuid(&self.repository.workspace)
            && metadata
                .pointer("/workspace/uuid")
                .and_then(Value::as_str)
                .is_none_or(|uuid| !uuid.eq_ignore_ascii_case(&self.repository.workspace))
        {
            return Err(error());
        }
        let (workspace, repo) = metadata
            .get("full_name")
            .and_then(Value::as_str)
            .and_then(|name| name.split_once('/'))
            .ok_or_else(error)?;
        let resolved = RepositoryIdentity {
            provider: self.repository.provider,
            workspace: workspace.into(),
            repo: repo.into(),
        };
        browser_url(params, &resolved)?;
        Ok(resolved)
    }

    pub fn user(&self) -> Result<CurrentUser, String> {
        let (value, _) = self.get(&format!("{}/user", self.root), Instant::now())?;
        let (account_id, display_name) = if self.repository.provider == ReviewProvider::Github {
            let user: GhUser = serde_json::from_value(value).map_err(|_| error())?;
            (
                user.login.clone(),
                user.name.filter(|s| !s.is_empty()).unwrap_or(user.login),
            )
        } else {
            let user: BbUser = serde_json::from_value(value).map_err(|_| error())?;
            (user.account_id.ok_or_else(error)?, user.display_name)
        };
        if account_id.is_empty() {
            return Err(error());
        }
        safe_text(&account_id, 256, false)?;
        safe_text(&display_name, 256, false)?;
        bounded_result(CurrentUser {
            repository: self.repository.clone(),
            account_id,
            display_name,
        })
    }
    pub fn detail(&self, params: DetailParams) -> Result<PrDetail, String> {
        params.validate()?;
        if params.repository != self.repository {
            return Err(error());
        }
        let started = Instant::now();
        let base = format!("{}/{}", self.collection(), params.pr_id);
        let (json, _) = self.get(&base, started)?;
        let (mut detail, author_id) =
            parse_metadata(self.repository.provider, &json, params.pr_id)?;
        if self.repository.provider == ReviewProvider::Github {
            let collection = format!("{base}/reviews");
            let mut page = 1;
            let mut last_review_id = 0;
            loop {
                let (reviews, link) =
                    self.get(&format!("{collection}?per_page=100&page={page}"), started)?;
                let reviews = reviews.as_array().ok_or_else(error)?;
                if reviews.len() > 100 {
                    return Err(error());
                }
                for review in reviews {
                    let review_id = review
                        .get("id")
                        .and_then(Value::as_u64)
                        .filter(|id| *id > last_review_id)
                        .ok_or_else(error)?;
                    last_review_id = review_id;
                    let login = review
                        .get("user")
                        .and_then(|v| v.get("login"))
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .ok_or_else(error)?;
                    safe_text(login, 256, false)?;
                    let state = review
                        .get("state")
                        .and_then(Value::as_str)
                        .ok_or_else(error)?;
                    if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED") {
                        if let Some(user) = detail
                            .reviewers
                            .iter_mut()
                            .find(|u| u.account_id.as_deref() == Some(login))
                        {
                            user.approved = state == "APPROVED";
                        } else {
                            detail.reviewers.push(Participant {
                                display_name: login.into(),
                                account_id: Some(login.into()),
                                role: "REVIEWER".into(),
                                approved: state == "APPROVED",
                            });
                        }
                    } else if !matches!(state, "COMMENTED" | "PENDING") {
                        return Err(error());
                    }
                }
                let next = github_next(link.as_deref())?;
                let next = self.next_page(next.as_deref(), &collection, page, 100)?;
                if next.is_some() && page >= MAX_REVIEW_PAGES {
                    return Err("PR review history exceeds the complete-read limit.".into());
                }
                if let Some(next) = next {
                    page = next;
                } else {
                    break;
                }
            }
        }
        validate_metadata(&detail, author_id.as_deref())?;
        let browser_url = format!(
            "https://{}/{}/{}/{}/{}",
            if self.repository.provider == ReviewProvider::Github {
                "github.com"
            } else {
                "bitbucket.org"
            },
            encode_path_segment(&self.repository.workspace),
            encode_path_segment(&self.repository.repo),
            if self.repository.provider == ReviewProvider::Github {
                "pull"
            } else {
                "pull-requests"
            },
            params.pr_id
        );
        bounded_result(PrDetail {
            request: params,
            detail,
            author_id,
            browser_url,
        })
    }
}
fn parse_metadata(
    provider: ReviewProvider,
    json: &Value,
    pr_id: u32,
) -> Result<(PullRequestDetail, Option<String>), String> {
    let author_id = if provider == ReviewProvider::Github {
        json.get("user").and_then(|v| v.get("login"))
    } else {
        json.get("author").and_then(|v| v.get("account_id"))
    }
    .and_then(Value::as_str)
    .map(str::to_owned);
    let detail = if provider == ReviewProvider::Github {
        let raw: GhPullRequest = serde_json::from_value(json.clone()).map_err(|_| error())?;
        if !matches!(raw.state.as_str(), "open" | "closed") {
            return Err(error());
        }
        map_gh_pr_detail(raw)
    } else {
        map_pr_detail(serde_json::from_value(json.clone()).map_err(|_| error())?)
    };
    if detail.id != pr_id
        || !matches!(
            detail.state.as_str(),
            "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED"
        )
    {
        return Err(error());
    }
    validate_metadata(&detail, author_id.as_deref())?;
    Ok((detail, author_id))
}
fn validate_metadata(detail: &PullRequestDetail, author_id: Option<&str>) -> Result<(), String> {
    safe_text(&detail.title, 4096, false)?;
    safe_text(&detail.description_raw, 64 * 1024, true)?;
    for text in [
        &detail.author_display_name,
        &detail.source_branch,
        &detail.destination_branch,
        &detail.created_on,
        &detail.updated_on,
    ] {
        safe_text(text, 256, false)?;
    }
    for revision in [&detail.source_commit_hash, &detail.destination_commit_hash]
        .into_iter()
        .flatten()
    {
        if !(7..=64).contains(&revision.len()) || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(error());
        }
    }
    if let Some(id) = author_id {
        safe_text(id, 256, false)?;
    }
    validate_reviewers(&detail.reviewers)?;
    Ok(())
}

fn github_next(link: Option<&str>) -> Result<Option<String>, String> {
    let mut next = None;
    if let Some(link) = link {
        for part in link.split(',') {
            let mut parts = part.trim().split(';');
            let target = parts.next().ok_or_else(error)?.trim();
            if parts.any(|p| p.trim() == "rel=\"next\"") {
                if next.is_some() {
                    return Err(error());
                }
                next = Some(
                    target
                        .strip_prefix('<')
                        .and_then(|s| s.strip_suffix('>'))
                        .ok_or_else(error)?
                        .into(),
                );
            }
        }
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
    };

    struct Server {
        root: String,
        requests: Arc<Mutex<Vec<String>>>,
        stopped: Arc<AtomicBool>,
        join: Option<thread::JoinHandle<()>>,
    }
    impl Server {
        fn new(
            handler: impl Fn(&str, &str) -> (u16, String, String) + Send + Sync + 'static,
        ) -> Self {
            Self::new_bytes(move |path, request| {
                let (status, headers, body) = handler(path, request);
                (status, headers, body.into_bytes())
            })
        }
        fn new_bytes(
            handler: impl Fn(&str, &str) -> (u16, String, Vec<u8>) + Send + Sync + 'static,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let root = format!("http://{}", listener.local_addr().unwrap());
            listener.set_nonblocking(true).unwrap();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let stopped = Arc::new(AtomicBool::new(false));
            let stop = stopped.clone();
            let seen = requests.clone();
            let handler = Arc::new(handler);
            let join = thread::spawn(move || {
                let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
                while !stop.load(Ordering::SeqCst) {
                    let mut i = 0;
                    while i < workers.len() {
                        if workers[i].is_finished() {
                            workers.swap_remove(i).join().unwrap();
                        } else {
                            i += 1;
                        }
                    }
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            assert!(workers.len() < 16, "owned fixture connection bound");
                            let seen = seen.clone();
                            let handler = handler.clone();
                            workers.push(thread::spawn(move || {
                                // Accepted sockets can inherit the listener's nonblocking flag.
                                stream.set_nonblocking(false).unwrap();
                                stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                                let mut bytes = Vec::new(); let mut chunk = [0; 2048];
                                while bytes.len() <= 16 * 1024 && !bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                                    match stream.read(&mut chunk) { Ok(0) | Err(_) => return, Ok(n) => bytes.extend_from_slice(&chunk[..n]) }
                                }
                                if !bytes.windows(4).any(|s| s == b"\r\n\r\n") { return; }
                                let request = String::from_utf8(bytes).unwrap();
                                let first = request.lines().next().unwrap();
                                assert!(first.starts_with("GET "), "read-only adapter unexpectedly writes");
                                let path = first.split_whitespace().nth(1).unwrap();
                                seen.lock().unwrap().push(path.into());
                                let (status, headers, body) = handler(path, &request);
                                let response = format!("HTTP/1.1 {status} Fixture\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n", body.len());
                                let _ = stream.write_all(response.as_bytes());
                                let _ = stream.write_all(&body);
                            }));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2))
                        }
                        Err(e) => panic!("owned fixture accept failed: {e}"),
                    }
                }
                for worker in workers {
                    worker.join().unwrap();
                }
            });
            Self {
                root,
                requests,
                stopped,
                join: Some(join),
            }
        }
        fn api(&self, provider: ReviewProvider) -> Api {
            let repository = identity(provider);
            let auth = if provider == ReviewProvider::Github {
                Auth::Github("owned-fixture-token".into())
            } else {
                Auth::Bitbucket(Credentials {
                    username: "owned-fixture-user".into(),
                    token: "owned-fixture-token".into(),
                })
            };
            let mut api = Api::with_auth(repository, auth).unwrap();
            api.root = self.root.clone();
            api
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::SeqCst);
            self.join.take().unwrap().join().unwrap();
        }
    }
    fn identity(provider: ReviewProvider) -> RepositoryIdentity {
        RepositoryIdentity {
            provider,
            workspace: "Owner".into(),
            repo: "Repo".into(),
        }
    }
    fn params(provider: ReviewProvider, filter: PrFilter) -> ListParams {
        ListParams {
            repository: identity(provider),
            filter,
            page: 1,
            limit: 2,
            query: None,
            author_id: None,
        }
    }
    fn gh(id: u32, draft: bool, state: &str) -> Value {
        json!({"number":id,"title":format!("Change {id}"),"body":"Actual description","state":state,"draft":draft,"user":{"login":"author"},"head":{"ref":"feature","sha":"a".repeat(40)},"base":{"ref":"main","sha":"b".repeat(40)},"requested_reviewers":[{"login":"reviewer"}]})
    }
    fn bb(id: u32, draft: bool) -> Value {
        json!({"id":id,"title":format!("Change {id}"),"state":"OPEN","draft":draft,"author":{"account_id":"bb-author","display_name":"Author"},"source":{"branch":{"name":"feature"},"commit":{"hash":"a".repeat(40)}},"destination":{"branch":{"name":"main"},"commit":{"hash":"b".repeat(40)}}})
    }

    #[test]
    fn native_provider_browser_urls_and_uuid_aliases_preserve_origin_and_identity() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(|_, _| panic!("ordinary browser URL acquired provider data"));
            let params = DetailParams {
                repository: identity(provider),
                pr_id: 7,
            };
            let resolved = server.api(provider).browser_repository(&params).unwrap();
            let url = browser_url(&params, &resolved).unwrap();
            assert_eq!(
                url,
                format!(
                    "https://{}/Owner/Repo/{}/7",
                    if provider == ReviewProvider::Github {
                        "github.com"
                    } else {
                        "bitbucket.org"
                    },
                    if provider == ReviewProvider::Github {
                        "pull"
                    } else {
                        "pull-requests"
                    }
                )
            );
            assert!(server.requests.lock().unwrap().is_empty());
            let mut foreign = resolved;
            foreign.repo = "Other".into();
            assert!(browser_url(&params, &foreign).is_err());
        }
        let owner = "{11111111-1111-1111-1111-111111111111}";
        let repo = "{22222222-2222-2222-2222-222222222222}";
        for fault in ["none", "owner", "repo", "path", "host", "redirect"] {
            let server = Server::new(move |_, _| {
                if fault == "redirect" {
                    return (
                        302,
                        "Location: http://127.0.0.1:1/private\r\n".into(),
                        String::new(),
                    );
                }
                (200,String::new(),json!({"uuid":if fault=="repo"{owner}else{repo},"workspace":{"uuid":if fault=="owner"{repo}else{owner}},"full_name":if fault=="path"{"Owned/../Repo"}else if fault=="host"{"https://evil.test/x"}else{"ActualWorkspace/ActualRepo"},"links":{"html":{"href":"https://evil.test/private"}}}).to_string())
            });
            let mut api = server.api(ReviewProvider::Bitbucket);
            api.repository = RepositoryIdentity {
                provider: ReviewProvider::Bitbucket,
                workspace: owner.into(),
                repo: repo.into(),
            };
            let params = DetailParams {
                repository: api.repository.clone(),
                pr_id: 7,
            };
            let result = api.browser_repository(&params);
            if fault == "none" {
                assert_eq!(
                    browser_url(&params, &result.unwrap()).unwrap(),
                    "https://bitbucket.org/ActualWorkspace/ActualRepo/pull-requests/7"
                );
            } else {
                assert!(result.is_err(), "{fault}");
            }
            assert_eq!(server.requests.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn filters_preserve_empty_page_continuation_and_full_provider_identity() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(move |path, request| {
                let host = request
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("host: ")
                            .or_else(|| l.strip_prefix("Host: "))
                    })
                    .unwrap();
                let collection = path.split('?').next().unwrap();
                if Url::parse(&format!("http://fixture{path}"))
                    .unwrap()
                    .query_pairs()
                    .any(|(k, v)| k == "page" && v == "2")
                {
                    let body = if provider == ReviewProvider::Github {
                        json!([gh(2, true, "open")])
                    } else {
                        json!({"page":2,"values":[bb(2,true)]})
                    };
                    return (200, String::new(), body.to_string());
                }
                let next = format!(
                    "http://{host}{collection}?page=2&{}=2",
                    if provider == ReviewProvider::Github {
                        "per_page"
                    } else {
                        "pagelen"
                    }
                );
                if provider == ReviewProvider::Github {
                    (
                        200,
                        format!("Link: <{next}>; rel=\"next\"\r\n"),
                        json!([gh(1, false, "open")]).to_string(),
                    )
                } else {
                    (
                        200,
                        String::new(),
                        json!({"page":1,"values":[bb(1,false)],"next":next}).to_string(),
                    )
                }
            });
            let api = server.api(provider);
            let first = api.list(params(provider, PrFilter::Draft)).unwrap();
            assert!(first.values.is_empty());
            assert_eq!(first.next_page, Some(2));
            assert_eq!(first.request.repository, identity(provider));
            let mut second_params = params(provider, PrFilter::Draft);
            second_params.page = 2;
            let second = api.list(second_params).unwrap();
            assert_eq!(second.values[0].id, 2);
            assert!(second.values[0].draft);
            assert_eq!(second.next_page, None);
            let mut filtered = params(provider, PrFilter::Open);
            filtered.author_id = Some("other-author".into());
            assert!(api.list(filtered).unwrap().values.is_empty());
            assert_eq!(
                api.list(params(provider, PrFilter::Open))
                    .unwrap()
                    .values
                    .len(),
                1
            );
            let mut wrong = params(provider, PrFilter::All);
            wrong.repository.provider = if provider == ReviewProvider::Github {
                ReviewProvider::Bitbucket
            } else {
                ReviewProvider::Github
            };
            assert!(api.list(wrong).is_err());
        }
    }
    #[test]
    fn merged_filter_uses_actual_merge_evidence_and_query_author_filters() {
        let server = Server::new(|_, _| {
            let mut merged = gh(2, false, "closed");
            merged["merged_at"] = json!("2026-10-11");
            (
                200,
                String::new(),
                json!([gh(1, false, "closed"), merged]).to_string(),
            )
        });
        let api = server.api(ReviewProvider::Github);
        let mut p = params(ReviewProvider::Github, PrFilter::Merged);
        p.author_id = Some("author".into());
        p.query = Some("change 2".into());
        let result = api.list(p).unwrap();
        assert_eq!(result.values.len(), 1);
        assert_eq!(result.values[0].state, "MERGED");
        assert_eq!(
            api.list(params(ReviewProvider::Github, PrFilter::All))
                .unwrap()
                .values
                .len(),
            2
        );
    }
    #[test]
    fn actual_detail_current_user_and_review_approval_are_native() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(move |path, _| {
                let value = if path == "/user" {
                    if provider == ReviewProvider::Github {
                        json!({"login":"native-author","name":"Native Author"})
                    } else {
                        json!({"account_id":"native-author","display_name":"Native Author"})
                    }
                } else if path.contains("/reviews?") {
                    json!([{"id":1,"state":"APPROVED","user":{"login":"reviewer"}},{"id":2,"state":"CHANGES_REQUESTED","user":{"login":"reviewer"}},{"id":3,"state":"APPROVED","user":{"login":"second"}}])
                } else if provider == ReviewProvider::Github {
                    gh(7, false, "open")
                } else {
                    let mut value = bb(7, false);
                    value["description"] = json!("Actual description");
                    value["participants"] = json!([{"role":"REVIEWER","approved":true,"user":{"account_id":"reviewer","display_name":"Reviewer"}}]);
                    value
                };
                (200, String::new(), value.to_string())
            });
            let api = server.api(provider);
            let user = api.user().unwrap();
            assert_eq!(user.account_id, "native-author");
            assert_eq!(user.repository, identity(provider));
            let detail = api
                .detail(DetailParams {
                    repository: identity(provider),
                    pr_id: 7,
                })
                .unwrap();
            assert_eq!(detail.detail.description_raw, "Actual description");
            assert_eq!(detail.detail.source_commit_hash, Some("a".repeat(40)));
            assert_eq!(
                detail.author_id,
                Some(
                    if provider == ReviewProvider::Github {
                        "author"
                    } else {
                        "bb-author"
                    }
                    .into()
                )
            );
            if provider == ReviewProvider::Github {
                assert!(!detail.detail.reviewers[0].approved);
                assert!(detail.detail.reviewers[1].approved);
                assert_eq!(detail.browser_url, "https://github.com/Owner/Repo/pull/7");
            } else {
                assert!(detail.detail.reviewers[0].approved);
                assert_eq!(
                    detail.browser_url,
                    "https://bitbucket.org/Owner/Repo/pull-requests/7"
                );
            }
        }
    }
    #[test]
    fn unsafe_redirects_paging_malformed_and_oversized_data_fail_without_retry() {
        for case in 0..7 {
            let server = Server::new(move |_, _| match case {
                0 => (
                    302,
                    "Location: http://127.0.0.1:1/credential-trap\r\n".into(),
                    String::new(),
                ),
                1 => (
                    200,
                    "Link: <https://attacker.invalid/pulls?page=2&per_page=2>; rel=\"next\"\r\n"
                        .into(),
                    json!([gh(1, false, "open")]).to_string(),
                ),
                2 => (200, String::new(), "x".repeat(BODY_BYTES + 1)),
                3 => (
                    200,
                    String::new(),
                    "malformed-sensitive-provider-body".into(),
                ),
                4 => (429, String::new(), "sensitive-provider-error".into()),
                5 => {
                    let mut v = gh(1, false, "open");
                    v["title"] = json!("x".repeat(4097));
                    (200, String::new(), json!([v]).to_string())
                }
                _ => (
                    200,
                    String::new(),
                    json!([gh(1, false, "invented-state")]).to_string(),
                ),
            });
            let error = server
                .api(ReviewProvider::Github)
                .list(params(ReviewProvider::Github, PrFilter::All))
                .err()
                .unwrap();
            assert!(!error.contains("sensitive"));
            assert!(!error.contains("owned-fixture-token"));
            assert_eq!(server.requests.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn bitbucket_uuid_aliases_are_preserved_and_encoded_as_single_segments() {
        let alias = "{01234567-89ab-cdef-0123-456789abcdef}";
        let mut repository = identity(ReviewProvider::Bitbucket);
        repository.workspace = alias.into();
        repository.repo = alias.into();
        assert!(repository.validate().is_ok());
        let mut api =
            Server::new(|_, _| (200, String::new(), "{}".into())).api(ReviewProvider::Bitbucket);
        api.repository = repository.clone();
        assert!(api.collection().contains("/%7B01234567-89ab-cdef-0123-456789abcdef%7D/%7B01234567-89ab-cdef-0123-456789abcdef%7D/pullrequests"));
        repository.provider = ReviewProvider::Github;
        assert!(repository.validate().is_err());
        repository.provider = ReviewProvider::Bitbucket;
        repository.workspace = "{not-a-uuid}".into();
        assert!(repository.validate().is_err());
    }
    #[test]
    fn invalid_identity_ids_filters_and_pagination_are_rejected() {
        let mut p = params(ReviewProvider::Github, PrFilter::All);
        p.repository.workspace = "..".into();
        assert!(p.validate().is_err());
        p.repository = identity(ReviewProvider::Github);
        p.page = 101;
        assert!(p.validate().is_err());
        p.page = 1;
        p.limit = 101;
        assert!(p.validate().is_err());
        p.limit = 2;
        p.query = Some("\u{1b}[2J".into());
        assert!(p.validate().is_err());
        assert!(DetailParams {
            repository: identity(ReviewProvider::Github),
            pr_id: crate::local_review::LOCAL_REVIEW_ID_FLAG
        }
        .validate()
        .is_err());
        let server = Server::new(|_, _| (200, String::new(), gh(8, false, "open").to_string()));
        assert!(server
            .api(ReviewProvider::Github)
            .detail(DetailParams {
                repository: identity(ReviewProvider::Github),
                pr_id: 7
            })
            .is_err());
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        let api = server.api(ReviewProvider::Github);
        let collection = api.collection();
        for next in [
            format!("{collection}?page=3&per_page=2"),
            format!("{collection}?page=2&page=2&per_page=2"),
            format!("{collection}/other?page=2&per_page=2"),
            format!("{collection}?page=2&per_page=100"),
        ] {
            assert!(api.next_page(Some(&next), &collection, 1, 2).is_err());
        }
    }
    #[test]
    fn immutable_provider_snapshots_use_actual_merge_base_and_fork_origin() {
        use crate::services::provider_snapshot::Manager;
        const PATCH: &str = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n";
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(move |path, request| {
                if path.contains("/compare/") && !path.contains('?') || path.contains("/diff/") {
                    if provider == ReviewProvider::Github {
                        assert!(request
                            .to_lowercase()
                            .contains("accept: application/vnd.github.diff"));
                    }
                    return (200, String::new(), PATCH.into());
                }
                let value = if path.contains("/compare/") {
                    json!({"base_commit":{"sha":"b".repeat(40)},"merge_base_commit":{"sha":"c".repeat(40)},"files":[]})
                } else if path.contains("/merge-base/") {
                    json!({"hash":"c".repeat(40)})
                } else if path.contains("/diffstat/") {
                    json!({"page":1,"values":[{"status":"modified","old":{"path":"x"},"new":{"path":"x"},"lines_added":1,"lines_removed":1}]})
                } else if provider == ReviewProvider::Github {
                    let mut value = gh(7, false, "open");
                    value["changed_files"] = json!(1);
                    value["head"]["repo"] = json!({"full_name":"ForkOwner/ForkRepo"});
                    value
                } else {
                    let mut value = bb(7, false);
                    value["source"]["repository"] = json!({"full_name":"ForkOwner/ForkRepo"});
                    value
                };
                (200, String::new(), value.to_string())
            });
            let capture = server
                .api(provider)
                .capture(DetailParams {
                    repository: identity(provider),
                    pr_id: 7,
                })
                .unwrap();
            assert_eq!(
                capture.source_repository.as_ref().unwrap().workspace,
                "ForkOwner"
            );
            let info = Manager::default().insert(capture).unwrap();
            assert_eq!(info.base_sha, "c".repeat(40));
            assert_eq!(info.destination_sha, "b".repeat(40));
            assert_eq!(info.file_count, 1);
            let requests = server.requests.lock().unwrap();
            if provider == ReviewProvider::Github {
                assert!(requests.iter().any(|path| path.ends_with(&format!(
                    "/compare/{}...{}",
                    "c".repeat(40),
                    "a".repeat(40)
                ))));
            } else {
                assert!(requests.iter().any(|path| path
                    == &format!(
                        "/repositories/ForkOwner/ForkRepo/diff/{}..{}?topic=false&renames=true",
                        "a".repeat(40),
                        "c".repeat(40)
                    )));
            }
            assert!(!requests.iter().any(
                |path| path.contains("/pullrequests/7/diff") || path.contains("/pulls/7/files")
            ));
        }
    }
    #[test]
    fn bitbucket_abbreviated_metadata_resolves_full_commits_and_fences_drift() {
        use crate::services::provider_snapshot::Manager;
        for fault in [
            "none",
            "prefix",
            "repository",
            "capture-drift",
            "start-drift",
        ] {
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let server = Server::new(move |path, _| {
                let value = if path.contains("/commit/") {
                    let count = calls.fetch_add(1, Ordering::SeqCst);
                    let source = path.contains("/Fork/Repo/");
                    let prefix = if source { "a" } else { "b" }.repeat(12);
                    let hash = if fault == "prefix" {
                        "d".repeat(40)
                    } else if (fault == "capture-drift" && count >= 2)
                        || (fault == "start-drift" && count >= 4)
                    {
                        format!("{prefix}{}", "e".repeat(28))
                    } else {
                        if source { "a" } else { "b" }.repeat(40)
                    };
                    json!({"hash":hash,"repository":{"full_name":if fault == "repository" { "Foreign/Repo" } else if source { "Fork/Repo" } else { "Owner/Repo" }}})
                } else if path.contains("/merge-base/") {
                    assert!(path.contains(&"a".repeat(40)) && path.contains(&"b".repeat(40)));
                    json!({"hash":"c".repeat(40)})
                } else if path.contains("/diffstat/") {
                    json!({"page":1,"values":[{"status":"modified","old":{"path":"x"},"new":{"path":"x"},"lines_added":1,"lines_removed":1}]})
                } else if path.contains("/diff/") {
                    return (
                        200,
                        String::new(),
                        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n".into(),
                    );
                } else {
                    let mut value = bb(7, false);
                    value["source"]["commit"]["hash"] = json!("a".repeat(12));
                    value["destination"]["commit"]["hash"] = json!("b".repeat(12));
                    value["source"]["repository"] = json!({"full_name":"Fork/Repo"});
                    value
                };
                (200, String::new(), value.to_string())
            });
            let api = server.api(ReviewProvider::Bitbucket);
            let capture = api.capture(DetailParams {
                repository: identity(ReviewProvider::Bitbucket),
                pr_id: 7,
            });
            if fault == "none" || fault == "start-drift" {
                let manager = Manager::default();
                let info = manager.insert(capture.unwrap()).unwrap();
                assert_eq!(info.head_sha, "a".repeat(40));
                assert_eq!(info.destination_sha, "b".repeat(40));
                let recheck = api.recheck_snapshot(&manager.get(&info.snapshot).unwrap());
                assert_eq!(recheck.is_ok(), fault == "none");
            } else {
                assert!(capture.is_err(), "accepted {fault}");
            }
        }
    }
    #[test]
    fn bitbucket_snapshot_accepts_suppressed_empty_context_without_relaxing_inventory() {
        use crate::services::provider_snapshot::Manager;
        for fault in ["none", "truncated", "inventory"] {
            let server = Server::new(move |path, _| {
                if path.contains("/diff/") {
                    let patch = if fault == "truncated" {
                        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n-old\n+new\n\n"
                    } else {
                        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n-old\n+new\n\n tail\n"
                    };
                    return (200, String::new(), patch.into());
                }
                let value = if path.contains("/merge-base/") {
                    json!({"hash":"c".repeat(40)})
                } else if path.contains("/diffstat/") {
                    assert!(path.contains("renames=true"));
                    json!({"page":1,"values":[{"status":"modified","old":{"path":"x"},"new":{"path":"x"},"lines_added":if fault == "inventory" { 2 } else { 1 },"lines_removed":1}]})
                } else {
                    let mut value = bb(7, false);
                    value["source"]["repository"] = json!({"full_name":"Fork/Repo"});
                    value
                };
                (200, String::new(), value.to_string())
            });
            let result = server
                .api(ReviewProvider::Bitbucket)
                .capture(DetailParams {
                    repository: identity(ReviewProvider::Bitbucket),
                    pr_id: 7,
                })
                .and_then(|capture| Manager::default().insert(capture));
            if fault == "none" {
                assert_eq!(result.unwrap().file_count, 1);
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.contains(if fault == "truncated" {
                        "incomplete"
                    } else {
                        "inventory"
                    }),
                    "{error}"
                );
                assert!(!error.contains("old\n") && !error.contains("Fork/Repo"));
            }
        }
    }
    #[test]
    fn review_start_rechecks_owned_provider_metadata_without_recapturing_patch() {
        use crate::services::provider_snapshot::Manager;
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            for fault in [
                "none",
                "head",
                "destination",
                "title",
                "source-branch",
                "destination-branch",
                "fork",
                "count",
            ] {
                if provider == ReviewProvider::Bitbucket && fault == "count" {
                    continue;
                }
                let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let metadata_reads = reads.clone();
                let server = Server::new(move |path, _| {
                    if path.contains("/compare/") && !path.contains('?') || path.contains("/diff/")
                    {
                        return (
                            200,
                            String::new(),
                            "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n"
                                .into(),
                        );
                    }
                    let value = if path.contains("/compare/") {
                        json!({"base_commit":{"sha":"b".repeat(40)},"merge_base_commit":{"sha":"c".repeat(40)}})
                    } else if path.contains("/merge-base/") {
                        json!({"hash":"c".repeat(40)})
                    } else if path.contains("/diffstat/") {
                        json!({"page":1,"values":[{"status":"modified","old":{"path":"x"},"new":{"path":"x"},"lines_added":1,"lines_removed":1}]})
                    } else {
                        let recheck = metadata_reads.fetch_add(1, Ordering::SeqCst) >= 2;
                        let mut value = if provider == ReviewProvider::Github {
                            gh(7, false, "open")
                        } else {
                            bb(7, false)
                        };
                        let source = if provider == ReviewProvider::Github {
                            "head"
                        } else {
                            "source"
                        };
                        let destination = if provider == ReviewProvider::Github {
                            "base"
                        } else {
                            "destination"
                        };
                        value[source]["repo"] = json!({"full_name":"ForkOwner/ForkRepo"});
                        if provider == ReviewProvider::Bitbucket {
                            value[source]["repository"] = json!({"full_name":"ForkOwner/ForkRepo"});
                        }
                        value["changed_files"] = json!(1);
                        if recheck {
                            match fault {
                                "head" | "destination" => {
                                    let side = if fault == "head" { source } else { destination };
                                    if provider == ReviewProvider::Github {
                                        value[side]["sha"] = json!("d".repeat(40));
                                    } else {
                                        value[side]["commit"]["hash"] = json!("d".repeat(40));
                                    }
                                }
                                "source-branch" | "destination-branch" => {
                                    let side = if fault == "source-branch" {
                                        source
                                    } else {
                                        destination
                                    };
                                    if provider == ReviewProvider::Github {
                                        value[side]["ref"] = json!("changed");
                                    } else {
                                        value[side]["branch"]["name"] = json!("changed");
                                    }
                                }
                                "title" => value["title"] = json!("Changed title"),
                                "fork" => {
                                    let field = if provider == ReviewProvider::Github {
                                        "repo"
                                    } else {
                                        "repository"
                                    };
                                    value[source][field]["full_name"] = json!("Other/Repo");
                                }
                                "count" => value["changed_files"] = json!(2),
                                _ => {}
                            }
                        }
                        value
                    };
                    (200, String::new(), value.to_string())
                });
                let api = server.api(provider);
                let manager = Manager::default();
                let info = manager
                    .insert(
                        api.capture(DetailParams {
                            repository: identity(provider),
                            pr_id: 7,
                        })
                        .unwrap(),
                    )
                    .unwrap();
                let before = server.requests.lock().unwrap().len();
                let result = api.recheck_snapshot(&manager.get(&info.snapshot).unwrap());
                assert_eq!(
                    result.is_ok(),
                    fault == "none",
                    "{provider:?} {fault}: {result:?}"
                );
                assert_eq!(server.requests.lock().unwrap().len(), before + 1);
                assert_eq!(reads.load(Ordering::SeqCst), 3);
            }
        }
    }

    #[test]
    fn snapshots_reject_changed_metadata_partial_inventory_redirects_and_rate_limits() {
        use crate::services::provider_snapshot::Manager;
        const PATCH: &str = "diff --git a/x b/x\nold mode 100644\nnew mode 100755\n";
        for fault in [
            "head",
            "source",
            "destination",
            "count",
            "truncated",
            "redirect",
            "rate",
        ] {
            let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let server = Server::new(move |path, _| {
                if path.contains("/compare/") && !path.contains('?') {
                    if fault == "redirect" {
                        return (
                            302,
                            "Location: http://127.0.0.1:1/private\r\n".into(),
                            String::new(),
                        );
                    }
                    if fault == "rate" {
                        return (429, "Retry-After: 60\r\n".into(), String::new());
                    }
                    return (200, String::new(), PATCH.into());
                }
                let value = if path.contains("/compare/") {
                    json!({"base_commit":{"sha":"b".repeat(40)},"merge_base_commit":{"sha":"c".repeat(40)}})
                } else {
                    let second = seen.fetch_add(1, Ordering::SeqCst) > 0;
                    let mut value = gh(7, false, "open");
                    value["head"]["repo"] = json!({"full_name": if second && fault == "source" { "Other/Repo" } else { "ForkOwner/ForkRepo" }});
                    value["changed_files"] = json!(if fault == "truncated" { 2 } else { 1 });
                    if second && fault == "head" {
                        value["head"]["sha"] = json!("d".repeat(40));
                    }
                    if second && fault == "destination" {
                        value["base"]["sha"] = json!("d".repeat(40));
                    }
                    if second && fault == "count" {
                        value["changed_files"] = json!(2);
                    }
                    value
                };
                (200, String::new(), value.to_string())
            });
            let result = server
                .api(ReviewProvider::Github)
                .capture(DetailParams {
                    repository: identity(ReviewProvider::Github),
                    pr_id: 7,
                })
                .and_then(|capture| Manager::default().insert(capture));
            assert!(result.is_err(), "accepted fault {fault}");
            let requests = server.requests.lock().unwrap();
            assert!(requests.len() <= 4);
            assert!(!requests.iter().any(|path| path == "/private"));
        }
    }
    #[test]
    fn snapshot_complete_patch_is_not_limited_to_github_compare_json_300_files() {
        use crate::services::provider_snapshot::Manager;
        let patch = (0..301)
            .map(|n| format!("diff --git a/file{n} b/file{n}\nold mode 100644\nnew mode 100755\n"))
            .collect::<String>();
        let server = Server::new(move |path, _| {
            if path.contains("/compare/") && !path.contains('?') {
                return (200, String::new(), patch.clone());
            }
            let value = if path.contains("/compare/") {
                json!({"base_commit":{"sha":"b".repeat(40)},"merge_base_commit":{"sha":"c".repeat(40)},"files":[]})
            } else {
                let mut value = gh(7, false, "open");
                value["changed_files"] = json!(301);
                value
            };
            (200, String::new(), value.to_string())
        });
        let capture = server
            .api(ReviewProvider::Github)
            .capture(DetailParams {
                repository: identity(ReviewProvider::Github),
                pr_id: 7,
            })
            .unwrap();
        assert_eq!(Manager::default().insert(capture).unwrap().file_count, 301);
    }
    #[test]
    fn bitbucket_snapshot_diffstat_pages_validate_exact_continuation_and_scope() {
        use crate::services::provider_snapshot::Manager;
        for fault in [
            "none",
            "link",
            "duplicate",
            "source-changed",
            "missing-source",
        ] {
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let server = Server::new(move |path, request| {
                if path.contains("/diff/") {
                    return (200, String::new(), "diff --git a/x b/x\nold mode 100644\nnew mode 100755\ndiff --git a/y b/y\nold mode 100644\nnew mode 100755\n".into());
                }
                let value = if path.contains("/merge-base/") {
                    json!({"hash":"c".repeat(40)})
                } else if path.contains("/diffstat/") {
                    let second = path.ends_with("page=2");
                    let name = if second && fault != "duplicate" {
                        "y"
                    } else {
                        "x"
                    };
                    let mut value = json!({"page": if second { 2 } else { 1 }, "values":[{"status":"modified","old":{"path":name},"new":{"path":name},"lines_added":0,"lines_removed":0}]});
                    if !second {
                        let host = request
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("host: ")
                                    .or_else(|| line.strip_prefix("Host: "))
                            })
                            .unwrap();
                        let collection = path.split('?').next().unwrap();
                        value["next"] = json!(if fault == "link" {
                            "https://private.example/steal".into()
                        } else {
                            format!("http://{host}{collection}?topic=false&pagelen=100&page=2")
                        });
                    }
                    value
                } else {
                    let second = calls.fetch_add(1, Ordering::SeqCst) > 0;
                    let mut value = bb(7, false);
                    if fault != "missing-source" {
                        value["source"]["repository"] = json!({"full_name": if second && fault == "source-changed" { "Other/Repo" } else { "Fork/Repo" }});
                    }
                    value
                };
                (200, String::new(), value.to_string())
            });
            let result = server
                .api(ReviewProvider::Bitbucket)
                .capture(DetailParams {
                    repository: identity(ReviewProvider::Bitbucket),
                    pr_id: 7,
                })
                .and_then(|capture| Manager::default().insert(capture));
            if fault == "none" {
                assert_eq!(result.unwrap().file_count, 2);
            } else {
                assert!(result.is_err(), "accepted {fault}");
            }
            assert!(!server
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|path| path.contains("steal")));
        }
    }
    #[test]
    fn snapshot_patch_body_limit_and_per_request_deadline_are_real() {
        for fault in ["oversize", "slow"] {
            let server = Server::new(move |path, _| {
                if path.contains("/compare/") && !path.contains('?') {
                    if fault == "slow" {
                        thread::sleep(Duration::from_secs(11));
                    }
                    return (
                        200,
                        String::new(),
                        "x".repeat(crate::services::provider_snapshot::PATCH_BYTES + 1),
                    );
                }
                let value = if path.contains("/compare/") {
                    json!({"base_commit":{"sha":"b".repeat(40)},"merge_base_commit":{"sha":"c".repeat(40)}})
                } else {
                    let mut value = gh(7, false, "open");
                    value["changed_files"] = json!(1);
                    value
                };
                (200, String::new(), value.to_string())
            });
            let started = Instant::now();
            assert!(server
                .api(ReviewProvider::Github)
                .capture(DetailParams {
                    repository: identity(ReviewProvider::Github),
                    pr_id: 7
                })
                .is_err());
            assert!(started.elapsed() < Duration::from_secs(12));
            assert_eq!(
                server
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|path| path.contains("/compare/") && !path.contains('?'))
                    .count(),
                1
            );
        }
    }
    #[test]
    fn snapshot_fixture_waits_for_delayed_fragmented_headers() {
        let server = Server::new(|path, _| {
            assert_eq!(path, "/owned");
            (200, String::new(), "owned response".into())
        });
        let mut socket =
            std::net::TcpStream::connect(server.root.strip_prefix("http://").unwrap()).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        thread::sleep(Duration::from_millis(50));
        socket
            .write_all(b"GET /owned HTTP/1.1\r\nHost: owned-fixture\r\n")
            .unwrap();
        thread::sleep(Duration::from_millis(50));
        socket.write_all(b"Connection: close\r\n\r\n").unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("owned response"));
        assert_eq!(*server.requests.lock().unwrap(), vec!["/owned".to_string()]);
    }
    #[test]
    fn immutable_images_use_verified_fork_git_objects_and_commit_file_metadata() {
        use sha2::Digest;
        let bytes = vec![137, 80, 78, 71, 0, 255, 128, 10];
        let mut object = sha1::Sha1::new();
        object.update(format!("blob {}\0", bytes.len()).as_bytes());
        object.update(&bytes);
        let blob = format!("{:x}", object.finalize());
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let raw = bytes.clone();
            let sha = blob.clone();
            let server = Server::new_bytes(move |path, request| {
                assert!(path.starts_with(if provider == ReviewProvider::Github {
                    "/repos/Fork/Source/"
                } else {
                    "/repositories/Fork/Source/"
                }));
                if path.contains("/git/blobs/") || path.ends_with("/dir/a%20b.png") {
                    if provider == ReviewProvider::Github {
                        assert!(request
                            .to_lowercase()
                            .contains("accept: application/vnd.github.raw+json"));
                    }
                    return (200, String::new(), raw.clone());
                }
                let value = if path.contains("/git/commits/") {
                    json!({"sha":"a".repeat(40),"tree":{"sha":"c".repeat(40)}})
                } else if path.ends_with(&format!("/git/trees/{}", "c".repeat(40))) {
                    json!({"sha":"c".repeat(40),"truncated":false,"tree":[{"path":"dir","type":"tree","mode":"040000","sha":"d".repeat(40)}]})
                } else if path.contains("/git/trees/") {
                    json!({"sha":"d".repeat(40),"truncated":false,"tree":[{"path":"a b.png","type":"blob","mode":"100644","sha":sha,"size":raw.len()}]})
                } else {
                    assert!(path.ends_with("/dir/a%20b.png?format=meta"));
                    json!({"type":"commit_file","path":"dir/a b.png","commit":{"hash":"a".repeat(40)},"size":raw.len(),"attributes":["binary"]})
                };
                (200, String::new(), value.to_string().into_bytes())
            });
            let source = RepositoryIdentity {
                provider,
                workspace: "Fork".into(),
                repo: "Source".into(),
            };
            assert_eq!(
                server
                    .api(provider)
                    .immutable_image(&source, &"a".repeat(40), "dir/a b.png")
                    .unwrap(),
                bytes
            );
            let requests = server.requests.lock().unwrap();
            assert_eq!(
                requests.len(),
                if provider == ReviewProvider::Github {
                    4
                } else {
                    2
                }
            );
            assert!(!requests.iter().any(|path| path.contains("recursive")
                || path.contains("contents")
                || path.contains("pullrequests")
                || path.contains("pulls")));
        }
    }

    #[test]
    fn immutable_images_reject_links_submodules_unverified_pins_partial_trees_and_redirects() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            for fault in [
                "link",
                "submodule",
                "commit",
                "path",
                "size",
                "truncated",
                "duplicate",
                "tree",
                "format",
                "hash",
                "redirect",
                "rate",
                "missing",
                "body",
            ] {
                if provider == ReviewProvider::Bitbucket
                    && matches!(fault, "truncated" | "duplicate" | "hash")
                {
                    continue;
                }
                let server = Server::new_bytes(move |path, _| {
                    if fault == "redirect" {
                        return (
                            302,
                            "Location: http://127.0.0.1:1/private\r\n".into(),
                            Vec::new(),
                        );
                    }
                    if fault == "rate" {
                        return (429, "Retry-After: 60\r\n".into(), Vec::new());
                    }
                    if fault == "missing" {
                        return (404, String::new(), Vec::new());
                    }
                    if path.contains("/git/blobs/")
                        || (provider == ReviewProvider::Bitbucket && !path.contains("?format=meta"))
                    {
                        return (
                            200,
                            String::new(),
                            if fault == "body" {
                                vec![0; 2 * 1024 * 1024 + 1]
                            } else {
                                vec![0, 1]
                            },
                        );
                    }
                    let value = if path.contains("/git/commits/") {
                        json!({"sha":if fault == "commit" { "b".repeat(40) } else { "a".repeat(40) },"tree":{"sha":"c".repeat(40)}})
                    } else if path.contains("/git/trees/") {
                        let entry = json!({"path":if fault == "path" { "other.png" } else { "a.png" },"type":if fault == "submodule" { "commit" } else { "blob" },"mode":if fault == "link" { "120000" } else if fault == "submodule" { "160000" } else { "100644" },"sha":if fault == "format" { "d".repeat(64) } else { "d".repeat(40) },"size":if fault == "size" { 2097153 } else { 2 }});
                        json!({"sha":if fault == "tree" { "e".repeat(40) } else { "c".repeat(40) },"truncated":fault == "truncated","tree":if fault == "duplicate" { vec![entry.clone(), entry] } else { vec![entry] }})
                    } else {
                        json!({"type":if fault == "tree" { "commit_directory" } else { "commit_file" },"path":if fault == "path" { "other.png" } else { "a.png" },"commit":{"hash":if fault == "commit" { "b".repeat(40) } else { "a".repeat(40) }},"size":if fault == "size" { 2097153 } else { 2 },"attributes":[if fault == "link" { "link" } else if fault == "submodule" { "subrepository" } else if fault == "format" { "unknown" } else { "binary" }]})
                    };
                    (200, String::new(), value.to_string().into_bytes())
                });
                assert!(
                    server
                        .api(provider)
                        .immutable_image(&identity(provider), &"a".repeat(40), "a.png")
                        .is_err(),
                    "{provider:?}/{fault}"
                );
                let requests = server.requests.lock().unwrap();
                if matches!(
                    fault,
                    "link"
                        | "submodule"
                        | "commit"
                        | "path"
                        | "size"
                        | "tree"
                        | "format"
                        | "redirect"
                        | "rate"
                        | "missing"
                ) {
                    assert!(!requests.iter().any(|path| path.contains("/git/blobs/")
                        || provider == ReviewProvider::Bitbucket
                            && !path.contains("?format=meta")));
                }
            }
        }
        let server = Server::new(|_, _| panic!("invalid image request reached HTTP"));
        let api = server.api(ReviewProvider::Github);
        for path in ["../x.png", "/x.png", "a\\b.png", "C:/x.png", "a//b.png"] {
            assert!(api
                .immutable_image(&identity(ReviewProvider::Github), &"a".repeat(40), path)
                .is_err());
        }
        assert!(api
            .immutable_image(
                &identity(ReviewProvider::Bitbucket),
                &"a".repeat(40),
                "x.png"
            )
            .is_err());
        assert!(api
            .immutable_image(&identity(ReviewProvider::Github), "main", "x.png")
            .is_err());
        assert!(server.requests.lock().unwrap().is_empty());
    }
}
