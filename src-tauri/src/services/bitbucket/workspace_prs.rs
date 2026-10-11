//! Bounded provider reads for the terminal workspace (ADR 0020, #357).
use super::*;
use reqwest::{blocking::Client, redirect::Policy, Url};
use serde_json::Value;
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
        let remaining = TOTAL_TIME
            .checked_sub(started.elapsed())
            .ok_or_else(error)?;
        let request = self
            .http
            .get(url)
            .timeout(remaining.min(Duration::from_secs(10)));
        let request = match &self.auth {
            Auth::Github(token) => request
                .bearer_auth(token)
                .header("Accept", "application/vnd.github+json"),
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
                .is_some_and(|n| n > BODY_BYTES as u64)
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
            .take((BODY_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| error())?;
        if bytes.len() > BODY_BYTES || started.elapsed() >= TOTAL_TIME {
            return Err(error());
        }
        Ok((serde_json::from_slice(&bytes).map_err(|_| error())?, link))
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
        let author_id = if self.repository.provider == ReviewProvider::Github {
            json.get("user").and_then(|v| v.get("login"))
        } else {
            json.get("author").and_then(|v| v.get("account_id"))
        }
        .and_then(Value::as_str)
        .map(str::to_owned);
        let mut detail = if self.repository.provider == ReviewProvider::Github {
            let raw: GhPullRequest = serde_json::from_value(json).map_err(|_| error())?;
            if !matches!(raw.state.as_str(), "open" | "closed") {
                return Err(error());
            }
            map_gh_pr_detail(raw)
        } else {
            map_pr_detail(serde_json::from_value(json).map_err(|_| error())?)
        };
        if detail.id != params.pr_id
            || !matches!(
                detail.state.as_str(),
                "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED"
            )
        {
            return Err(error());
        }
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
            if !(7..=64).contains(&revision.len())
                || !revision.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(error());
            }
        }
        if let Some(id) = &author_id {
            safe_text(id, 256, false)?;
        }
        validate_reviewers(&detail.reviewers)?;
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
                                let response = format!("HTTP/1.1 {status} Fixture\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n{body}", body.len());
                                let _ = stream.write_all(response.as_bytes());
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
}
