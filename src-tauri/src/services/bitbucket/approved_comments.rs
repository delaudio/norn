//! Bounded, at-most-once adapters for explicitly approved local draft comments.
use super::{credentials, dry_run, Credentials, ReviewProvider};
use crate::draft_publication::{
    ApprovedComment, ApprovedCommentApi, CommentMode, CommentWrite, PublicationScope,
    RemoteRevision,
};
use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Method, StatusCode, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::Read,
    time::{Duration, Instant},
};

const PAGE_BYTES: usize = 1024 * 1024;
const SCAN_BYTES: usize = 16 * 1024 * 1024;
const MAX_COMMENTS: usize = 1000;
const MAX_PAGES: usize = 20;
const TOTAL_READ_TIME: Duration = Duration::from_secs(20);
const REQUEST_TIME: Duration = Duration::from_secs(10);
const BB_FIELDS: &str = "next,values.id,values.deleted,values.created_on,values.user.account_id,values.user.display_name,values.content.raw,values.inline.path,values.inline.to,values.inline.from,values.inline.start_to,values.inline.start_from,values.parent.id";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteComment {
    pub id: String,
    pub parent_id: Option<String>,
    pub author_id: String,
    pub author_name: String,
    pub raw: String,
    pub created_at: String,
    pub deleted: bool,
    pub mode: CommentMode,
    pub path: String,
    pub to: Option<u32>,
    pub from: Option<u32>,
    pub commit_id: Option<String>,
    pub range_start: Option<u32>,
}
enum NativeAuth {
    Bitbucket(Credentials),
    Github(String),
}
pub struct StoredApprovedCommentApi {
    http: Client,
    auth: NativeAuth,
    provider: ReviewProvider,
    root: String,
    author: std::sync::OnceLock<Result<String, String>>,
}
fn error() -> String {
    "The provider comment response could not be verified. No automatic retry was attempted.".into()
}
fn bounded_json(mut response: Response, maximum: usize) -> Result<(Value, usize), String> {
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size > maximum as u64)
    {
        return Err(error());
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| error())?;
    if bytes.len() > maximum {
        return Err(error());
    }
    serde_json::from_slice(&bytes)
        .map(|value| (value, bytes.len()))
        .map_err(|_| error())
}
fn text(value: &Value, key: &str, maximum: usize, multiline: bool) -> Result<String, String> {
    let value = value.get(key).and_then(Value::as_str).unwrap_or("");
    if value.len() > maximum
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\r' | '\n' | '\t')))
    {
        return Err(error());
    }
    Ok(value.into())
}
fn numeric_id(value: &Value) -> Result<String, String> {
    let id = match value {
        Value::Number(number) => number
            .as_u64()
            .filter(|id| *id > 0)
            .map(|id| id.to_string()),
        Value::String(id)
            if !id.starts_with('0') && id.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            id.parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .map(|id| id.to_string())
        }
        _ => None,
    };
    id.ok_or_else(error)
}
fn optional_id(value: Option<&Value>) -> Result<Option<String>, String> {
    value
        .filter(|value| !value.is_null())
        .map(numeric_id)
        .transpose()
}
fn optional_line(value: Option<&Value>) -> Result<Option<u32>, String> {
    value
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_u64()
                .and_then(|line| u32::try_from(line).ok())
                .filter(|line| *line > 0)
                .ok_or_else(error)
        })
        .transpose()
}
fn parse_comment(
    provider: ReviewProvider,
    value: &Value,
    scope_url: &str,
    review: bool,
) -> Result<RemoteComment, String> {
    let id = numeric_id(value.get("id").ok_or_else(error)?)?;
    let user = value.get("user").unwrap_or(&Value::Null);
    match provider {
        ReviewProvider::Bitbucket => {
            let anchor = value.get("inline").filter(|value| !value.is_null());
            let (path, to, from) = if let Some(anchor) = anchor {
                (
                    text(anchor, "path", 4096, false)?,
                    optional_line(anchor.get("to"))?,
                    optional_line(anchor.get("from"))?,
                )
            } else {
                (String::new(), None, None)
            };
            if to.is_some() && from.is_some() {
                return Err(error());
            }
            Ok(RemoteComment {
                id,
                parent_id: optional_id(value.get("parent").and_then(|parent| parent.get("id")))?,
                author_id: text(user, "account_id", 256, false)?,
                author_name: text(user, "display_name", 256, false)?,
                raw: text(
                    value.get("content").unwrap_or(&Value::Null),
                    "raw",
                    64 * 1024,
                    true,
                )?,
                created_at: text(value, "created_on", 256, false)?,
                deleted: value
                    .get("deleted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                mode: if anchor.is_none() {
                    CommentMode::General
                } else if to.is_some() || from.is_some() {
                    CommentMode::Inline
                } else {
                    CommentMode::File
                },
                path,
                to,
                from,
                commit_id: None,
                range_start: anchor
                    .map(|value| {
                        optional_line(value.get(if from.is_some() {
                            "start_from"
                        } else {
                            "start_to"
                        }))
                    })
                    .transpose()?
                    .flatten(),
            })
        }
        ReviewProvider::Github => {
            let returned_scope = text(
                value,
                if review {
                    "pull_request_url"
                } else {
                    "issue_url"
                },
                4096,
                false,
            )?;
            if !returned_scope.eq_ignore_ascii_case(scope_url) {
                return Err(error());
            }
            let path = if review {
                text(value, "path", 4096, false)?
            } else {
                String::new()
            };
            let line = if review {
                optional_line(value.get("line"))?.or(optional_line(value.get("original_line"))?)
            } else {
                None
            };
            let side = text(value, "side", 32, false)?;
            if review && line.is_some() && !matches!(side.as_str(), "LEFT" | "RIGHT") {
                return Err(error());
            }
            let (to, from) = if side == "LEFT" {
                (None, line)
            } else {
                (line, None)
            };
            let commit_id = if review {
                let sha = text(value, "original_commit_id", 64, false)?;
                Some(if sha.is_empty() {
                    text(value, "commit_id", 64, false)?
                } else {
                    sha
                })
            } else {
                None
            };
            Ok(RemoteComment {
                id,
                parent_id: optional_id(value.get("in_reply_to_id"))?,
                author_id: text(user, "login", 256, false)?,
                author_name: text(user, "login", 256, false)?,
                raw: text(value, "body", 64 * 1024, true)?,
                created_at: text(value, "created_at", 256, false)?,
                deleted: false,
                mode: if !review {
                    CommentMode::General
                } else if line.is_some() {
                    CommentMode::Inline
                } else {
                    CommentMode::File
                },
                path,
                to,
                from,
                commit_id,
                range_start: if review {
                    optional_line(value.get("original_start_line"))?
                        .or(optional_line(value.get("start_line"))?)
                } else {
                    None
                },
            })
        }
    }
}
fn safe_pagination(value: &str, endpoint: &str) -> Result<String, String> {
    let next = Url::parse(value).map_err(|_| error())?;
    let expected = Url::parse(endpoint).map_err(|_| error())?;
    // The production root is fixed HTTPS. Only private owned test instances
    // use a loopback HTTP root; provider responses cannot change that root.
    if next.scheme() != expected.scheme()
        || next.host_str() != expected.host_str()
        || next.port_or_known_default() != expected.port_or_known_default()
        || next.path() != expected.path()
        || !next.username().is_empty()
        || next.password().is_some()
        || next.fragment().is_some()
        || next.query().is_none()
    {
        return Err(error());
    }
    Ok(value.into())
}
impl StoredApprovedCommentApi {
    pub fn from_stored(provider: ReviewProvider) -> Result<Self, String> {
        let auth = match provider {
            ReviewProvider::Bitbucket => NativeAuth::Bitbucket(
                credentials::load().ok_or("Bitbucket credentials are unavailable.")?,
            ),
            ReviewProvider::Github => NativeAuth::Github(
                credentials::load_github_token().ok_or("GitHub credentials are unavailable.")?,
            ),
        };
        Self::with_auth(provider, auth)
    }
    fn with_auth(provider: ReviewProvider, auth: NativeAuth) -> Result<Self, String> {
        let builder = Client::builder()
            .user_agent("norn")
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(REQUEST_TIME);
        #[cfg(test)]
        let builder = builder.no_proxy();
        let http = builder.build().map_err(|_| error())?;
        Ok(Self {
            http,
            auth,
            author: std::sync::OnceLock::new(),
            provider,
            root: if provider == ReviewProvider::Github {
                "https://api.github.com"
            } else {
                "https://api.bitbucket.org/2.0"
            }
            .into(),
        })
    }
    pub fn create_once_cancellable(
        &self,
        comment: &ApprovedComment,
        stopped: &dyn Fn() -> bool,
    ) -> CommentWrite {
        // Dry-run is a no-write rejection, never a fabricated durable acknowledgment.
        if stopped() || dry_run() {
            return CommentWrite::Rejected;
        }
        let Ok(body) = self.write_body(comment) else {
            return CommentWrite::Rejected;
        };
        let Ok(base) = self.scope_base(&comment.scope) else {
            return CommentWrite::Rejected;
        };
        let Ok(author) = self.authenticated_author() else {
            return CommentWrite::Rejected;
        };
        if self.validate_supported(comment).is_err() {
            return CommentWrite::Rejected;
        }
        if !self.current_revision(&comment.scope).is_ok_and(|revision| {
            revision.head_sha.eq_ignore_ascii_case(&comment.head_sha)
                && revision.base_sha.eq_ignore_ascii_case(&comment.base_sha)
        }) {
            return CommentWrite::Rejected;
        }
        let review = comment.mode != CommentMode::General;
        let scope_url = format!(
            "{base}/{}/{id}",
            if self.provider == ReviewProvider::Github {
                if review {
                    "pulls"
                } else {
                    "issues"
                }
            } else {
                "pullrequests"
            },
            id = comment.scope.pr_id
        );
        let url = if self.provider == ReviewProvider::Github && comment.parent_id.is_some() {
            format!(
                "{scope_url}/comments/{}/replies",
                comment.parent_id.as_deref().unwrap_or_default()
            )
        } else {
            format!("{scope_url}/comments")
        };
        if stopped() {
            return CommentWrite::Rejected;
        }
        // Deliberately no send_checked/rate-limit retry helper for POST.
        let Ok(response) = self.request(Method::POST, &url).json(&body).send() else {
            return CommentWrite::Unknown;
        };
        if response.status() != StatusCode::CREATED {
            return CommentWrite::Unknown;
        }
        let Ok((value, _)) = bounded_json(response, PAGE_BYTES) else {
            return CommentWrite::Unknown;
        };
        match parse_comment(self.provider, &value, &scope_url, review) {
            Ok(actual) if self.matches(&actual, comment, &author) => {
                CommentWrite::Created(actual.id)
            }
            _ => CommentWrite::Unknown,
        }
    }
    fn scope_base(&self, scope: &PublicationScope) -> Result<String, String> {
        if self.provider != scope.provider
            || scope.pr_id == 0
            || crate::local_review::is_local_review_id(u64::from(scope.pr_id))
            || [&scope.workspace, &scope.repo].iter().any(|value| {
                value.trim().is_empty()
                    || value.len() > 256
                    || value.chars().any(char::is_control)
                    || (matches!(value.as_str(), "." | "..") || value.contains(['/', '\\', ':']))
            })
        {
            return Err("Select a valid provider pull request.".into());
        }
        let mut root = Url::parse(&self.root).map_err(|_| error())?;
        root.path_segments_mut().map_err(|_| error())?.extend([
            if scope.provider == ReviewProvider::Github {
                "repos"
            } else {
                "repositories"
            },
            &scope.workspace,
            &scope.repo,
        ]);
        Ok(root.to_string().trim_end_matches('/').into())
    }
    fn request(&self, method: Method, url: &str) -> RequestBuilder {
        let request = self.http.request(method, url).header(
            reqwest::header::ACCEPT,
            if self.provider == ReviewProvider::Github {
                "application/vnd.github+json"
            } else {
                "application/json"
            },
        );
        match &self.auth {
            NativeAuth::Bitbucket(auth) => request.basic_auth(&auth.username, Some(&auth.token)),
            NativeAuth::Github(token) => request.bearer_auth(token),
        }
    }
    fn get(&self, url: &str) -> Result<(Value, usize), String> {
        bounded_json(
            self.request(Method::GET, url).send().map_err(|_| error())?,
            PAGE_BYTES,
        )
    }
    fn authenticated_author(&self) -> Result<String, String> {
        self.author
            .get_or_init(|| {
                let (value, _) = self.get(&format!("{}/user", self.root))?;
                let author = text(
                    &value,
                    if self.provider == ReviewProvider::Github {
                        "login"
                    } else {
                        "account_id"
                    },
                    256,
                    false,
                )?;
                if author.is_empty() {
                    return Err(error());
                }
                Ok(author)
            })
            .clone()
    }
    fn get_before(&self, url: &str, deadline: Instant) -> Result<(Value, usize), String> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(error)?;
        bounded_json(
            self.request(Method::GET, url)
                .timeout(remaining.min(REQUEST_TIME))
                .send()
                .map_err(|_| error())?,
            PAGE_BYTES,
        )
    }
    pub fn list_comments(&self, scope: &PublicationScope) -> Result<Vec<RemoteComment>, String> {
        let base = self.scope_base(scope)?;
        let started = Instant::now();
        let mut bytes = 0;
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        if self.provider == ReviewProvider::Bitbucket {
            let endpoint = format!("{base}/pullrequests/{}/comments", scope.pr_id);
            let mut next = Some(format!("{endpoint}?pagelen=100&fields={BB_FIELDS}"));
            for _ in 0..MAX_PAGES {
                let Some(url) = next.take() else {
                    return Ok(out);
                };
                if started.elapsed() > TOTAL_READ_TIME || !seen.insert(url.clone()) {
                    return Err(error());
                }
                let (page, size) = self.get_before(&url, started + TOTAL_READ_TIME)?;
                bytes += size;
                let values = page
                    .get("values")
                    .and_then(Value::as_array)
                    .ok_or_else(error)?;
                if bytes > SCAN_BYTES
                    || values.len() > 100
                    || out.len() + values.len() > MAX_COMMENTS
                {
                    return Err(error());
                }
                for value in values {
                    out.push(parse_comment(self.provider, value, &endpoint, true)?);
                }
                next = page
                    .get("next")
                    .filter(|value| !value.is_null())
                    .map(|value| {
                        let raw = value.as_str().ok_or_else(error)?;
                        safe_pagination(raw, &endpoint)
                    })
                    .transpose()?;
            }
            if next.is_some() {
                return Err(error());
            }
        } else {
            for review in [true, false] {
                let scope_url = format!(
                    "{base}/{}/{id}",
                    if review { "pulls" } else { "issues" },
                    id = scope.pr_id
                );
                let endpoint = format!("{scope_url}/comments");
                let mut complete = false;
                for page in 1..=MAX_PAGES {
                    if started.elapsed() > TOTAL_READ_TIME {
                        return Err(error());
                    }
                    let remaining = (started + TOTAL_READ_TIME)
                        .checked_duration_since(Instant::now())
                        .filter(|duration| !duration.is_zero())
                        .ok_or_else(error)?;
                    let response = self
                        .request(Method::GET, &format!("{endpoint}?per_page=100&page={page}"))
                        .timeout(remaining.min(REQUEST_TIME))
                        .send()
                        .map_err(|_| error())?;
                    // Follow only locally constructed page numbers; no provider URL receives credentials.
                    let has_next = response
                        .headers()
                        .get(reqwest::header::LINK)
                        .and_then(|value| value.to_str().ok())
                        .is_some_and(|value| {
                            value.split(',').any(|entry| entry.contains("rel=\"next\""))
                        });
                    let (values, size) = bounded_json(response, PAGE_BYTES)?;
                    bytes += size;
                    let values = values.as_array().ok_or_else(error)?;
                    if bytes > SCAN_BYTES
                        || values.len() > 100
                        || out.len() + values.len() > MAX_COMMENTS
                    {
                        return Err(error());
                    }
                    for value in values {
                        out.push(parse_comment(self.provider, value, &scope_url, review)?);
                    }
                    if !has_next {
                        complete = true;
                        break;
                    }
                }
                if !complete {
                    return Err(error());
                }
            }
        }
        out.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(out)
    }
    fn matches(&self, actual: &RemoteComment, comment: &ApprovedComment, author: &str) -> bool {
        !actual.deleted
            && actual.author_id.eq_ignore_ascii_case(author)
            && actual.raw == comment.markdown()
            && actual.parent_id == comment.parent_id
            && actual.mode == comment.mode
            && actual.path == comment.path
            && actual.to == comment.to
            && actual.from == comment.from
            && actual
                .range_start
                .is_none_or(|start| Some(start) == comment.to.or(comment.from))
            && (self.provider != ReviewProvider::Github
                || comment.mode == CommentMode::General
                || actual
                    .commit_id
                    .as_deref()
                    .is_some_and(|sha| sha.eq_ignore_ascii_case(&comment.head_sha)))
    }
    fn read_diff(&self, scope: &PublicationScope) -> Result<String, String> {
        let base = self.scope_base(scope)?;
        let mut url = format!(
            "{base}/{}/{}/diff",
            if self.provider == ReviewProvider::Github {
                "pulls"
            } else {
                "pullrequests"
            },
            scope.pr_id
        );
        if self.provider == ReviewProvider::Github {
            url = format!("{base}/pulls/{}", scope.pr_id);
        }
        let deadline = Instant::now() + TOTAL_READ_TIME;
        let mut visited = HashSet::new();
        for _ in 0..3 {
            if !visited.insert(url.clone()) {
                return Err(error());
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(error)?;
            let mut response = self
                .request(Method::GET, &url)
                .header(
                    reqwest::header::ACCEPT,
                    if self.provider == ReviewProvider::Github {
                        "application/vnd.github.v3.diff"
                    } else {
                        "text/plain"
                    },
                )
                .timeout(remaining.min(REQUEST_TIME))
                .send()
                .map_err(|_| error())?;
            if response.status().is_redirection() && self.provider == ReviewProvider::Bitbucket {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(error)?;
                let next = Url::parse(&url)
                    .map_err(|_| error())?
                    .join(location)
                    .map_err(|_| error())?;
                let expected = Url::parse(&base).map_err(|_| error())?;
                if next.scheme() != expected.scheme()
                    || next.host_str() != expected.host_str()
                    || next.port_or_known_default() != expected.port_or_known_default()
                    || !next
                        .path()
                        .starts_with(&format!("{}/diff/", expected.path()))
                    || !next.username().is_empty()
                    || next.password().is_some()
                    || next.fragment().is_some()
                {
                    return Err(error());
                }
                url = next.into();
                continue;
            }
            const DIFF_BYTES: usize = 8 * 1024 * 1024;
            if !response.status().is_success()
                || response
                    .content_length()
                    .is_some_and(|size| size > DIFF_BYTES as u64)
            {
                return Err(error());
            }
            let mut bytes = Vec::new();
            response
                .by_ref()
                .take((DIFF_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| error())?;
            if bytes.len() > DIFF_BYTES {
                return Err("The provider patch exceeds the publication validation limit. Preserve this draft and choose a supported position explicitly.".into());
            }
            return String::from_utf8(bytes).map_err(|_| error());
        }
        Err(error())
    }
    fn changed_paths(&self, scope: &PublicationScope) -> Result<HashSet<String>, String> {
        let base = self.scope_base(scope)?;
        let endpoint = format!(
            "{base}/{}/{}/{}",
            if self.provider == ReviewProvider::Github {
                "pulls"
            } else {
                "pullrequests"
            },
            scope.pr_id,
            if self.provider == ReviewProvider::Github {
                "files"
            } else {
                "diffstat"
            }
        );
        let deadline = Instant::now() + TOTAL_READ_TIME;
        let mut next = Some(format!(
            "{endpoint}?pagelen=100&fields=next,values.new.path,values.old.path"
        ));
        let mut visited = HashSet::new();
        let mut paths = HashSet::new();
        let mut count = 0;
        let mut bytes = 0;
        for page in 1..=32 {
            let url = if self.provider == ReviewProvider::Github {
                format!("{endpoint}?per_page=100&page={page}")
            } else {
                let Some(url) = next.take() else {
                    return Ok(paths);
                };
                url
            };
            if !visited.insert(url.clone()) {
                return Err(error());
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(error)?;
            let response = self
                .request(Method::GET, &url)
                .timeout(remaining.min(REQUEST_TIME))
                .send()
                .map_err(|_| error())?;
            let gh_next = response
                .headers()
                .get(reqwest::header::LINK)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.split(',').any(|entry| entry.contains("rel=\"next\"")));
            let (value, size) = bounded_json(response, PAGE_BYTES)?;
            bytes += size;
            let values = if self.provider == ReviewProvider::Github {
                value.as_array()
            } else {
                value.get("values").and_then(Value::as_array)
            }
            .ok_or_else(error)?;
            count += values.len();
            if values.len() > 100 || count > 3000 || bytes > SCAN_BYTES {
                return Err(error());
            }
            for value in values {
                let path = if self.provider == ReviewProvider::Github {
                    text(value, "filename", 4096, false)?
                } else {
                    let new = value.get("new").filter(|value| !value.is_null());
                    text(
                        new.or_else(|| value.get("old")).unwrap_or(&Value::Null),
                        "path",
                        4096,
                        false,
                    )?
                };
                if path.is_empty() {
                    return Err(error());
                }
                paths.insert(path);
            }
            if self.provider == ReviewProvider::Github {
                if !gh_next {
                    return Ok(paths);
                }
            } else {
                next = value
                    .get("next")
                    .filter(|value| !value.is_null())
                    .map(|value| safe_pagination(value.as_str().ok_or_else(error)?, &endpoint))
                    .transpose()?;
                if next.is_none() {
                    return Ok(paths);
                }
            }
        }
        Err(error())
    }
    pub fn validate_positions(&self, comments: &[ApprovedComment]) -> Result<(), String> {
        let Some(first) = comments.first() else {
            return Err("Select at least one draft.".into());
        };
        if comments.len() > 20 || comments.iter().any(|comment| comment.scope != first.scope) {
            return Err(
                "Publication preview requires at most 20 drafts from one pull request.".into(),
            );
        }
        for comment in comments {
            self.validate_supported(comment)?;
        }
        let matches = |revision: &RemoteRevision| {
            comments.iter().all(|comment| {
                revision.head_sha.eq_ignore_ascii_case(&comment.head_sha)
                    && revision.base_sha.eq_ignore_ascii_case(&comment.base_sha)
            })
        };
        if !matches(&self.current_revision(&first.scope)?) {
            return Err("The pull request changed since these drafts were reviewed. No publication was approved.".into());
        }
        if comments
            .iter()
            .any(|comment| comment.mode == CommentMode::Inline && comment.parent_id.is_none())
        {
            let patch = self.read_diff(&first.scope)?;
            super::approved_positions::validate_inline_positions(&patch, comments)?;
        }
        if comments
            .iter()
            .any(|comment| comment.mode == CommentMode::File && comment.parent_id.is_none())
        {
            let paths = self.changed_paths(&first.scope)?;
            if comments.iter().any(|comment| {
                comment.mode == CommentMode::File
                    && comment.parent_id.is_none()
                    && !paths.contains(&comment.path)
            }) {
                return Err("A selected file is absent from this pull request's changed files. No anchor was relocated.".into());
            }
        }
        if !matches(&self.current_revision(&first.scope)?) {
            return Err(
                "The pull request changed during preview validation. No publication was approved."
                    .into(),
            );
        }
        Ok(())
    }
    pub fn validate_supported(&self, comment: &ApprovedComment) -> Result<(), String> {
        if dry_run() {
            return Err("Comment publication is disabled by dry-run mode.".into());
        }
        self.write_body(comment)?;
        self.authenticated_author()?;
        if let Some(parent) = &comment.parent_id {
            let comments = self.list_comments(&comment.scope)?;
            if !comments.iter().any(|actual| {
                &actual.id == parent
                    && !actual.deleted
                    && actual.parent_id.is_none()
                    && actual.mode == comment.mode
                    && actual.path == comment.path
                    && actual.to == comment.to
                    && actual.from == comment.from
                    && actual
                        .range_start
                        .is_none_or(|start| Some(start) == comment.to.or(comment.from))
                    && (self.provider != ReviewProvider::Github
                        || actual
                            .commit_id
                            .as_deref()
                            .is_some_and(|sha| sha.eq_ignore_ascii_case(&comment.head_sha)))
            }) {
                return Err("The selected root thread no longer matches this draft's original anchor and revision. No reply can be sent.".into());
            }
        }
        Ok(())
    }
    fn write_body(&self, comment: &ApprovedComment) -> Result<Value, String> {
        comment.validate()?;
        self.scope_base(&comment.scope)?;
        if let Some(parent) = &comment.parent_id {
            numeric_id(&Value::String(parent.clone()))?;
        }
        if self.provider == ReviewProvider::Bitbucket {
            let mut body = json!({"content":{"raw":comment.markdown()}});
            if comment.mode != CommentMode::General {
                body["inline"] = json!({"path":comment.path});
                if let Some(line) = comment.to {
                    body["inline"]["to"] = json!(line);
                }
                if let Some(line) = comment.from {
                    body["inline"]["from"] = json!(line);
                }
            }
            if let Some(parent) = &comment.parent_id {
                body["parent"] = json!({"id":parent.parse::<u64>().map_err(|_| error())?});
            }
            Ok(body)
        } else {
            if comment.parent_id.is_some() {
                if comment.mode == CommentMode::General {
                    return Err("GitHub general comments do not support a reply parent. Stage a review-thread reply explicitly.".into());
                }
                return Ok(json!({"body":comment.markdown()}));
            }
            let mut body = json!({"body":comment.markdown()});
            if comment.mode != CommentMode::General {
                body["commit_id"] = json!(comment.head_sha);
                body["path"] = json!(comment.path);
                body["subject_type"] = json!(if comment.mode == CommentMode::File {
                    "file"
                } else {
                    "line"
                });
                if let Some(line) = comment.to.or(comment.from) {
                    body["line"] = json!(line);
                    body["side"] = json!(if comment.from.is_some() {
                        "LEFT"
                    } else {
                        "RIGHT"
                    });
                }
            }
            Ok(body)
        }
    }
}
impl ApprovedCommentApi for StoredApprovedCommentApi {
    fn current_revision(&self, scope: &PublicationScope) -> Result<RemoteRevision, String> {
        let base = self.scope_base(scope)?;
        let (value, _) = self.get(&format!(
            "{base}/{}/{id}",
            if self.provider == ReviewProvider::Github {
                "pulls"
            } else {
                "pullrequests"
            },
            id = scope.pr_id
        ))?;
        let id_field = if self.provider == ReviewProvider::Github {
            "number"
        } else {
            "id"
        };
        if value.get(id_field).and_then(Value::as_u64) != Some(u64::from(scope.pr_id)) {
            return Err(error());
        }
        let (head, base) = if self.provider == ReviewProvider::Github {
            (value.pointer("/head/sha"), value.pointer("/base/sha"))
        } else {
            (
                value.pointer("/source/commit/hash"),
                value.pointer("/destination/commit/hash"),
            )
        };
        let head_sha = head.and_then(Value::as_str).ok_or_else(error)?;
        let base_sha = base.and_then(Value::as_str).ok_or_else(error)?;
        if [head_sha, base_sha].iter().any(|sha| {
            !matches!(sha.len(), 40 | 64) || !sha.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(error());
        }
        Ok(RemoteRevision {
            head_sha: head_sha.into(),
            base_sha: base_sha.into(),
        })
    }
    fn create_once(&self, comment: &ApprovedComment) -> CommentWrite {
        self.create_once_cancellable(comment, &|| false)
    }
    fn reconcile(&self, comment: &ApprovedComment) -> Result<Option<String>, String> {
        comment.validate()?;
        let author = self.authenticated_author()?;
        let comments = self.list_comments(&comment.scope)?;
        let matches = comments
            .iter()
            .filter(|actual| self.matches(actual, comment, &author))
            .collect::<Vec<_>>();
        match matches.as_slice() { [] => Ok(None), [actual] => Ok(Some(actual.id.clone())), _ => Err("Multiple matching remote comments require manual inspection; no automatic retry is allowed.".into()) }
    }
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
    fn comment(provider: ReviewProvider, mode: CommentMode) -> ApprovedComment {
        ApprovedComment {
            scope: PublicationScope {
                provider,
                workspace: "fixture".into(),
                repo: "demo".into(),
                pr_id: 9,
            },
            local_id: "owned-draft".into(),
            head_sha: "a".repeat(40),
            base_sha: "b".repeat(40),
            mode,
            path: if mode == CommentMode::General {
                String::new()
            } else {
                "src/time:stamp.rs".into()
            },
            to: None,
            from: if mode == CommentMode::Inline {
                Some(7)
            } else {
                None
            },
            parent_id: None,
            raw: "original è\r\nbody".into(),
            finding_ref: None,
            original_draft_hash: None,
        }
    }
    fn api(provider: ReviewProvider) -> StoredApprovedCommentApi {
        StoredApprovedCommentApi::with_auth(
            provider,
            if provider == ReviewProvider::Github {
                NativeAuth::Github("synthetic-test-token".into())
            } else {
                NativeAuth::Bitbucket(Credentials {
                    username: "synthetic-user".into(),
                    token: "synthetic-test-token".into(),
                })
            },
        )
        .unwrap()
    }
    fn remote(comment: &ApprovedComment, root: &str, author: &str, id: u64) -> Value {
        if comment.scope.provider == ReviewProvider::Bitbucket {
            let mut value = json!({"id":id,"content":{"raw":comment.markdown()},"user":{"account_id":author,"display_name":"Synthetic reviewer"},"created_on":"2026-10-11T00:00:00Z","deleted":false});
            if comment.mode != CommentMode::General {
                value["inline"] = json!({"path":comment.path,"to":comment.to,"from":comment.from});
            }
            if let Some(parent) = &comment.parent_id {
                value["parent"] = json!({"id":parent.parse::<u64>().unwrap()});
            }
            value
        } else {
            let mut value = json!({"id":id,"body":comment.markdown(),"user":{"login":author},"created_at":"2026-10-11T00:00:00Z"});
            if comment.mode == CommentMode::General {
                value["issue_url"] = json!(format!("{root}/repos/fixture/demo/issues/9"));
            } else {
                value["pull_request_url"] = json!(format!("{root}/repos/fixture/demo/pulls/9"));
                value["path"] = json!(comment.path);
                value["line"] = json!(comment.to.or(comment.from));
                value["side"] = json!(if comment.from.is_some() {
                    "LEFT"
                } else {
                    "RIGHT"
                });
                value["original_commit_id"] = json!(comment.head_sha);
            }
            if let Some(parent) = &comment.parent_id {
                value["in_reply_to_id"] = json!(parent.parse::<u64>().unwrap());
            }
            value
        }
    }
    struct Server {
        root: String,
        requests: Arc<Mutex<Vec<(String, String)>>>,
        stop: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl Server {
        fn new(
            handler: impl Fn(&str, &str, &str) -> (u16, Value, String) + Send + 'static,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let root = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let thread_requests = requests.clone();
            let thread_stop = stop.clone();
            let worker = thread::spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    let Ok((mut socket, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    };
                    socket
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut data = Vec::new();
                    let mut buffer = [0; 4096];
                    loop {
                        let count = socket.read(&mut buffer).unwrap_or(0);
                        if count == 0 {
                            break;
                        }
                        data.extend_from_slice(&buffer[..count]);
                        if data.len() > 64 * 1024 {
                            break;
                        }
                        if let Some(end) = data.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&data[..end]);
                            let length = headers
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|value| value.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if data.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    let request = String::from_utf8_lossy(&data);
                    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
                    let method = parts.next().unwrap_or("");
                    let path = parts.next().unwrap_or("");
                    thread_requests
                        .lock()
                        .unwrap()
                        .push((method.into(), path.into()));
                    let (status, value, headers) = handler(method, path, &request);
                    let body = if value.is_string()
                        && (path.ends_with("/diff")
                            || request
                                .to_ascii_lowercase()
                                .contains("application/vnd.github.v3.diff"))
                    {
                        value.as_str().unwrap().as_bytes().to_vec()
                    } else {
                        serde_json::to_vec(&value).unwrap()
                    };
                    let head = format!("HTTP/1.1 {status} fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len());
                    let _ = socket.write_all(head.as_bytes());
                    let _ = socket.write_all(&body);
                }
            });
            Self {
                root,
                requests,
                stop,
                worker: Some(worker),
            }
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(worker) = self.worker.take() {
                worker.join().unwrap();
            }
        }
    }
    #[test]
    fn preview_validates_provider_patch_file_inventory_and_never_posts() {
        for mode in [CommentMode::Inline, CommentMode::File] {
            let draft = comment(ReviewProvider::Github, mode);
            let server = Server::new(move |method, path, request| {
                assert_eq!(method, "GET");
                if path == "/user" {
                    return (200, json!({"login":"synthetic-author"}), String::new());
                }
                if path.contains("/files?") {
                    return (
                        200,
                        json!([{"filename":"src/time:stamp.rs"}]),
                        String::new(),
                    );
                }
                if request
                    .to_ascii_lowercase()
                    .contains("application/vnd.github.v3.diff")
                {
                    return (200, json!("diff --git a/src/time:stamp.rs b/src/time:stamp.rs\n--- a/src/time:stamp.rs\n+++ b/src/time:stamp.rs\n@@ -7 +7 @@\n-old\n+new\n"), String::new());
                }
                (
                    200,
                    json!({"number":9,"head":{"sha":"a".repeat(40)},"base":{"sha":"b".repeat(40)}}),
                    String::new(),
                )
            });
            let mut api = api(ReviewProvider::Github);
            api.root = server.root.clone();
            api.validate_positions(std::slice::from_ref(&draft))
                .unwrap();
            let mut invalid = draft;
            invalid.path = "src/absent.rs".into();
            assert!(api.validate_positions(&[invalid]).is_err());
            assert!(server
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|(method, _)| method == "GET"));
        }
    }
    #[test]
    fn cancellation_after_provider_reads_stops_before_post() {
        let canceled = Arc::new(AtomicBool::new(false));
        let flag = canceled.clone();
        let server = Server::new(move |method, path, _| {
            assert_eq!(method, "GET");
            if path == "/user" {
                return (200, json!({"login":"synthetic-author"}), String::new());
            }
            flag.store(true, Ordering::SeqCst);
            (
                200,
                json!({"number":9,"head":{"sha":"a".repeat(40)},"base":{"sha":"b".repeat(40)}}),
                String::new(),
            )
        });
        let mut api = api(ReviewProvider::Github);
        api.root = server.root.clone();
        let result = api.create_once_cancellable(
            &comment(ReviewProvider::Github, CommentMode::General),
            &|| canceled.load(Ordering::SeqCst),
        );
        assert!(matches!(result, CommentWrite::Rejected));
        assert_eq!(server.requests.lock().unwrap().len(), 2);
    }
    #[test]
    fn explicit_modes_have_exact_original_body_and_no_anchor_fallback() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let api = api(provider);
            for mode in [CommentMode::General, CommentMode::File, CommentMode::Inline] {
                let draft = comment(provider, mode);
                let body = api.write_body(&draft).unwrap();
                assert_eq!(
                    body.pointer(if provider == ReviewProvider::Github {
                        "/body"
                    } else {
                        "/content/raw"
                    })
                    .and_then(Value::as_str),
                    Some(draft.markdown().as_str())
                );
                if provider == ReviewProvider::Github && mode == CommentMode::Inline {
                    assert_eq!(body["side"], "LEFT");
                    assert_eq!(body["line"], 7);
                    assert_eq!(body["commit_id"], "a".repeat(40));
                }
                if provider == ReviewProvider::Github && mode == CommentMode::File {
                    assert_eq!(body["subject_type"], "file");
                    assert!(body.get("line").is_none());
                }
                if provider == ReviewProvider::Bitbucket && mode == CommentMode::Inline {
                    assert_eq!(body["inline"]["from"], 7);
                    assert!(body["inline"].get("to").is_none());
                }
                let actual = parse_comment(
                    provider,
                    &remote(&draft, &api.root, "synthetic-author", 123),
                    &format!(
                        "{}/repos/fixture/demo/{}s/9",
                        api.root,
                        if mode == CommentMode::General {
                            "issue"
                        } else {
                            "pull"
                        }
                    ),
                    mode != CommentMode::General,
                )
                .unwrap();
                assert!(api.matches(&actual, &draft, "synthetic-author"));
                assert!(!api.matches(&actual, &draft, "other-author"));
                let mut altered = actual.clone();
                altered.raw.push_str(" changed");
                assert!(!api.matches(&altered, &draft, "synthetic-author"));
                altered = actual.clone();
                altered.parent_id = Some("999".into());
                assert!(!api.matches(&altered, &draft, "synthetic-author"));
                altered = actual;
                altered.path = "another.rs".into();
                assert!(!api.matches(&altered, &draft, "synthetic-author"));
            }
        }
        let mut draft = comment(ReviewProvider::Github, CommentMode::General);
        draft.parent_id = Some("1".into());
        assert!(api(ReviewProvider::Github).write_body(&draft).is_err());
    }
    #[test]
    fn post_rate_limit_is_not_retried_and_wrong_ack_anchor_stays_unknown() {
        for wrong_anchor in [false, true] {
            let root_holder = Arc::new(Mutex::new(String::new()));
            let handler_root = root_holder.clone();
            let draft = comment(ReviewProvider::Github, CommentMode::Inline);
            let expected = draft.clone();
            let server = Server::new(move |method, path, request| {
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer synthetic-test-token"));
                if path == "/user" {
                    return (200, json!({"login":"synthetic-author"}), String::new());
                }
                if method == "GET" && path == "/repos/fixture/demo/pulls/9" {
                    return (
                        200,
                        json!({"number":9,"head":{"sha":"a".repeat(40)},"base":{"sha":"b".repeat(40)}}),
                        String::new(),
                    );
                }
                if method == "POST" && path == "/repos/fixture/demo/pulls/9/comments" {
                    if !wrong_anchor {
                        return (
                            429,
                            json!({"message":"synthetic rate limit"}),
                            "Retry-After: 0\r\n".into(),
                        );
                    }
                    let mut value = remote(
                        &expected,
                        &handler_root.lock().unwrap(),
                        "synthetic-author",
                        1,
                    );
                    value["side"] = json!("RIGHT");
                    return (201, value, String::new());
                }
                (500, json!({}), String::new())
            });
            *root_holder.lock().unwrap() = server.root.clone();
            let mut api = api(ReviewProvider::Github);
            api.root = server.root.clone();
            assert!(matches!(api.create_once(&draft), CommentWrite::Unknown));
            assert_eq!(
                server
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(method, _)| method == "POST")
                    .count(),
                1
            );
        }
    }
    #[test]
    fn redirects_and_wrong_scope_do_not_receive_credentials_or_write() {
        let server = Server::new(|_, _, _| {
            (
                302,
                json!({}),
                "Location: http://127.0.0.1:1/never\r\n".into(),
            )
        });
        let mut api = api(ReviewProvider::Github);
        api.root = server.root.clone();
        assert!(matches!(
            api.create_once(&comment(ReviewProvider::Github, CommentMode::General)),
            CommentWrite::Rejected
        ));
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        let mut scope = comment(ReviewProvider::Github, CommentMode::General).scope;
        scope.repo = "..".into();
        assert!(api.list_comments(&scope).is_err());
        assert_eq!(server.requests.lock().unwrap().len(), 1);
    }
    #[test]
    fn reconciliation_requires_complete_unique_exact_author_and_body_matches() {
        for duplicate in [false, true] {
            let root_holder = Arc::new(Mutex::new(String::new()));
            let handler_root = root_holder.clone();
            let draft = comment(ReviewProvider::Github, CommentMode::Inline);
            let expected = draft.clone();
            let server = Server::new(move |_, path, _| {
                if path == "/user" {
                    return (200, json!({"login":"synthetic-author"}), String::new());
                }
                if path.starts_with("/repos/fixture/demo/pulls/9/comments?") {
                    let value = remote(
                        &expected,
                        &handler_root.lock().unwrap(),
                        "synthetic-author",
                        1,
                    );
                    let mut unrelated = value.clone();
                    unrelated["id"] = json!(2);
                    unrelated["user"]["login"] = json!("other-author");
                    let mut values = vec![value.clone(), unrelated];
                    if duplicate {
                        let mut other = value;
                        other["id"] = json!(3);
                        values.push(other);
                    }
                    return (200, json!(values), String::new());
                }
                (200, json!([]), String::new())
            });
            *root_holder.lock().unwrap() = server.root.clone();
            let mut api = api(ReviewProvider::Github);
            api.root = server.root.clone();
            let result = api.reconcile(&draft);
            if duplicate {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().as_deref(), Some("1"));
            }
            assert!(server
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|(method, _)| method == "GET"));
        }
    }
    #[test]
    fn unsafe_or_looping_bitbucket_pagination_never_follows_another_endpoint() {
        for unsafe_next in [true, false] {
            let root_holder = Arc::new(Mutex::new(String::new()));
            let handler_root = root_holder.clone();
            let server = Server::new(move |_, path, _| {
                let next = if unsafe_next {
                    "https://attacker.invalid/comments?page=2".into()
                } else {
                    format!("{}/repositories/fixture/demo/pullrequests/9/comments?pagelen=100&fields={BB_FIELDS}", handler_root.lock().unwrap())
                };
                assert!(path.starts_with("/repositories/fixture/demo/pullrequests/9/comments?"));
                (200, json!({"values":[],"next":next}), String::new())
            });
            *root_holder.lock().unwrap() = server.root.clone();
            let mut api = api(ReviewProvider::Bitbucket);
            api.root = server.root.clone();
            // Private owned HTTP root models the fixed HTTPS production root.
            // A repeated next URL fails the loop fence before another request.
            assert!(api
                .list_comments(&comment(ReviewProvider::Bitbucket, CommentMode::General).scope)
                .is_err());
            assert_eq!(server.requests.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn github_reply_preserves_root_and_never_falls_back_on_stale_parent() {
        for stale in [false, true] {
            let root_holder = Arc::new(Mutex::new(String::new()));
            let handler_root = root_holder.clone();
            let mut draft = comment(ReviewProvider::Github, CommentMode::Inline);
            draft.parent_id = Some("99".into());
            let expected = draft.clone();
            let server = Server::new(move |method, path, request| {
                let root = handler_root.lock().unwrap();
                if path == "/user" {
                    return (200, json!({"login":"synthetic-author"}), String::new());
                }
                if method == "GET" && path == "/repos/fixture/demo/pulls/9" {
                    return (
                        200,
                        json!({"number":9,"head":{"sha":"a".repeat(40)},"base":{"sha":"b".repeat(40)}}),
                        String::new(),
                    );
                }
                if path.starts_with("/repos/fixture/demo/pulls/9/comments?") {
                    let mut original = expected.clone();
                    original.parent_id = None;
                    let mut value = remote(&original, &root, "other-author", 99);
                    if stale {
                        value["original_commit_id"] = json!("c".repeat(40));
                    }
                    return (200, json!([value]), String::new());
                }
                if path.starts_with("/repos/fixture/demo/issues/9/comments?") {
                    return (200, json!([]), String::new());
                }
                if method == "POST" && path == "/repos/fixture/demo/pulls/9/comments/99/replies" {
                    let body: Value =
                        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                    assert_eq!(body.as_object().unwrap().len(), 1);
                    assert_eq!(body["body"], expected.markdown());
                    return (
                        201,
                        remote(&expected, &root, "synthetic-author", 123),
                        String::new(),
                    );
                }
                (500, json!({}), String::new())
            });
            *root_holder.lock().unwrap() = server.root.clone();
            let mut api = api(ReviewProvider::Github);
            api.root = server.root.clone();
            let outcome = api.create_once(&draft);
            if stale {
                assert!(matches!(outcome, CommentWrite::Rejected));
            } else {
                assert!(matches!(outcome, CommentWrite::Created(id) if id == "123"));
            }
            assert_eq!(
                server
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(method, _)| method == "POST")
                    .count(),
                usize::from(!stale)
            );
        }
    }
    #[test]
    fn bitbucket_general_file_and_old_line_use_verified_single_post() {
        for mode in [CommentMode::General, CommentMode::File, CommentMode::Inline] {
            let draft = comment(ReviewProvider::Bitbucket, mode);
            let expected = draft.clone();
            let server = Server::new(move |method, path, request| {
                if path == "/user" {
                    return (
                        200,
                        json!({"account_id":"synthetic-account"}),
                        String::new(),
                    );
                }
                if method == "GET" && path == "/repositories/fixture/demo/pullrequests/9" {
                    return (
                        200,
                        json!({"id":9,"source":{"commit":{"hash":"a".repeat(40)}},"destination":{"commit":{"hash":"b".repeat(40)}}}),
                        String::new(),
                    );
                }
                if method == "POST" && path == "/repositories/fixture/demo/pullrequests/9/comments"
                {
                    assert!(request
                        .to_ascii_lowercase()
                        .contains("accept: application/json"));
                    let body: Value =
                        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                    assert_eq!(body["content"]["raw"], expected.markdown());
                    return (
                        201,
                        remote(&expected, "unused", "synthetic-account", 123),
                        String::new(),
                    );
                }
                (500, json!({}), String::new())
            });
            let mut api = api(ReviewProvider::Bitbucket);
            api.root = server.root.clone();
            assert!(matches!(api.create_once(&draft), CommentWrite::Created(id) if id == "123"));
            assert_eq!(
                server
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(method, _)| method == "POST")
                    .count(),
                1
            );
        }
    }
    #[test]
    fn an_early_match_cannot_acknowledge_an_incomplete_pagination_scan() {
        let root_holder = Arc::new(Mutex::new(String::new()));
        let handler_root = root_holder.clone();
        let draft = comment(ReviewProvider::Github, CommentMode::Inline);
        let expected = draft.clone();
        let server = Server::new(move |_, path, _| {
            if path == "/user" {
                return (200, json!({"login":"synthetic-author"}), String::new());
            }
            if path.ends_with("page=1") {
                return (
                    200,
                    json!([remote(
                        &expected,
                        &handler_root.lock().unwrap(),
                        "synthetic-author",
                        1
                    )]),
                    "Link: <https://attacker.invalid/never>; rel=\"next\"\r\n".into(),
                );
            }
            (500, json!({}), String::new())
        });
        *root_holder.lock().unwrap() = server.root.clone();
        let mut api = api(ReviewProvider::Github);
        api.root = server.root.clone();
        assert!(api.reconcile(&draft).is_err());
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|(method, _)| method == "GET"));
        assert_eq!(
            requests.last().unwrap().1,
            "/repos/fixture/demo/pulls/9/comments?per_page=100&page=2"
        );
    }
    #[test]
    fn large_acknowledgment_and_mismatched_scope_cannot_be_confirmed() {
        let draft = comment(ReviewProvider::Github, CommentMode::General);
        let server = Server::new(move |method, path, _| {
            if path == "/user" {
                return (200, json!({"login":"synthetic-author"}), String::new());
            }
            if method == "GET" && path == "/repos/fixture/demo/pulls/9" {
                return (
                    200,
                    json!({"number":9,"head":{"sha":"a".repeat(40)},"base":{"sha":"b".repeat(40)}}),
                    String::new(),
                );
            }
            (
                201,
                json!({"oversized":"x".repeat(PAGE_BYTES + 1)}),
                String::new(),
            )
        });
        let mut api = api(ReviewProvider::Github);
        api.root = server.root.clone();
        assert!(matches!(api.create_once(&draft), CommentWrite::Unknown));
        assert_eq!(
            server
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|(method, _)| method == "POST")
                .count(),
            1
        );
        let wrong = remote(&draft, "https://different.invalid", "synthetic-author", 1);
        assert!(parse_comment(
            ReviewProvider::Github,
            &wrong,
            "https://api.github.com/repos/fixture/demo/issues/9",
            false
        )
        .is_err());
        let mut multi = remote(
            &comment(ReviewProvider::Github, CommentMode::Inline),
            "https://api.github.com",
            "synthetic-author",
            1,
        );
        multi["original_start_line"] = json!(3);
        let actual = parse_comment(
            ReviewProvider::Github,
            &multi,
            "https://api.github.com/repos/fixture/demo/pulls/9",
            true,
        )
        .unwrap();
        assert!(!api.matches(
            &actual,
            &comment(ReviewProvider::Github, CommentMode::Inline),
            "synthetic-author"
        ));
    }
}
