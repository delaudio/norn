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
                if principal == actor.account_id
                    && review.get("commit_id").and_then(Value::as_str) == Some(head)
                    && matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED")
                {
                    latest = Some(state == "APPROVED");
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
        let approved = self.approved_reviewer(params, &actor, &head, &before, started)?;
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
