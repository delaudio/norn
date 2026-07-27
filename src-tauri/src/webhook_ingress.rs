//! Authenticated HTTP boundary for provider pull-request webhooks.

#[cfg(test)]
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::Mutex;

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::review_event::{
    PullRequestClosedOutcome, PullRequestEventActor, PullRequestReviewEvent,
    PullRequestReviewEventKind, PullRequestReviewEventProvider,
    PullRequestReviewEventSchemaVersion, PullRequestRevision,
};

pub const MAX_WEBHOOK_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_WEBHOOK_HEADER_COUNT: usize = 64;
pub const MAX_WEBHOOK_HEADER_NAME_BYTES: usize = 128;
pub const MAX_WEBHOOK_HEADER_VALUE_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy)]
pub struct WebhookHttpRequest<'a> {
    pub method: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookIngressRejection {
    MethodNotAllowed,
    UnsupportedMediaType,
    PayloadTooLarge,
    HeadersTooLarge,
    MissingSignature,
    InvalidSignature,
    InvalidHeaders,
    InvalidPayload,
    Unavailable,
}

impl WebhookIngressRejection {
    pub const fn status_code(self) -> u16 {
        match self {
            Self::MethodNotAllowed => 405,
            Self::UnsupportedMediaType => 415,
            Self::PayloadTooLarge => 413,
            Self::HeadersTooLarge => 431,
            Self::MissingSignature | Self::InvalidSignature => 401,
            Self::InvalidHeaders | Self::InvalidPayload => 400,
            Self::Unavailable => 503,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::MethodNotAllowed => "method_not_allowed",
            Self::UnsupportedMediaType => "unsupported_media_type",
            Self::PayloadTooLarge => "payload_too_large",
            Self::HeadersTooLarge => "request_headers_too_large",
            Self::MissingSignature => "missing_signature",
            Self::InvalidSignature => "invalid_signature",
            Self::InvalidHeaders => "invalid_headers",
            Self::InvalidPayload => "invalid_payload",
            Self::Unavailable => "ingress_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookIngressOutcome {
    Accepted(Box<PullRequestReviewEvent>),
    Ignored,
    Duplicate,
    Rejected(WebhookIngressRejection),
}

impl WebhookIngressOutcome {
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::Accepted(_) => 202,
            Self::Ignored | Self::Duplicate => 200,
            Self::Rejected(rejection) => rejection.status_code(),
        }
    }

    pub const fn code(&self) -> &'static str {
        match self {
            Self::Accepted(_) => "accepted",
            Self::Ignored => "ignored",
            Self::Duplicate => "duplicate",
            Self::Rejected(rejection) => rejection.code(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookEventAcceptance {
    Accepted,
    DuplicateDelivery,
    UnchangedReviewState,
}

pub trait WebhookEventSink {
    /// Atomically records a delivery receipt and durably accepts changed review state.
    ///
    /// Review state consists of base, head, draft status, and terminal outcome for
    /// one tenant/provider/repository/pull request. A new delivery with unchanged
    /// review state is recorded without creating review work. An error must leave
    /// both the delivery receipt and review state unchanged so provider retry is safe.
    fn accept(&self, event: &PullRequestReviewEvent) -> Result<WebhookEventAcceptance, String>;
}

pub trait WebhookCommitResolver {
    /// Resolves a provider commit id to the full SHA required by the public event contract.
    fn resolve(
        &self,
        provider: PullRequestReviewEventProvider,
        tenant_id: &str,
        workspace: &str,
        repository: &str,
        commit_id: &str,
    ) -> Result<String, String>;
}

#[cfg(test)]
#[derive(Debug, Default)]
struct InMemoryWebhookEventSink {
    state: Mutex<InMemoryWebhookEventSinkState>,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct InMemoryWebhookEventSinkState {
    deliveries: HashSet<(String, PullRequestReviewEventProvider, String)>,
    pull_requests:
        HashMap<(String, PullRequestReviewEventProvider, String, String, u64), InMemoryReviewState>,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct InMemoryReviewState {
    base: PullRequestRevision,
    head: PullRequestRevision,
    draft: bool,
    closed_outcome: Option<PullRequestClosedOutcome>,
}

#[cfg(test)]
impl WebhookEventSink for InMemoryWebhookEventSink {
    fn accept(&self, event: &PullRequestReviewEvent) -> Result<WebhookEventAcceptance, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "webhook event sink lock is poisoned".to_string())?;
        let delivery_key = (
            event.tenant_id.clone(),
            event.provider,
            event.delivery_id.clone(),
        );
        if state.deliveries.contains(&delivery_key) {
            return Ok(WebhookEventAcceptance::DuplicateDelivery);
        }

        let pull_request_key = (
            event.tenant_id.clone(),
            event.provider,
            event.workspace.clone(),
            event.repository.clone(),
            event.pull_request_id,
        );
        let review_state = InMemoryReviewState {
            base: event.base.clone(),
            head: event.head.clone(),
            draft: event.draft,
            closed_outcome: event.closed_outcome,
        };
        state.deliveries.insert(delivery_key);
        if state.pull_requests.get(&pull_request_key) == Some(&review_state) {
            return Ok(WebhookEventAcceptance::UnchangedReviewState);
        }
        state.pull_requests.insert(pull_request_key, review_state);
        Ok(WebhookEventAcceptance::Accepted)
    }
}

#[derive(Debug)]
pub struct WebhookIngress<S, C> {
    sink: S,
    commit_resolver: C,
}

impl<S, C> WebhookIngress<S, C>
where
    S: WebhookEventSink,
    C: WebhookCommitResolver,
{
    pub const fn new(sink: S, commit_resolver: C) -> Self {
        Self {
            sink,
            commit_resolver,
        }
    }

    pub fn handle(
        &self,
        provider: PullRequestReviewEventProvider,
        tenant_id: &str,
        secret: &[u8],
        request: WebhookHttpRequest<'_>,
    ) -> WebhookIngressOutcome {
        if request.method != "POST" {
            return rejected(WebhookIngressRejection::MethodNotAllowed);
        }
        if request.headers.len() > MAX_WEBHOOK_HEADER_COUNT
            || request.headers.iter().any(|(name, value)| {
                name.len() > MAX_WEBHOOK_HEADER_NAME_BYTES
                    || value.len() > MAX_WEBHOOK_HEADER_VALUE_BYTES
            })
        {
            return rejected(WebhookIngressRejection::HeadersTooLarge);
        }
        let content_type = match unique_header(request.headers, "content-type") {
            Ok(Some(value)) => value,
            Ok(None) => return rejected(WebhookIngressRejection::UnsupportedMediaType),
            Err(()) => return rejected(WebhookIngressRejection::InvalidHeaders),
        };
        if !is_json_content_type(content_type) {
            return rejected(WebhookIngressRejection::UnsupportedMediaType);
        }
        if request.body.len() > MAX_WEBHOOK_BODY_BYTES {
            return rejected(WebhookIngressRejection::PayloadTooLarge);
        }
        if secret.is_empty() {
            return rejected(WebhookIngressRejection::Unavailable);
        }

        let signature_header = match provider {
            PullRequestReviewEventProvider::Github => "x-hub-signature-256",
            PullRequestReviewEventProvider::Bitbucket => "x-hub-signature",
        };
        let signature = match unique_header(request.headers, signature_header) {
            Ok(Some(value)) => value,
            Ok(None) => return rejected(WebhookIngressRejection::MissingSignature),
            Err(()) => return rejected(WebhookIngressRejection::InvalidHeaders),
        };
        if !verify_hmac_sha256(secret, request.body, signature) {
            return rejected(WebhookIngressRejection::InvalidSignature);
        }

        let (event_header, delivery_header) = match provider {
            PullRequestReviewEventProvider::Github => ("x-github-event", "x-github-delivery"),
            PullRequestReviewEventProvider::Bitbucket => ("x-event-key", "x-request-uuid"),
        };
        let event_name = match unique_header(request.headers, event_header) {
            Ok(Some(value)) => value,
            _ => return rejected(WebhookIngressRejection::InvalidHeaders),
        };
        let delivery_id = match unique_header(request.headers, delivery_header) {
            Ok(Some(value)) if !value.trim().is_empty() => value,
            _ => return rejected(WebhookIngressRejection::InvalidHeaders),
        };
        let Some(kind) = supported_event_kind(provider, event_name) else {
            return WebhookIngressOutcome::Ignored;
        };

        let event = match provider {
            PullRequestReviewEventProvider::Github => {
                normalize_github(tenant_id, delivery_id, request.body)
            }
            PullRequestReviewEventProvider::Bitbucket => normalize_bitbucket(
                &self.commit_resolver,
                tenant_id,
                delivery_id,
                event_name,
                kind,
                request.body,
            ),
        };
        let event = match event {
            Ok(Some(event)) => event,
            Ok(None) => return WebhookIngressOutcome::Ignored,
            Err(rejection) => return rejected(rejection),
        };
        if event.validate().is_err() {
            return rejected(WebhookIngressRejection::InvalidPayload);
        }
        match self.sink.accept(&event) {
            Ok(WebhookEventAcceptance::Accepted) => {
                WebhookIngressOutcome::Accepted(Box::new(event))
            }
            Ok(WebhookEventAcceptance::DuplicateDelivery) => WebhookIngressOutcome::Duplicate,
            Ok(WebhookEventAcceptance::UnchangedReviewState) => WebhookIngressOutcome::Ignored,
            Err(_) => rejected(WebhookIngressRejection::Unavailable),
        }
    }
}

fn rejected(rejection: WebhookIngressRejection) -> WebhookIngressOutcome {
    WebhookIngressOutcome::Rejected(rejection)
}

fn unique_header<'a>(headers: &'a [(&'a str, &'a str)], name: &str) -> Result<Option<&'a str>, ()> {
    let mut values = headers
        .iter()
        .filter(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| *value);
    let value = values.next();
    if values.next().is_some() {
        Err(())
    } else {
        Ok(value)
    }
}

fn is_json_content_type(value: &str) -> bool {
    value
        .split(';')
        .next()
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

fn verify_hmac_sha256(secret: &[u8], body: &[u8], signature: &str) -> bool {
    let Some(encoded) = signature.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(signature) = hex::decode(encoded) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

fn is_full_commit_sha(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn supported_event_kind(
    provider: PullRequestReviewEventProvider,
    event_name: &str,
) -> Option<PullRequestReviewEventKind> {
    match (provider, event_name) {
        (PullRequestReviewEventProvider::Github, "pull_request") => {
            Some(PullRequestReviewEventKind::Opened)
        }
        (PullRequestReviewEventProvider::Bitbucket, "pullrequest:created") => {
            Some(PullRequestReviewEventKind::Opened)
        }
        (PullRequestReviewEventProvider::Bitbucket, "pullrequest:updated") => {
            Some(PullRequestReviewEventKind::Synchronized)
        }
        (PullRequestReviewEventProvider::Bitbucket, "pullrequest:fulfilled")
        | (PullRequestReviewEventProvider::Bitbucket, "pullrequest:rejected") => {
            Some(PullRequestReviewEventKind::Closed)
        }
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct GithubPayload {
    action: String,
    #[serde(default)]
    changes: Option<GithubChanges>,
    pull_request: GithubPullRequest,
    repository: GithubRepository,
    sender: GithubActor,
}

#[derive(Debug, Deserialize)]
struct GithubChanges {
    base: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GithubPullRequest {
    number: u64,
    base: GithubRevision,
    head: GithubRevision,
    #[serde(default)]
    draft: bool,
    merged: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GithubRevision {
    #[serde(rename = "ref")]
    ref_name: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    name: String,
    owner: GithubOwner,
}

#[derive(Debug, Deserialize)]
struct GithubOwner {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GithubActor {
    id: u64,
    login: String,
}

fn normalize_github(
    tenant_id: &str,
    delivery_id: &str,
    body: &[u8],
) -> Result<Option<PullRequestReviewEvent>, WebhookIngressRejection> {
    let payload: GithubPayload =
        serde_json::from_slice(body).map_err(|_| WebhookIngressRejection::InvalidPayload)?;
    let (kind, closed_outcome) = match payload.action.as_str() {
        "opened" => (PullRequestReviewEventKind::Opened, None),
        "reopened" => (PullRequestReviewEventKind::Reopened, None),
        "synchronize" => (PullRequestReviewEventKind::Synchronized, None),
        "converted_to_draft" => (PullRequestReviewEventKind::Synchronized, None),
        "edited"
            if payload
                .changes
                .as_ref()
                .is_some_and(|changes| changes.base.is_some()) =>
        {
            (PullRequestReviewEventKind::Synchronized, None)
        }
        "ready_for_review" => (PullRequestReviewEventKind::ReadyForReview, None),
        "closed" => (
            PullRequestReviewEventKind::Closed,
            Some(
                if payload
                    .pull_request
                    .merged
                    .ok_or(WebhookIngressRejection::InvalidPayload)?
                {
                    PullRequestClosedOutcome::Merged
                } else {
                    PullRequestClosedOutcome::ClosedWithoutMerge
                },
            ),
        ),
        _ => return Ok(None),
    };
    Ok(Some(PullRequestReviewEvent {
        schema_version: PullRequestReviewEventSchemaVersion::V1,
        kind,
        provider: PullRequestReviewEventProvider::Github,
        tenant_id: tenant_id.to_string(),
        workspace: payload.repository.owner.login,
        repository: payload.repository.name,
        pull_request_id: payload.pull_request.number,
        base: PullRequestRevision {
            ref_name: payload.pull_request.base.ref_name,
            sha: payload.pull_request.base.sha,
        },
        head: PullRequestRevision {
            ref_name: payload.pull_request.head.ref_name,
            sha: payload.pull_request.head.sha,
        },
        draft: payload.pull_request.draft,
        closed_outcome,
        actor: PullRequestEventActor {
            id: payload.sender.id.to_string(),
            login: payload.sender.login,
            display_name: None,
        },
        delivery_id: delivery_id.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct BitbucketPayload {
    actor: BitbucketActor,
    pullrequest: BitbucketPullRequest,
    repository: BitbucketRepository,
}

#[derive(Debug, Deserialize)]
struct BitbucketActor {
    uuid: Option<String>,
    account_id: Option<String>,
    nickname: Option<String>,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BitbucketPullRequest {
    id: u64,
    source: BitbucketRevision,
    destination: BitbucketRevision,
    #[serde(default)]
    draft: bool,
}

#[derive(Debug, Deserialize)]
struct BitbucketRevision {
    branch: BitbucketBranch,
    commit: BitbucketCommit,
    repository: BitbucketRepository,
}

#[derive(Debug, Deserialize)]
struct BitbucketBranch {
    name: String,
}

#[derive(Debug, Deserialize)]
struct BitbucketCommit {
    hash: String,
}

#[derive(Debug, Deserialize)]
struct BitbucketRepository {
    full_name: String,
    workspace: BitbucketWorkspace,
}

#[derive(Debug, Deserialize)]
struct BitbucketWorkspace {
    slug: String,
}

fn normalize_bitbucket<C: WebhookCommitResolver>(
    commit_resolver: &C,
    tenant_id: &str,
    delivery_id: &str,
    event_name: &str,
    kind: PullRequestReviewEventKind,
    body: &[u8],
) -> Result<Option<PullRequestReviewEvent>, WebhookIngressRejection> {
    let payload: BitbucketPayload =
        serde_json::from_slice(body).map_err(|_| WebhookIngressRejection::InvalidPayload)?;
    let repository = bitbucket_repository_slug(&payload.repository)?;
    let base_sha = resolve_bitbucket_commit(
        commit_resolver,
        tenant_id,
        &payload.pullrequest.destination.repository,
        &payload.pullrequest.destination.commit.hash,
    )?;
    let head_sha = resolve_bitbucket_commit(
        commit_resolver,
        tenant_id,
        &payload.pullrequest.source.repository,
        &payload.pullrequest.source.commit.hash,
    )?;
    let closed_outcome = match event_name {
        "pullrequest:fulfilled" => Some(PullRequestClosedOutcome::Merged),
        "pullrequest:rejected" => Some(PullRequestClosedOutcome::ClosedWithoutMerge),
        _ => None,
    };
    let actor_id = payload
        .actor
        .uuid
        .or(payload.actor.account_id)
        .filter(|value| !value.trim().is_empty())
        .ok_or(WebhookIngressRejection::InvalidPayload)?;
    let display_name = payload.actor.display_name;
    let actor_login = payload
        .actor
        .nickname
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            display_name
                .clone()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| actor_id.clone());
    Ok(Some(PullRequestReviewEvent {
        schema_version: PullRequestReviewEventSchemaVersion::V1,
        kind,
        provider: PullRequestReviewEventProvider::Bitbucket,
        tenant_id: tenant_id.to_string(),
        workspace: payload.repository.workspace.slug,
        repository,
        pull_request_id: payload.pullrequest.id,
        base: PullRequestRevision {
            ref_name: payload.pullrequest.destination.branch.name,
            sha: base_sha,
        },
        head: PullRequestRevision {
            ref_name: payload.pullrequest.source.branch.name,
            sha: head_sha,
        },
        draft: payload.pullrequest.draft,
        closed_outcome,
        actor: PullRequestEventActor {
            id: actor_id,
            login: actor_login,
            display_name,
        },
        delivery_id: delivery_id.to_string(),
    }))
}

fn resolve_bitbucket_commit<C: WebhookCommitResolver>(
    commit_resolver: &C,
    tenant_id: &str,
    repository: &BitbucketRepository,
    commit_id: &str,
) -> Result<String, WebhookIngressRejection> {
    if is_full_commit_sha(commit_id) {
        return Ok(commit_id.to_string());
    }
    commit_resolver
        .resolve(
            PullRequestReviewEventProvider::Bitbucket,
            tenant_id,
            &repository.workspace.slug,
            &bitbucket_repository_slug(repository)?,
            commit_id,
        )
        .map_err(|_| WebhookIngressRejection::Unavailable)
}

fn bitbucket_repository_slug(
    repository: &BitbucketRepository,
) -> Result<String, WebhookIngressRejection> {
    repository
        .full_name
        .rsplit_once('/')
        .map(|(_, repository)| repository.to_string())
        .filter(|repository| !repository.is_empty())
        .ok_or(WebhookIngressRejection::InvalidPayload)
}

#[cfg(test)]
mod tests {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    use super::*;

    const SECRET: &[u8] = b"synthetic-webhook-test-key";
    const BASE_SHA: &str = "1111111111111111111111111111111111111111";
    const HEAD_SHA: &str = "2222222222222222222222222222222222222222";

    #[derive(Debug, Clone, Copy)]
    struct SyntheticCommitResolver;

    impl WebhookCommitResolver for SyntheticCommitResolver {
        fn resolve(
            &self,
            provider: PullRequestReviewEventProvider,
            tenant_id: &str,
            workspace: &str,
            repository: &str,
            commit_id: &str,
        ) -> Result<String, String> {
            if provider != PullRequestReviewEventProvider::Bitbucket || tenant_id != "tenant-acme" {
                return Err("unexpected synthetic resolver identity".to_string());
            }
            match (workspace, repository) {
                ("acme", "payments") if BASE_SHA.starts_with(commit_id) => Ok(BASE_SHA.to_string()),
                ("contributor", "payments-fork") if HEAD_SHA.starts_with(commit_id) => {
                    Ok(HEAD_SHA.to_string())
                }
                _ => Err("synthetic commit not found".to_string()),
            }
        }
    }

    fn ingress() -> WebhookIngress<InMemoryWebhookEventSink, SyntheticCommitResolver> {
        WebhookIngress::new(InMemoryWebhookEventSink::default(), SyntheticCommitResolver)
    }

    #[derive(Debug, Default)]
    struct FailOnceWebhookEventSink {
        failed: Mutex<bool>,
    }

    impl WebhookEventSink for FailOnceWebhookEventSink {
        fn accept(&self, _: &PullRequestReviewEvent) -> Result<WebhookEventAcceptance, String> {
            let mut failed = self
                .failed
                .lock()
                .map_err(|_| "synthetic sink lock is poisoned".to_string())?;
            if *failed {
                Ok(WebhookEventAcceptance::Accepted)
            } else {
                *failed = true;
                Err("synthetic durable accept failure".to_string())
            }
        }
    }

    fn signature(body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).expect("test HMAC key");
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    fn github_body(action: &str) -> Vec<u8> {
        let base_changed = action == "edited";
        serde_json::to_vec(&serde_json::json!({
            "action": action,
            "changes": if base_changed {
                Some(serde_json::json!({"base": {"ref": {"from": "develop"}}}))
            } else {
                None
            },
            "pull_request": {
                "number": 42,
                "base": {
                    "ref": if base_changed { "release" } else { "main" },
                    "sha": BASE_SHA
                },
                "head": {"ref": "feature/retry", "sha": HEAD_SHA},
                "draft": action == "converted_to_draft",
                "merged": action == "closed"
            },
            "repository": {
                "name": "payments",
                "owner": {"login": "acme"}
            },
            "sender": {"id": 7, "login": "reviewer"}
        }))
        .expect("GitHub fixture")
    }

    fn bitbucket_body() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "actor": {
                "uuid": "user-7",
                "nickname": "reviewer",
                "display_name": "Review User"
            },
            "pullrequest": {
                "id": 42,
                "source": {
                    "branch": {"name": "feature/retry"},
                    "commit": {"hash": &HEAD_SHA[..12]},
                    "repository": {
                        "full_name": "contributor/payments-fork",
                        "workspace": {"slug": "contributor"}
                    }
                },
                "destination": {
                    "branch": {"name": "main"},
                    "commit": {"hash": &BASE_SHA[..12]},
                    "repository": {
                        "full_name": "acme/payments",
                        "workspace": {"slug": "acme"}
                    }
                },
                "draft": false
            },
            "repository": {
                "full_name": "acme/payments",
                "workspace": {"slug": "acme"}
            }
        }))
        .expect("Bitbucket fixture")
    }

    fn github_headers<'a>(signature: &'a str) -> Vec<(&'a str, &'a str)> {
        vec![
            ("Content-Type", "application/json; charset=utf-8"),
            ("X-Hub-Signature-256", signature),
            ("X-GitHub-Event", "pull_request"),
            ("X-GitHub-Delivery", "delivery-1"),
        ]
    }

    fn bitbucket_headers<'a>(signature: &'a str, event_name: &'a str) -> Vec<(&'a str, &'a str)> {
        bitbucket_headers_with_delivery(signature, event_name, "delivery-1")
    }

    fn bitbucket_headers_with_delivery<'a>(
        signature: &'a str,
        event_name: &'a str,
        delivery_id: &'a str,
    ) -> Vec<(&'a str, &'a str)> {
        vec![
            ("content-type", "application/json"),
            ("x-hub-signature", signature),
            ("x-event-key", event_name),
            ("x-request-uuid", delivery_id),
        ]
    }

    #[test]
    fn valid_provider_fixtures_normalize_to_equivalent_events() {
        let github_body = github_body("opened");
        let github_signature = signature(&github_body);
        let github_headers = github_headers(&github_signature);
        let github = ingress().handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &github_headers,
                body: &github_body,
            },
        );

        let bitbucket_body = bitbucket_body();
        let bitbucket_signature = signature(&bitbucket_body);
        let bitbucket_headers = bitbucket_headers(&bitbucket_signature, "pullrequest:created");
        let bitbucket = ingress().handle(
            PullRequestReviewEventProvider::Bitbucket,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &bitbucket_headers,
                body: &bitbucket_body,
            },
        );

        let WebhookIngressOutcome::Accepted(github) = github else {
            panic!("GitHub fixture should be accepted");
        };
        let WebhookIngressOutcome::Accepted(bitbucket) = bitbucket else {
            panic!("Bitbucket fixture should be accepted");
        };
        assert_eq!(github.kind, bitbucket.kind);
        assert_eq!(github.tenant_id, bitbucket.tenant_id);
        assert_eq!(github.workspace, bitbucket.workspace);
        assert_eq!(github.repository, bitbucket.repository);
        assert_eq!(github.pull_request_id, bitbucket.pull_request_id);
        assert_eq!(github.base, bitbucket.base);
        assert_eq!(github.head, bitbucket.head);
        assert_eq!(github.draft, bitbucket.draft);
        assert_eq!(github.actor.id, "7");
        assert_eq!(bitbucket.actor.id, "user-7");
    }

    #[test]
    fn supported_provider_actions_map_to_review_event_kinds_and_outcomes() {
        for (action, expected_kind, expected_outcome) in [
            ("opened", PullRequestReviewEventKind::Opened, None),
            ("reopened", PullRequestReviewEventKind::Reopened, None),
            (
                "synchronize",
                PullRequestReviewEventKind::Synchronized,
                None,
            ),
            (
                "converted_to_draft",
                PullRequestReviewEventKind::Synchronized,
                None,
            ),
            ("edited", PullRequestReviewEventKind::Synchronized, None),
            (
                "ready_for_review",
                PullRequestReviewEventKind::ReadyForReview,
                None,
            ),
            (
                "closed",
                PullRequestReviewEventKind::Closed,
                Some(PullRequestClosedOutcome::Merged),
            ),
        ] {
            let body = github_body(action);
            let signature = signature(&body);
            let headers = github_headers(&signature);
            let outcome = ingress().handle(
                PullRequestReviewEventProvider::Github,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &headers,
                    body: &body,
                },
            );
            let WebhookIngressOutcome::Accepted(event) = outcome else {
                panic!("GitHub {action} should be accepted");
            };
            assert_eq!(event.kind, expected_kind);
            assert_eq!(event.closed_outcome, expected_outcome);
            if action == "converted_to_draft" {
                assert!(event.draft);
            }
            if action == "edited" {
                assert_eq!(event.base.ref_name, "release");
            }
        }

        for (event_name, expected_kind, expected_outcome) in [
            (
                "pullrequest:created",
                PullRequestReviewEventKind::Opened,
                None,
            ),
            (
                "pullrequest:updated",
                PullRequestReviewEventKind::Synchronized,
                None,
            ),
            (
                "pullrequest:fulfilled",
                PullRequestReviewEventKind::Closed,
                Some(PullRequestClosedOutcome::Merged),
            ),
            (
                "pullrequest:rejected",
                PullRequestReviewEventKind::Closed,
                Some(PullRequestClosedOutcome::ClosedWithoutMerge),
            ),
        ] {
            let body = bitbucket_body();
            let signature = signature(&body);
            let headers = bitbucket_headers(&signature, event_name);
            let outcome = ingress().handle(
                PullRequestReviewEventProvider::Bitbucket,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &headers,
                    body: &body,
                },
            );
            let WebhookIngressOutcome::Accepted(event) = outcome else {
                panic!("Bitbucket {event_name} should be accepted");
            };
            assert_eq!(event.kind, expected_kind);
            assert_eq!(event.closed_outcome, expected_outcome);
        }
    }

    #[test]
    fn authentication_precedes_payload_parsing_and_sink_acceptance() {
        let ingress = ingress();
        let invalid_body = b"not-json";
        let headers = github_headers("sha256=invalid");

        let outcome = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &headers,
                body: invalid_body,
            },
        );

        assert_eq!(outcome, rejected(WebhookIngressRejection::InvalidSignature));

        let missing_signature_headers = vec![
            ("content-type", "application/json"),
            ("x-github-event", "pull_request"),
            ("x-github-delivery", "delivery-1"),
        ];
        assert_eq!(
            ingress.handle(
                PullRequestReviewEventProvider::Github,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &missing_signature_headers,
                    body: invalid_body,
                },
            ),
            rejected(WebhookIngressRejection::MissingSignature)
        );
    }

    #[test]
    fn unsupported_events_are_authenticated_then_ignored_without_sink_acceptance() {
        let body = b"not-json";
        let signature = signature(body);
        let headers = vec![
            ("content-type", "application/json"),
            ("x-hub-signature-256", signature.as_str()),
            ("x-github-event", "issues"),
            ("x-github-delivery", "delivery-1"),
        ];
        let ingress = ingress();

        let outcome = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &headers,
                body,
            },
        );

        assert_eq!(outcome, WebhookIngressOutcome::Ignored);
        assert_eq!(outcome.status_code(), 200);
        assert_eq!(outcome.code(), "ignored");
    }

    #[test]
    fn github_edits_without_a_base_change_are_ignored() {
        let mut payload: serde_json::Value =
            serde_json::from_slice(&github_body("edited")).expect("GitHub edited fixture");
        payload["changes"] = serde_json::json!({"title": {"from": "Old title"}});
        let body = serde_json::to_vec(&payload).expect("GitHub title edit fixture");
        let signature = signature(&body);
        let headers = github_headers(&signature);

        let outcome = ingress().handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &headers,
                body: &body,
            },
        );

        assert_eq!(outcome, WebhookIngressOutcome::Ignored);
    }

    #[test]
    fn bitbucket_metadata_updates_do_not_create_duplicate_review_work() {
        let ingress = ingress();
        let body = bitbucket_body();
        let initial_signature = signature(&body);
        let created_headers = bitbucket_headers_with_delivery(
            &initial_signature,
            "pullrequest:created",
            "delivery-created",
        );
        let updated_headers = bitbucket_headers_with_delivery(
            &initial_signature,
            "pullrequest:updated",
            "delivery-updated",
        );

        assert!(matches!(
            ingress.handle(
                PullRequestReviewEventProvider::Bitbucket,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &created_headers,
                    body: &body,
                },
            ),
            WebhookIngressOutcome::Accepted(_)
        ));
        assert_eq!(
            ingress.handle(
                PullRequestReviewEventProvider::Bitbucket,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &updated_headers,
                    body: &body,
                },
            ),
            WebhookIngressOutcome::Ignored
        );
        assert_eq!(
            ingress.handle(
                PullRequestReviewEventProvider::Bitbucket,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &updated_headers,
                    body: &body,
                },
            ),
            WebhookIngressOutcome::Duplicate
        );

        let mut changed: serde_json::Value =
            serde_json::from_slice(&body).expect("Bitbucket fixture");
        changed["pullrequest"]["source"]["commit"]["hash"] =
            serde_json::json!("3333333333333333333333333333333333333333");
        let changed_body = serde_json::to_vec(&changed).expect("Bitbucket changed-head fixture");
        let changed_signature = signature(&changed_body);
        let changed_headers = bitbucket_headers_with_delivery(
            &changed_signature,
            "pullrequest:updated",
            "delivery-new-head",
        );
        assert!(matches!(
            ingress.handle(
                PullRequestReviewEventProvider::Bitbucket,
                "tenant-acme",
                SECRET,
                WebhookHttpRequest {
                    method: "POST",
                    headers: &changed_headers,
                    body: &changed_body,
                },
            ),
            WebhookIngressOutcome::Accepted(_)
        ));
    }

    #[test]
    fn bitbucket_actor_uses_stable_identity_fallbacks() {
        let mut payload: serde_json::Value =
            serde_json::from_slice(&bitbucket_body()).expect("Bitbucket fixture");
        payload["actor"] = serde_json::json!({
            "account_id": "account-7",
            "display_name": "Review User"
        });
        let body = serde_json::to_vec(&payload).expect("Bitbucket actor fallback fixture");
        let signature = signature(&body);
        let headers = bitbucket_headers(&signature, "pullrequest:created");

        let outcome = ingress().handle(
            PullRequestReviewEventProvider::Bitbucket,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &headers,
                body: &body,
            },
        );
        let WebhookIngressOutcome::Accepted(event) = outcome else {
            panic!("Bitbucket actor fallback fixture should be accepted");
        };
        assert_eq!(event.actor.id, "account-7");
        assert_eq!(event.actor.login, "Review User");
    }

    #[test]
    fn duplicate_delivery_ids_are_idempotent() {
        let body = github_body("opened");
        let signature = signature(&body);
        let headers = github_headers(&signature);
        let ingress = ingress();
        let request = WebhookHttpRequest {
            method: "POST",
            headers: &headers,
            body: &body,
        };

        assert!(matches!(
            ingress.handle(
                PullRequestReviewEventProvider::Github,
                "tenant-acme",
                SECRET,
                request
            ),
            WebhookIngressOutcome::Accepted(_)
        ));
        let duplicate = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            request,
        );
        assert_eq!(duplicate, WebhookIngressOutcome::Duplicate);
        assert_eq!(duplicate.status_code(), 200);
        assert_eq!(duplicate.code(), "duplicate");
    }

    #[test]
    fn durable_sink_failures_leave_delivery_retryable() {
        let body = github_body("opened");
        let signature = signature(&body);
        let headers = github_headers(&signature);
        let ingress =
            WebhookIngress::new(FailOnceWebhookEventSink::default(), SyntheticCommitResolver);
        let request = WebhookHttpRequest {
            method: "POST",
            headers: &headers,
            body: &body,
        };

        let failed = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            request,
        );
        assert_eq!(failed, rejected(WebhookIngressRejection::Unavailable));
        assert!(matches!(
            ingress.handle(
                PullRequestReviewEventProvider::Github,
                "tenant-acme",
                SECRET,
                request
            ),
            WebhookIngressOutcome::Accepted(_)
        ));
    }

    #[test]
    fn request_limits_return_stable_http_outcomes() {
        let body = github_body("opened");
        let signature = signature(&body);
        let mut headers = github_headers(&signature);
        headers[0] = ("content-type", "text/plain");
        let ingress = ingress();

        let wrong_type = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &headers,
                body: &body,
            },
        );
        assert_eq!(wrong_type.status_code(), 415);
        assert_eq!(wrong_type.code(), "unsupported_media_type");

        let oversized = vec![b' '; MAX_WEBHOOK_BODY_BYTES + 1];
        headers[0] = ("content-type", "application/json");
        let too_large = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &headers,
                body: &oversized,
            },
        );
        assert_eq!(too_large.status_code(), 413);
        assert_eq!(too_large.code(), "payload_too_large");

        let oversized_header_value = "a".repeat(MAX_WEBHOOK_HEADER_VALUE_BYTES + 1);
        let oversized_headers = vec![
            ("content-type", "application/json"),
            ("x-extra", oversized_header_value.as_str()),
        ];
        let headers_too_large = ingress.handle(
            PullRequestReviewEventProvider::Github,
            "tenant-acme",
            SECRET,
            WebhookHttpRequest {
                method: "POST",
                headers: &oversized_headers,
                body: &body,
            },
        );
        assert_eq!(headers_too_large.status_code(), 431);
        assert_eq!(headers_too_large.code(), "request_headers_too_large");
    }

    #[test]
    fn official_provider_hmac_sha256_vectors_are_supported() {
        assert!(verify_hmac_sha256(
            b"It's a Secret to Everybody",
            b"Hello, World!",
            "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17"
        ));
        assert!(verify_hmac_sha256(
            b"It's a Secret to Everybody",
            b"Hello World!",
            "sha256=a4771c39fbe90f317c7824e83ddef3caae9cb3d976c214ace1f2937e133263c9"
        ));
    }
}
