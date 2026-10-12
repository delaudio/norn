//! Native human approval adapter; a POST acknowledgment is never inferred from a read.
use super::*;
use crate::services::provider_approval::{Actor, Proof};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WriteOutcome {
    Approved(Option<String>),
    Rejected,
    Uncertain,
}
impl Api {
    fn approval_actor(&self, started: Instant) -> Result<Actor, String> {
        let (user, _) = self.get(&format!("{}/user", self.root), started)?;
        let (account_id, display_name) = match self.repository.provider {
            ReviewProvider::Github => {
                let id = user
                    .get("id")
                    .and_then(Value::as_u64)
                    .filter(|id| *id > 0)
                    .ok_or_else(error)?;
                let login = user
                    .get("login")
                    .and_then(Value::as_str)
                    .filter(|login| !login.is_empty())
                    .ok_or_else(error)?;
                (
                    id.to_string(),
                    user.get("name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty())
                        .unwrap_or(login)
                        .to_string(),
                )
            }
            ReviewProvider::Bitbucket => (
                user.get("account_id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(error)?
                    .to_string(),
                user.get("display_name")
                    .and_then(Value::as_str)
                    .ok_or_else(error)?
                    .to_string(),
            ),
        };
        safe_text(&account_id, 256, false)?;
        safe_text(&display_name, 256, false)?;
        Ok(Actor {
            account_id,
            display_name,
        })
    }
    fn approval_repository_id(&self, value: &Value) -> Result<String, String> {
        let native = value
            .pointer(if self.repository.provider == ReviewProvider::Github {
                "/base/repo"
            } else {
                "/destination/repository"
            })
            .ok_or_else(error)?;
        let (workspace, repo) = native
            .get("full_name")
            .and_then(Value::as_str)
            .and_then(|name| name.split_once('/'))
            .ok_or_else(error)?;
        let resolved = RepositoryIdentity {
            provider: self.repository.provider,
            workspace: workspace.into(),
            repo: repo.into(),
        };
        // Reuse canonical alias/path checks while proving immutable native IDs.
        browser_url(
            &DetailParams {
                repository: self.repository.clone(),
                pr_id: 1,
            },
            &resolved,
        )?;
        match self.repository.provider {
            ReviewProvider::Github => Ok(native
                .get("id")
                .and_then(Value::as_u64)
                .filter(|id| *id > 0)
                .ok_or_else(error)?
                .to_string()),
            ReviewProvider::Bitbucket => {
                let uuid = native
                    .get("uuid")
                    .and_then(Value::as_str)
                    .filter(|id| braced_uuid(id))
                    .ok_or_else(error)?;
                if braced_uuid(&self.repository.repo)
                    && !uuid.eq_ignore_ascii_case(&self.repository.repo)
                {
                    return Err(error());
                }
                if braced_uuid(&self.repository.workspace)
                    && native
                        .pointer("/workspace/uuid")
                        .and_then(Value::as_str)
                        .is_none_or(|id| !id.eq_ignore_ascii_case(&self.repository.workspace))
                {
                    return Err(error());
                }
                Ok(uuid.to_ascii_lowercase())
            }
        }
    }
    fn approved_reviewer(
        &self,
        params: &DetailParams,
        actor: &Actor,
        head: &str,
        metadata: &Value,
        started: Instant,
        historical: bool,
    ) -> Result<bool, String> {
        if self.repository.provider == ReviewProvider::Bitbucket {
            let participants = metadata
                .get("participants")
                .and_then(Value::as_array)
                .ok_or_else(error)?;
            if participants.len() > 100 {
                return Err(error());
            }
            let mut found = None;
            for participant in participants {
                let id = participant
                    .pointer("/user/account_id")
                    .and_then(Value::as_str)
                    .ok_or_else(error)?;
                safe_text(id, 256, false)?;
                if id == actor.account_id {
                    if found.is_some() {
                        return Err(error());
                    }
                    found = Some(
                        participant
                            .get("approved")
                            .and_then(Value::as_bool)
                            .ok_or_else(error)?,
                    );
                }
            }
            return Ok(found.unwrap_or(false));
        }
        let collection = format!("{}/{}/reviews", self.collection(), params.pr_id);
        let mut page = 1;
        let mut last_id = 0;
        let mut latest = None;
        loop {
            let (values, link) =
                self.get(&format!("{collection}?per_page=100&page={page}"), started)?;
            let reviews = values
                .as_array()
                .filter(|values| values.len() <= 100)
                .ok_or_else(error)?;
            for review in reviews {
                let id = review
                    .get("id")
                    .and_then(Value::as_u64)
                    .filter(|id| *id > last_id)
                    .ok_or_else(error)?;
                last_id = id;
                let principal = review
                    .pointer("/user/id")
                    .and_then(Value::as_u64)
                    .filter(|id| *id > 0)
                    .ok_or_else(error)?
                    .to_string();
                let state = review
                    .get("state")
                    .and_then(Value::as_str)
                    .ok_or_else(error)?;
                if !matches!(
                    state,
                    "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED" | "DISMISSED" | "PENDING"
                ) {
                    return Err(error());
                }
                if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED") {
                    let commit = crate::services::provider_snapshot::full_sha(
                        review
                            .get("commit_id")
                            .and_then(Value::as_str)
                            .ok_or_else(error)?,
                    )?;
                    if principal == actor.account_id && (!historical || commit == head) {
                        // Current status follows the latest decisive actor review,
                        // including changes requested on another revision. Historical
                        // recovery can prove only the explicitly stored old head.
                        latest = Some(state == "APPROVED" && commit == head);
                    }
                }
            }
            let next = github_next(link.as_deref())?;
            let next = self.next_page(next.as_deref(), &collection, page, 100)?;
            if next.is_some() && page >= MAX_REVIEW_PAGES {
                return Err("Approval history exceeds the complete-read limit.".into());
            }
            if let Some(next) = next {
                page = next
            } else {
                break;
            }
        }
        Ok(latest.unwrap_or(false))
    }
    pub(crate) fn approval_proof(
        &self,
        params: &DetailParams,
        started: Instant,
    ) -> Result<Proof, String> {
        params.validate()?;
        if params.repository != self.repository {
            return Err(error());
        }
        let url = format!("{}/{}", self.collection(), params.pr_id);
        let actor = self.approval_actor(started)?;
        let (before, _) = self.get(&url, started)?;
        let (detail, _) = parse_metadata(self.repository.provider, &before, params.pr_id)?;
        let head = crate::services::provider_snapshot::full_sha(
            detail.source_commit_hash.as_deref().ok_or_else(error)?,
        )?;
        let destination = crate::services::provider_snapshot::full_sha(
            detail
                .destination_commit_hash
                .as_deref()
                .ok_or_else(error)?,
        )?;
        let approved = self.approved_reviewer(params, &actor, &head, &before, started, false)?;
        let proof = Proof {
            request: params.clone(),
            repository_id: self.approval_repository_id(&before)?,
            actor,
            title: detail.title,
            head_sha: head,
            destination_sha: destination,
            source_branch: detail.source_branch,
            destination_branch: detail.destination_branch,
            source_repository: super::snapshots::snapshot_source_repository(
                &before,
                self.repository.provider,
            )?,
            state: detail.state,
            draft: detail.draft,
            already_approved: approved,
        };
        proof.validate()?;
        let (after, _) = self.get(&url, started)?;
        // Metadata equality includes fork, branches, pins, draft/state and title.
        let (after_detail, _) = parse_metadata(self.repository.provider, &after, params.pr_id)?;
        if self.approval_repository_id(&after)? != proof.repository_id
            || after_detail.title != proof.title
            || after_detail.source_branch != proof.source_branch
            || after_detail.destination_branch != proof.destination_branch
            || after_detail.source_commit_hash.as_deref() != Some(proof.head_sha.as_str())
            || after_detail.destination_commit_hash.as_deref()
                != Some(proof.destination_sha.as_str())
            || after_detail.state != proof.state
            || after_detail.draft != proof.draft
            || super::snapshots::snapshot_source_repository(&after, self.repository.provider)?
                != proof.source_repository
        {
            return Err("The PR changed while reading its approval preview. Refresh explicitly before approving.".into());
        }
        Ok(proof)
    }
    pub(crate) fn approval_observed(
        &self,
        expected: &Proof,
        current: &Proof,
        started: Instant,
    ) -> Result<bool, String> {
        if expected.request != current.request
            || expected.repository_id != current.repository_id
            || expected.actor.account_id != current.actor.account_id
        {
            return Err(error());
        }
        if self.repository.provider == ReviewProvider::Bitbucket {
            return Ok(expected.same_preview(current) && current.already_approved);
        }
        if expected.head_sha == current.head_sha {
            return Ok(current.already_approved);
        }
        self.approved_reviewer(
            &expected.request,
            &expected.actor,
            &expected.head_sha,
            &Value::Null,
            started,
            true,
        )
    }
    pub(crate) fn post_approval(&self, proof: &Proof, started: Instant) -> WriteOutcome {
        if proof.validate().is_err() || proof.request.repository != self.repository {
            return WriteOutcome::Rejected;
        }
        let Some(remaining) = TOTAL_TIME.checked_sub(started.elapsed()) else {
            return WriteOutcome::Rejected;
        };
        let url = format!(
            "{}/{}/{}",
            self.collection(),
            proof.request.pr_id,
            if self.repository.provider == ReviewProvider::Github {
                "reviews"
            } else {
                "approve"
            }
        );
        let request = self
            .http
            .post(&url)
            .timeout(remaining.min(Duration::from_secs(10)))
            .header("Accept", "application/json");
        let request = match &self.auth {
            Auth::Github(token) => request
                .bearer_auth(token)
                .json(&json!({"event":"APPROVE","commit_id":proof.head_sha})),
            Auth::Bitbucket(creds) => request.basic_auth(&creds.username, Some(&creds.token)),
        };
        let Ok(mut response) = request.send() else {
            return WriteOutcome::Uncertain;
        };
        if matches!(
            response.status().as_u16(),
            400 | 401 | 403 | 404 | 409 | 422 | 429
        ) {
            return WriteOutcome::Rejected;
        }
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|bytes| bytes > BODY_BYTES as u64)
        {
            return WriteOutcome::Uncertain;
        }
        let mut bytes = Vec::new();
        if response
            .by_ref()
            .take((BODY_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > BODY_BYTES
            || started.elapsed() >= TOTAL_TIME
        {
            return WriteOutcome::Uncertain;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return WriteOutcome::Uncertain;
        };
        let remote = match self.repository.provider {
            ReviewProvider::Github => {
                if value.get("state").and_then(Value::as_str) != Some("APPROVED")
                    || value.get("commit_id").and_then(Value::as_str)
                        != Some(proof.head_sha.as_str())
                    || value
                        .pointer("/user/id")
                        .and_then(Value::as_u64)
                        .map(|id| id.to_string())
                        .as_deref()
                        != Some(proof.actor.account_id.as_str())
                {
                    return WriteOutcome::Uncertain;
                }
                let Some(id) = value.get("id").and_then(Value::as_u64).filter(|id| *id > 0) else {
                    return WriteOutcome::Uncertain;
                };
                Some(id.to_string())
            }
            ReviewProvider::Bitbucket => {
                if value.get("approved").and_then(Value::as_bool) != Some(true)
                    || value.pointer("/user/account_id").and_then(Value::as_str)
                        != Some(proof.actor.account_id.as_str())
                {
                    return WriteOutcome::Uncertain;
                }
                None
            }
        };
        // Bitbucket's approval API has no commit CAS field. An acknowledged
        // participant is not a proof for different PR revisions after the POST.
        if self.repository.provider == ReviewProvider::Bitbucket {
            let Ok(current) = self.approval_proof(&proof.request, started) else {
                return WriteOutcome::Uncertain;
            };
            if !proof.same_preview(&current) || !current.already_approved {
                return WriteOutcome::Uncertain;
            }
        }
        WriteOutcome::Approved(remote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Mutex,
        },
        thread,
    };
    const REPO_UUID: &str = "{11111111-1111-4111-8111-111111111111}";
    fn repository(provider: ReviewProvider) -> RepositoryIdentity {
        RepositoryIdentity {
            provider,
            workspace: "Owned".into(),
            repo: "Review".into(),
        }
    }
    fn metadata(provider: ReviewProvider, approved: bool) -> Value {
        if provider == ReviewProvider::Github {
            json!({"number":7,"title":"Owned change","body":"Owned description","state":"open","draft":false,"user":{"login":"owned-author"},"head":{"ref":"feature","sha":"a".repeat(40),"repo":{"full_name":"Fork/Review"}},"base":{"ref":"main","sha":"b".repeat(40),"repo":{"id":99,"full_name":"Owned/Review"}},"requested_reviewers":[]})
        } else {
            json!({"id":7,"title":"Owned change","state":"OPEN","draft":false,"author":{"account_id":"owned-author","display_name":"Owned author"},"source":{"branch":{"name":"feature"},"commit":{"hash":"a".repeat(40)},"repository":{"full_name":"Fork/Review"}},"destination":{"branch":{"name":"main"},"commit":{"hash":"b".repeat(40)},"repository":{"uuid":REPO_UUID,"full_name":"Owned/Review"}},"participants":[{"user":{"account_id":"stable-actor","display_name":"Owned reviewer"},"role":"REVIEWER","approved":approved}]})
        }
    }
    type RecordedRequest = (String, String, Vec<u8>);
    struct Server {
        root: String,
        stopped: Arc<AtomicBool>,
        join: Option<thread::JoinHandle<()>>,
        requests: Arc<Mutex<Vec<RecordedRequest>>>,
        posts: Arc<AtomicUsize>,
        metadata: Arc<Mutex<Value>>,
        reviews: Arc<Mutex<Option<Value>>>,
    }
    impl Server {
        fn new(provider: ReviewProvider, fault: &'static str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let root = format!("http://{}", listener.local_addr().unwrap());
            let stopped = Arc::new(AtomicBool::new(false));
            let stop = stopped.clone();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let seen = requests.clone();
            let posts = Arc::new(AtomicUsize::new(0));
            let writes = posts.clone();
            let metadata = Arc::new(Mutex::new(metadata(provider, false)));
            let current = metadata.clone();
            let reviews = Arc::new(Mutex::new(None));
            let history = reviews.clone();
            let join = thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    let mut chunk = [0u8; 2048];
                    let header_end = loop {
                        assert!(bytes.len() < 16384);
                        let n = stream.read(&mut chunk).unwrap();
                        if n == 0 {
                            break None;
                        };
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                            break Some(end + 4);
                        }
                    };
                    let Some(end) = header_end else { continue };
                    let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                                .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    assert!(length <= 65536);
                    while bytes.len() < end + length {
                        let n = stream.read(&mut chunk).unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    let first = headers.lines().next().unwrap();
                    let mut parts = first.split_whitespace();
                    let method = parts.next().unwrap();
                    let path = parts.next().unwrap();
                    let body = bytes[end..end + length].to_vec();
                    assert!(headers.to_ascii_lowercase().contains("authorization:"));
                    seen.lock()
                        .unwrap()
                        .push((method.into(), path.into(), body.clone()));
                    let (status, extra, response) = if method == "POST" {
                        writes.fetch_add(1, Ordering::SeqCst);
                        if provider == ReviewProvider::Github {
                            assert!(path.ends_with("/pulls/7/reviews"));
                            assert_eq!(
                                serde_json::from_slice::<Value>(&body).unwrap(),
                                json!({"event":"APPROVE","commit_id":"a".repeat(40)})
                            );
                        } else {
                            assert!(path.ends_with("/pullrequests/7/approve"));
                            assert!(body.is_empty());
                            current.lock().unwrap()["participants"][0]["approved"] = json!(true);
                        }
                        if fault == "drop" {
                            continue;
                        }
                        if fault == "slow" {
                            thread::sleep(Duration::from_millis(150));
                        }
                        let mut ack = if provider == ReviewProvider::Github {
                            json!({"id":55,"user":{"id":42},"state":"APPROVED","commit_id":"a".repeat(40)})
                        } else {
                            json!({"user":{"account_id":"stable-actor"},"approved":true})
                        };
                        if fault == "actor" {
                            if provider == ReviewProvider::Github {
                                ack["user"]["id"] = json!(43)
                            } else {
                                ack["user"]["account_id"] = json!("foreign")
                            }
                        }
                        if fault == "pin" {
                            ack["commit_id"] = json!("c".repeat(40));
                            if provider == ReviewProvider::Bitbucket {
                                current.lock().unwrap()["source"]["commit"]["hash"] =
                                    json!("c".repeat(40));
                            }
                        }
                        match fault {
                            "malformed" => (200, String::new(), "invalid-json".into()),
                            "oversize" => (200, String::new(), "x".repeat(BODY_BYTES + 1)),
                            "redirect" => (
                                302,
                                "Location: https://example.invalid/unsafe\r\n".into(),
                                String::new(),
                            ),
                            "rejected" => (403, String::new(), String::new()),
                            "rate" => (429, String::new(), String::new()),
                            "server" => (500, String::new(), String::new()),
                            _ => (200, String::new(), ack.to_string()),
                        }
                    } else {
                        assert_eq!(method, "GET");
                        let value = if path == "/user" {
                            if provider == ReviewProvider::Github {
                                json!({"id":42,"login":"owned-login","name":"Owned reviewer"})
                            } else {
                                json!({"account_id":"stable-actor","display_name":"Owned reviewer"})
                            }
                        } else if path.contains("/reviews?") {
                            if let Some(value) = history.lock().unwrap().clone() {
                                value
                            } else if writes.load(Ordering::SeqCst) > 0 {
                                json!([{"id":55,"user":{"id":42},"state":"APPROVED","commit_id":"a".repeat(40)}])
                            } else {
                                json!([])
                            }
                        } else {
                            current.lock().unwrap().clone()
                        };
                        (200, String::new(), value.to_string())
                    };
                    let response_header=format!("HTTP/1.1 {status} Fixture\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n",response.len());
                    let _ = stream.write_all(response_header.as_bytes());
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            Self {
                root,
                stopped,
                join: Some(join),
                requests,
                posts,
                metadata,
                reviews,
            }
        }
        fn api(&self, provider: ReviewProvider) -> Api {
            let auth = if provider == ReviewProvider::Github {
                Auth::Github("owned-token".into())
            } else {
                Auth::Bitbucket(Credentials {
                    username: "owned-user".into(),
                    token: "owned-token".into(),
                })
            };
            let mut api = Api::with_auth(repository(provider), auth).unwrap();
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
    #[test]
    fn both_native_adapters_pin_actor_repo_and_exact_approval_request() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(provider, "");
            let api = server.api(provider);
            let request = DetailParams {
                repository: repository(provider),
                pr_id: 7,
            };
            let proof = api.approval_proof(&request, Instant::now()).unwrap();
            assert_eq!(
                proof.actor.account_id,
                if provider == ReviewProvider::Github {
                    "42"
                } else {
                    "stable-actor"
                }
            );
            assert_eq!(
                proof.repository_id,
                if provider == ReviewProvider::Github {
                    "99"
                } else {
                    REPO_UUID
                }
            );
            assert!(!proof.already_approved);
            assert!(matches!(
                api.post_approval(&proof, Instant::now()),
                WriteOutcome::Approved(_)
            ));
            assert_eq!(server.posts.load(Ordering::SeqCst), 1);
            assert!(
                api.approval_proof(&request, Instant::now())
                    .unwrap()
                    .already_approved
            );
            assert!(server
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|(method, _, _)| matches!(method.as_str(), "GET" | "POST")));
        }
    }
    #[test]
    fn accepted_write_dropped_malformed_foreign_redirect_and_oversize_ack_stay_uncertain_without_retry(
    ) {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            for fault in [
                "drop",
                "malformed",
                "actor",
                "pin",
                "redirect",
                "oversize",
                "server",
            ] {
                let server = Server::new(provider, fault);
                let api = server.api(provider);
                let request = DetailParams {
                    repository: repository(provider),
                    pr_id: 7,
                };
                let proof = api.approval_proof(&request, Instant::now()).unwrap();
                assert_eq!(
                    api.post_approval(&proof, Instant::now()),
                    WriteOutcome::Uncertain,
                    "{provider:?} {fault}"
                );
                assert_eq!(server.posts.load(Ordering::SeqCst), 1);
            }
        }
    }
    #[test]
    fn definitive_rejection_total_budget_and_slow_ack_are_distinguished() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            for fault in ["rejected", "rate", "slow"] {
                let server = Server::new(provider, fault);
                let api = server.api(provider);
                let proof = api
                    .approval_proof(
                        &DetailParams {
                            repository: repository(provider),
                            pr_id: 7,
                        },
                        Instant::now(),
                    )
                    .unwrap();
                assert_eq!(
                    api.post_approval(&proof, Instant::now() - Duration::from_secs(21)),
                    WriteOutcome::Rejected
                );
                assert_eq!(server.posts.load(Ordering::SeqCst), 0);
                let started = if fault == "slow" {
                    Instant::now() - Duration::from_millis(19950)
                } else {
                    Instant::now()
                };
                let expected = if fault == "slow" {
                    WriteOutcome::Uncertain
                } else {
                    WriteOutcome::Rejected
                };
                assert_eq!(api.post_approval(&proof, started), expected);
                assert_eq!(server.posts.load(Ordering::SeqCst), 1);
            }
        }
    }
    #[test]
    fn latest_actor_review_controls_current_approval_and_malformed_decisive_commits_fail() {
        let server = Server::new(ReviewProvider::Github, "");
        let api = server.api(ReviewProvider::Github);
        let request = DetailParams {
            repository: repository(ReviewProvider::Github),
            pr_id: 7,
        };
        let approved =
            json!({"id":1,"user":{"id":42},"state":"APPROVED","commit_id":"a".repeat(40)});
        let changes =
            json!({"id":2,"user":{"id":42},"state":"CHANGES_REQUESTED","commit_id":"c".repeat(40)});
        *server.reviews.lock().unwrap() = Some(json!([approved, changes]));
        assert!(
            !api.approval_proof(&request, Instant::now())
                .unwrap()
                .already_approved
        );
        *server.reviews.lock().unwrap() = Some(
            json!([{"id":1,"user":{"id":42},"state":"APPROVED","commit_id":"a".repeat(40)},{"id":2,"user":{"id":42},"state":"DISMISSED","commit_id":null}]),
        );
        assert!(api.approval_proof(&request, Instant::now()).is_err());
        assert_eq!(server.posts.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn historical_github_receipt_is_read_only_and_bitbucket_changed_pins_remain_unproved() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(provider, "");
            let api = server.api(provider);
            let request = DetailParams {
                repository: repository(provider),
                pr_id: 7,
            };
            let proof = api.approval_proof(&request, Instant::now()).unwrap();
            assert!(matches!(
                api.post_approval(&proof, Instant::now()),
                WriteOutcome::Approved(_)
            ));
            if provider == ReviewProvider::Github {
                server.metadata.lock().unwrap()["head"]["sha"] = json!("c".repeat(40));
            } else {
                server.metadata.lock().unwrap()["source"]["commit"]["hash"] = json!("c".repeat(40));
            }
            let current = api.approval_proof(&request, Instant::now()).unwrap();
            assert_ne!(proof.head_sha, current.head_sha);
            assert_eq!(
                api.approval_observed(&proof, &current, Instant::now())
                    .unwrap(),
                provider == ReviewProvider::Github
            );
            assert_eq!(server.posts.load(Ordering::SeqCst), 1);
        }
    }
    #[test]
    fn native_metadata_identity_and_bitbucket_historical_pin_drift_are_not_inferred() {
        for provider in [ReviewProvider::Github, ReviewProvider::Bitbucket] {
            let server = Server::new(provider, "");
            let api = server.api(provider);
            let request = DetailParams {
                repository: repository(provider),
                pr_id: 7,
            };
            let proof = api.approval_proof(&request, Instant::now()).unwrap();
            if provider == ReviewProvider::Github {
                server.metadata.lock().unwrap()["base"]["repo"]["full_name"] =
                    json!("Other/Review");
            } else {
                server.metadata.lock().unwrap()["destination"]["repository"]["full_name"] =
                    json!("Other/Review");
            }
            assert!(api.approval_proof(&request, Instant::now()).is_err());
            assert_eq!(server.posts.load(Ordering::SeqCst), 0);
            let mut current = proof.clone();
            current.head_sha = "c".repeat(40);
            current.already_approved = true;
            if provider == ReviewProvider::Bitbucket {
                assert!(!api
                    .approval_observed(&proof, &current, Instant::now())
                    .unwrap());
            }
        }
    }
}
