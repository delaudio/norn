//! Bitbucket Cloud OAuth onboarding boundary for the optional shared service.
//!
//! Refresh material crosses this module only from the OAuth client to the
//! credential-store port; it is intentionally neither serializable nor returned.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const STATE_TTL_MS: u64 = 5 * 60 * 1_000;

/// Minimum Bitbucket Cloud scopes required by automated pull-request review.
pub const REQUIRED_SCOPES: &[&str] = &["account", "workspace", "repository", "pullrequest"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitbucketOAuthConfig {
    pub client_id: String,
    pub redirect_url: String,
}

impl BitbucketOAuthConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.client_id.trim().is_empty() || self.client_id.len() > 256 {
            return Err("Bitbucket OAuth client id is invalid".to_string());
        }
        if !self.redirect_url.starts_with("https://") {
            return Err("Bitbucket OAuth redirect URL must use HTTPS".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BitbucketRefreshToken(String);

impl BitbucketRefreshToken {
    pub fn new(value: String) -> Result<Self, String> {
        if value.trim().is_empty() || value.len() > 16_384 {
            return Err("Bitbucket OAuth refresh token is invalid".to_string());
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for BitbucketRefreshToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BitbucketRefreshToken([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BitbucketAccessToken(String);

impl BitbucketAccessToken {
    pub fn new(value: String) -> Result<Self, String> {
        if value.trim().is_empty() || value.len() > 16_384 {
            return Err("Bitbucket OAuth access token is invalid".to_string());
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for BitbucketAccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BitbucketAccessToken([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitbucketOAuthTokenPair {
    pub access_token: BitbucketAccessToken,
    pub refresh_token: BitbucketRefreshToken,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BitbucketWorkspace {
    pub uuid: String,
    pub slug: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BitbucketRepository {
    pub uuid: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BitbucketAuthorizationStatus {
    Active,
    Revoked,
    WorkspaceAccessRemoved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitbucketEnrollment {
    pub tenant_id: String,
    pub workspace: BitbucketWorkspace,
    pub repositories: Vec<BitbucketRepository>,
    pub status: BitbucketAuthorizationStatus,
}

pub trait BitbucketOAuthClient {
    fn exchange_code(
        &self,
        code: &str,
        redirect_url: &str,
    ) -> Result<BitbucketOAuthTokenPair, String>;
    fn workspace_and_repositories(
        &self,
        access_token: &BitbucketAccessToken,
    ) -> Result<(BitbucketWorkspace, Vec<BitbucketRepository>), String>;
    /// Validate a preconfigured webhook without mutating the provider.
    fn validate_webhook(
        &self,
        access_token: &BitbucketAccessToken,
        workspace: &BitbucketWorkspace,
        repository: &BitbucketRepository,
    ) -> Result<(), String>;
}

/// Implemented by issue #108. The onboarding flow never stores OAuth secrets itself.
pub trait BitbucketCredentialStore {
    fn store_refresh_token(
        &mut self,
        tenant_id: &str,
        workspace_uuid: &str,
        refresh_token: BitbucketRefreshToken,
    ) -> Result<(), String>;
}

pub trait BitbucketEnrollmentStore {
    fn save(&mut self, enrollment: BitbucketEnrollment) -> Result<(), String>;
    fn load(&self, tenant_id: &str) -> Result<Option<BitbucketEnrollment>, String>;
}

#[derive(Default)]
pub struct InMemoryBitbucketEnrollmentStore(BTreeMap<String, BitbucketEnrollment>);

impl BitbucketEnrollmentStore for InMemoryBitbucketEnrollmentStore {
    fn save(&mut self, enrollment: BitbucketEnrollment) -> Result<(), String> {
        self.0.insert(enrollment.tenant_id.clone(), enrollment);
        Ok(())
    }

    fn load(&self, tenant_id: &str) -> Result<Option<BitbucketEnrollment>, String> {
        Ok(self.0.get(tenant_id).cloned())
    }
}

#[derive(Default)]
pub struct SqliteBitbucketEnrollmentStore;

impl BitbucketEnrollmentStore for SqliteBitbucketEnrollmentStore {
    fn save(&mut self, enrollment: BitbucketEnrollment) -> Result<(), String> {
        crate::review_storage::save_bitbucket_oauth_enrollment(&enrollment)
    }

    fn load(&self, tenant_id: &str) -> Result<Option<BitbucketEnrollment>, String> {
        crate::review_storage::load_bitbucket_oauth_enrollment(tenant_id)
    }
}

struct PendingAuthorization {
    browser_binding_digest: String,
    expires_at_ms: u64,
}

pub struct BitbucketOAuthOnboarding<C, E, K> {
    config: BitbucketOAuthConfig,
    client: C,
    enrollments: E,
    credentials: K,
    pending: Mutex<BTreeMap<String, PendingAuthorization>>,
}

impl<C, E, K> BitbucketOAuthOnboarding<C, E, K>
where
    C: BitbucketOAuthClient,
    E: BitbucketEnrollmentStore,
    K: BitbucketCredentialStore,
{
    pub fn new(
        config: BitbucketOAuthConfig,
        client: C,
        enrollments: E,
        credentials: K,
    ) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            config,
            client,
            enrollments,
            credentials,
            pending: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn start_authorization(&self, browser_binding: &str) -> Result<String, String> {
        validate_identifier("browser binding", browser_binding)?;
        let state = random_state()?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "OAuth state lock is unavailable".to_string())?;
        pending.retain(|_, authorization| authorization.expires_at_ms > now_ms());
        pending.insert(
            state.clone(),
            PendingAuthorization {
                browser_binding_digest: digest(browser_binding),
                expires_at_ms: now_ms().saturating_add(STATE_TTL_MS),
            },
        );
        Ok(format!(
            "https://bitbucket.org/site/oauth2/authorize?client_id={}&response_type=code&state={}&scope={}",
            percent_encode(&self.config.client_id), state, REQUIRED_SCOPES.join("%20")
        ))
    }

    pub fn complete_callback(
        &mut self,
        tenant_id: &str,
        code: &str,
        state: &str,
        browser_binding: &str,
        selected_repository_uuids: &[String],
    ) -> Result<BitbucketEnrollment, String> {
        validate_identifier("tenant id", tenant_id)?;
        validate_identifier("authorization code", code)?;
        self.consume_state(state, browser_binding)?;
        let tokens = self.client.exchange_code(code, &self.config.redirect_url)?;
        let (workspace, repositories) = self
            .client
            .workspace_and_repositories(&tokens.access_token)?;
        validate_workspace(&workspace)?;
        let selected = select_repositories(&repositories, selected_repository_uuids)?;
        for repository in &selected {
            self.client
                .validate_webhook(&tokens.access_token, &workspace, repository)?;
        }
        self.credentials
            .store_refresh_token(tenant_id, &workspace.uuid, tokens.refresh_token)?;
        let enrollment = BitbucketEnrollment {
            tenant_id: tenant_id.to_string(),
            workspace,
            repositories: selected,
            status: BitbucketAuthorizationStatus::Active,
        };
        self.enrollments.save(enrollment.clone())?;
        Ok(enrollment)
    }

    pub fn remove_repository(
        &mut self,
        tenant_id: &str,
        repository_uuid: &str,
    ) -> Result<BitbucketEnrollment, String> {
        let mut enrollment = self.require_enrollment(tenant_id)?;
        let before = enrollment.repositories.len();
        enrollment
            .repositories
            .retain(|repository| repository.uuid != repository_uuid);
        if enrollment.repositories.len() == before {
            return Err("Repository is not enrolled for tenant".to_string());
        }
        self.enrollments.save(enrollment.clone())?;
        Ok(enrollment)
    }

    pub fn record_authorization_status(
        &mut self,
        tenant_id: &str,
        status: BitbucketAuthorizationStatus,
    ) -> Result<BitbucketEnrollment, String> {
        let mut enrollment = self.require_enrollment(tenant_id)?;
        enrollment.status = status;
        self.enrollments.save(enrollment.clone())?;
        Ok(enrollment)
    }

    pub fn allows_repository(
        &self,
        tenant_id: &str,
        repository_uuid: &str,
    ) -> Result<bool, String> {
        Ok(self.enrollments.load(tenant_id)?.is_some_and(|enrollment| {
            enrollment.status == BitbucketAuthorizationStatus::Active
                && enrollment
                    .repositories
                    .iter()
                    .any(|repository| repository.uuid == repository_uuid)
        }))
    }

    fn consume_state(&self, state: &str, browser_binding: &str) -> Result<(), String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "OAuth state lock is unavailable".to_string())?;
        let authorization = pending
            .remove(state)
            .ok_or_else(|| "OAuth state is invalid or already used".to_string())?;
        if authorization.expires_at_ms <= now_ms()
            || authorization.browser_binding_digest != digest(browser_binding)
        {
            return Err("OAuth state is invalid or expired".to_string());
        }
        Ok(())
    }

    fn require_enrollment(&self, tenant_id: &str) -> Result<BitbucketEnrollment, String> {
        self.enrollments
            .load(tenant_id)?
            .ok_or_else(|| "No Bitbucket enrollment for tenant".to_string())
    }
}

fn select_repositories(
    available: &[BitbucketRepository],
    selected: &[String],
) -> Result<Vec<BitbucketRepository>, String> {
    if selected.is_empty() {
        return Err("At least one repository must be selected".to_string());
    }
    let ids = selected.iter().collect::<BTreeSet<_>>();
    if ids.len() != selected.len() {
        return Err("Selected repository identifiers must be unique".to_string());
    }
    let enrolled = available
        .iter()
        .filter(|repository| ids.contains(&repository.uuid))
        .cloned()
        .collect::<Vec<_>>();
    if enrolled.len() != ids.len() {
        return Err("Selected repository is not available to the workspace".to_string());
    }
    Ok(enrolled)
}

fn validate_workspace(workspace: &BitbucketWorkspace) -> Result<(), String> {
    validate_identifier("workspace uuid", &workspace.uuid)?;
    validate_identifier("workspace slug", &workspace.slug)
}

fn validate_identifier(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 4_096 {
        Err(format!("Bitbucket OAuth {name} is invalid"))
    } else {
        Ok(())
    }
}

fn random_state() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "Unable to generate OAuth state".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct MockClient {
        validated: RefCell<Vec<String>>,
    }
    impl BitbucketOAuthClient for MockClient {
        fn exchange_code(&self, _: &str, _: &str) -> Result<BitbucketOAuthTokenPair, String> {
            Ok(BitbucketOAuthTokenPair {
                access_token: BitbucketAccessToken::new("access".to_string())?,
                refresh_token: BitbucketRefreshToken::new("refresh".to_string())?,
            })
        }
        fn workspace_and_repositories(
            &self,
            _: &BitbucketAccessToken,
        ) -> Result<(BitbucketWorkspace, Vec<BitbucketRepository>), String> {
            Ok((
                BitbucketWorkspace {
                    uuid: "workspace-1".to_string(),
                    slug: "acme".to_string(),
                },
                vec![
                    BitbucketRepository {
                        uuid: "repo-1".to_string(),
                        name: "api".to_string(),
                    },
                    BitbucketRepository {
                        uuid: "repo-2".to_string(),
                        name: "web".to_string(),
                    },
                ],
            ))
        }
        fn validate_webhook(
            &self,
            _: &BitbucketAccessToken,
            _: &BitbucketWorkspace,
            repository: &BitbucketRepository,
        ) -> Result<(), String> {
            self.validated.borrow_mut().push(repository.uuid.clone());
            Ok(())
        }
    }
    #[derive(Default)]
    struct MockCredentials {
        stored: RefCell<Vec<(String, String)>>,
    }
    impl BitbucketCredentialStore for MockCredentials {
        fn store_refresh_token(
            &mut self,
            tenant: &str,
            workspace: &str,
            _: BitbucketRefreshToken,
        ) -> Result<(), String> {
            self.stored
                .borrow_mut()
                .push((tenant.to_string(), workspace.to_string()));
            Ok(())
        }
    }
    fn onboarding(
    ) -> BitbucketOAuthOnboarding<MockClient, InMemoryBitbucketEnrollmentStore, MockCredentials>
    {
        BitbucketOAuthOnboarding::new(
            BitbucketOAuthConfig {
                client_id: "client".to_string(),
                redirect_url: "https://lachesi.example/callback".to_string(),
            },
            MockClient {
                validated: RefCell::new(Vec::new()),
            },
            InMemoryBitbucketEnrollmentStore::default(),
            MockCredentials::default(),
        )
        .expect("onboarding")
    }

    #[test]
    fn callback_consumes_state_and_enrolls_only_selected_repositories() {
        let mut onboarding = onboarding();
        let url = onboarding.start_authorization("browser-1").expect("start");
        let state = url
            .split("state=")
            .nth(1)
            .expect("state")
            .split('&')
            .next()
            .expect("state");
        let enrollment = onboarding
            .complete_callback(
                "tenant-acme",
                "code",
                state,
                "browser-1",
                &["repo-2".to_string()],
            )
            .expect("callback");
        assert_eq!(enrollment.repositories[0].uuid, "repo-2");
        assert!(onboarding
            .allows_repository("tenant-acme", "repo-2")
            .expect("allowed"));
        assert!(!onboarding
            .allows_repository("tenant-acme", "repo-1")
            .expect("denied"));
        assert!(onboarding
            .complete_callback(
                "tenant-acme",
                "code",
                state,
                "browser-1",
                &["repo-2".to_string()]
            )
            .is_err());
    }

    #[test]
    fn forged_state_and_browser_binding_are_rejected() {
        let mut onboarding = onboarding();
        assert!(onboarding
            .complete_callback(
                "tenant-acme",
                "code",
                "forged",
                "browser-1",
                &["repo-1".to_string()]
            )
            .is_err());
        let url = onboarding.start_authorization("browser-1").expect("start");
        let state = url
            .split("state=")
            .nth(1)
            .expect("state")
            .split('&')
            .next()
            .expect("state");
        assert!(onboarding
            .complete_callback(
                "tenant-acme",
                "code",
                state,
                "other-browser",
                &["repo-1".to_string()]
            )
            .is_err());
    }

    #[test]
    fn revocation_and_workspace_access_removal_disable_every_repository() {
        let mut onboarding = onboarding();
        let url = onboarding.start_authorization("browser-1").expect("start");
        let state = url
            .split("state=")
            .nth(1)
            .expect("state")
            .split('&')
            .next()
            .expect("state");
        onboarding
            .complete_callback(
                "tenant-acme",
                "code",
                state,
                "browser-1",
                &["repo-1".to_string()],
            )
            .expect("callback");
        for status in [
            BitbucketAuthorizationStatus::Revoked,
            BitbucketAuthorizationStatus::WorkspaceAccessRemoved,
        ] {
            onboarding
                .record_authorization_status("tenant-acme", status)
                .expect("status");
            assert!(!onboarding
                .allows_repository("tenant-acme", "repo-1")
                .expect("denied"));
        }
    }
}
