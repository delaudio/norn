//! Generic OpenID Connect authentication boundary for the optional team service.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use openidconnect::core::{
    CoreClient, CoreClientAuthMethod, CoreGenderClaim, CoreProviderMetadata, CoreResponseType,
};
use openidconnect::{
    reqwest, AdditionalClaims, AuthType, AuthenticationFlow, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet, EndpointSet, HttpRequest,
    HttpResponse, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, SyncHttpClient, UserInfoClaims,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::administrative_audit::validate_identifier;
use crate::team_authorization::{TeamActor, TeamActorKind, TeamRole};

const LOGIN_ATTEMPT_TTL_MS: u64 = 5 * 60 * 1_000;
const OIDC_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OIDC_HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_SESSION_TTL_MS: u64 = 60 * 1_000;
const MAX_SESSION_TTL_MS: u64 = 24 * 60 * 60 * 1_000;
const MAX_GROUPS: usize = 128;
const MAX_GROUP_LENGTH: usize = 256;
const MAX_IDENTIFIER_LENGTH: usize = 256;
const MAX_CONFIGURED_CAPACITY: usize = 100_000;
const OPAQUE_SECRET_BYTES: usize = 32;

type TeamOidcClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcPublicConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub redirect_url: String,
    pub group_claim: String,
    pub scopes: Vec<String>,
    pub session_ttl_ms: u64,
    pub max_pending_logins: usize,
    pub max_sessions: usize,
}

impl OidcPublicConfig {
    pub fn validate(&self) -> Result<(), OidcAuthenticationError> {
        let issuer = IssuerUrl::new(self.issuer_url.clone())
            .map_err(|_| OidcAuthenticationError::InvalidConfiguration("issuerUrl"))?;
        if issuer.url().scheme() != "https" {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "issuerUrl.scheme",
            ));
        }
        if issuer.url().query().is_some() || issuer.url().fragment().is_some() {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "issuerUrl.shape",
            ));
        }
        validate_non_empty("clientId", &self.client_id, MAX_IDENTIFIER_LENGTH)?;
        let redirect = RedirectUrl::new(self.redirect_url.clone())
            .map_err(|_| OidcAuthenticationError::InvalidConfiguration("redirectUrl"))?;
        let is_loopback_http = redirect.url().scheme() == "http"
            && redirect
                .url()
                .host_str()
                .is_some_and(|host| host == "localhost" || host == "127.0.0.1" || host == "::1");
        if redirect.url().scheme() != "https" && !is_loopback_http {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "redirectUrl.scheme",
            ));
        }
        if redirect.url().fragment().is_some() {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "redirectUrl.fragment",
            ));
        }
        validate_non_empty("groupClaim", &self.group_claim, MAX_IDENTIFIER_LENGTH)?;
        if self.scopes.len() > 32 {
            return Err(OidcAuthenticationError::InvalidConfiguration("scopes"));
        }
        for scope in &self.scopes {
            validate_non_empty("scopes", scope, MAX_IDENTIFIER_LENGTH)?;
        }
        if !(MIN_SESSION_TTL_MS..=MAX_SESSION_TTL_MS).contains(&self.session_ttl_ms) {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "sessionTtlMs",
            ));
        }
        if self.max_pending_logins == 0
            || self.max_pending_logins > MAX_CONFIGURED_CAPACITY
            || self.max_sessions == 0
            || self.max_sessions > MAX_CONFIGURED_CAPACITY
        {
            return Err(OidcAuthenticationError::InvalidConfiguration("capacity"));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct OidcClientSecret(String);

impl OidcClientSecret {
    pub fn new(secret: String) -> Result<Self, OidcAuthenticationError> {
        validate_non_empty("clientSecret", &secret, 4_096)?;
        Ok(Self(secret))
    }
}

impl fmt::Debug for OidcClientSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OidcClientSecret([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcGroupMapping {
    pub group: String,
    pub role: TeamRole,
    pub team_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcIdentityMapping {
    pub organization_id: String,
    pub groups: Vec<OidcGroupMapping>,
}

impl OidcIdentityMapping {
    pub fn validate(&self) -> Result<(), OidcAuthenticationError> {
        validate_identifier("organizationId", &self.organization_id)
            .map_err(|_| OidcAuthenticationError::InvalidConfiguration("organizationId"))?;
        if self.groups.len() > MAX_GROUPS {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "groupMappings",
            ));
        }
        let mut configured_groups = BTreeSet::new();
        for mapping in &self.groups {
            validate_non_empty("groupMappings.group", &mapping.group, MAX_GROUP_LENGTH)?;
            if !configured_groups.insert(mapping.group.as_str()) {
                return Err(OidcAuthenticationError::InvalidConfiguration(
                    "groupMappings.group",
                ));
            }
            if matches!(mapping.role, TeamRole::Unknown | TeamRole::ServiceAccount) {
                return Err(OidcAuthenticationError::InvalidConfiguration(
                    "groupMappings.role",
                ));
            }
            if mapping.team_ids.len() > MAX_GROUPS {
                return Err(OidcAuthenticationError::InvalidConfiguration(
                    "groupMappings.teamIds",
                ));
            }
            for team_id in &mapping.team_ids {
                validate_identifier("groupMappings.teamIds", team_id).map_err(|_| {
                    OidcAuthenticationError::InvalidConfiguration("groupMappings.teamIds")
                })?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OidcSessionHandle(String);

impl OidcSessionHandle {
    pub fn from_secret(secret: String) -> Result<Self, OidcAuthenticationError> {
        validate_opaque_secret(&secret).map_err(|_| OidcAuthenticationError::InvalidSession)?;
        Ok(Self(secret))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OidcLoginBinding(String);

impl OidcLoginBinding {
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn from_secret(secret: String) -> Result<Self, OidcAuthenticationError> {
        validate_opaque_secret(&secret).map_err(|_| OidcAuthenticationError::InvalidState)?;
        Ok(Self(secret))
    }
}

impl fmt::Debug for OidcLoginBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OidcLoginBinding([REDACTED])")
    }
}

impl fmt::Debug for OidcSessionHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OidcSessionHandle([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OidcLoginStart {
    pub authorization_url: String,
    pub browser_binding: OidcLoginBinding,
    pub expires_at_ms: u64,
}

impl fmt::Debug for OidcLoginStart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OidcLoginStart")
            .field("authorization_url", &"[REDACTED]")
            .field("browser_binding", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcSession {
    pub handle: OidcSessionHandle,
    pub expires_at_ms: u64,
}

struct PendingLogin {
    nonce: Nonce,
    pkce_verifier: PkceCodeVerifier,
    browser_binding_digest: String,
    expires_at_ms: u64,
}

#[derive(Clone)]
struct StoredSession {
    actor: TeamActor,
    expires_at_ms: u64,
}

#[derive(Default)]
struct OidcState {
    pending_logins: HashMap<String, PendingLogin>,
    sessions: HashMap<String, StoredSession>,
    reserved_sessions: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DynamicClaims(HashMap<String, Value>);

impl AdditionalClaims for DynamicClaims {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcAuthenticationError {
    InvalidConfiguration(&'static str),
    DiscoveryFailed,
    CapacityExceeded,
    InvalidState,
    LoginExpired,
    TokenExchangeFailed,
    MissingIdToken,
    InvalidIdToken,
    UserInfoFailed,
    InvalidGroupClaim,
    AmbiguousRoleMapping,
    Unauthorized,
    InvalidIdentity,
    InvalidSession,
    SessionExpired,
    InternalState,
}

impl fmt::Display for OidcAuthenticationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(field) => {
                write!(formatter, "invalid OIDC configuration: {field}")
            }
            Self::DiscoveryFailed => formatter.write_str("OIDC provider discovery failed"),
            Self::CapacityExceeded => formatter.write_str("OIDC capacity exceeded"),
            Self::InvalidState => formatter.write_str("invalid OIDC login state"),
            Self::LoginExpired => formatter.write_str("OIDC login attempt expired"),
            Self::TokenExchangeFailed => formatter.write_str("OIDC token exchange failed"),
            Self::MissingIdToken => {
                formatter.write_str("OIDC response did not include an ID token")
            }
            Self::InvalidIdToken => formatter.write_str("OIDC ID token validation failed"),
            Self::UserInfoFailed => formatter.write_str("OIDC UserInfo validation failed"),
            Self::InvalidGroupClaim => formatter.write_str("invalid OIDC group claim"),
            Self::AmbiguousRoleMapping => formatter.write_str("ambiguous OIDC role mapping"),
            Self::Unauthorized => formatter.write_str("OIDC user is not authorized"),
            Self::InvalidIdentity => formatter.write_str("invalid authenticated OIDC identity"),
            Self::InvalidSession => formatter.write_str("invalid OIDC session"),
            Self::SessionExpired => formatter.write_str("OIDC session expired"),
            Self::InternalState => formatter.write_str("OIDC state is unavailable"),
        }
    }
}

impl std::error::Error for OidcAuthenticationError {}

pub struct OidcAuthenticator<C> {
    config: OidcPublicConfig,
    identity_mapping: OidcIdentityMapping,
    client: Mutex<TeamOidcClient>,
    client_secret: OidcClientSecret,
    http_client: HttpsOnlyHttpClient<C>,
    state: Mutex<OidcState>,
}

struct SessionReservation<'a, C> {
    authenticator: &'a OidcAuthenticator<C>,
    committed: bool,
}

impl<C> Drop for SessionReservation<'_, C> {
    fn drop(&mut self) {
        if !self.committed {
            if let Ok(mut state) = self.authenticator.state.lock() {
                state.reserved_sessions = state.reserved_sessions.saturating_sub(1);
            }
        }
    }
}

struct HttpsOnlyHttpClient<C>(C);

#[derive(Debug)]
enum HttpsOnlyHttpClientError<E> {
    InvalidRequestUrl,
    InsecureScheme,
    Inner(E),
}

impl<E: fmt::Display> fmt::Display for HttpsOnlyHttpClientError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequestUrl => formatter.write_str("invalid OIDC request URL"),
            Self::InsecureScheme => formatter.write_str("OIDC request URL must use HTTPS"),
            Self::Inner(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for HttpsOnlyHttpClientError<E> {}

impl<C> SyncHttpClient for HttpsOnlyHttpClient<C>
where
    C: SyncHttpClient,
{
    type Error = HttpsOnlyHttpClientError<C::Error>;

    fn call(&self, request: HttpRequest) -> Result<HttpResponse, Self::Error> {
        let url = openidconnect::url::Url::parse(&request.uri().to_string())
            .map_err(|_| HttpsOnlyHttpClientError::InvalidRequestUrl)?;
        if url.scheme() != "https" {
            return Err(HttpsOnlyHttpClientError::InsecureScheme);
        }
        self.0
            .call(request)
            .map_err(HttpsOnlyHttpClientError::Inner)
    }
}

impl OidcAuthenticator<reqwest::blocking::Client> {
    pub fn discover(
        config: OidcPublicConfig,
        client_secret: OidcClientSecret,
        identity_mapping: OidcIdentityMapping,
    ) -> Result<Self, OidcAuthenticationError> {
        let http_client = reqwest::blocking::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(OIDC_HTTP_CONNECT_TIMEOUT)
            .timeout(OIDC_HTTP_TOTAL_TIMEOUT)
            .build()
            .map_err(|_| OidcAuthenticationError::DiscoveryFailed)?;
        Self::discover_with_http_client(config, client_secret, identity_mapping, http_client)
    }
}

impl<C> OidcAuthenticator<C>
where
    C: SyncHttpClient,
{
    fn discover_with_http_client(
        config: OidcPublicConfig,
        client_secret: OidcClientSecret,
        identity_mapping: OidcIdentityMapping,
        http_client: C,
    ) -> Result<Self, OidcAuthenticationError> {
        config.validate()?;
        identity_mapping.validate()?;
        let issuer = IssuerUrl::new(config.issuer_url.clone())
            .map_err(|_| OidcAuthenticationError::InvalidConfiguration("issuerUrl"))?;
        let http_client = HttpsOnlyHttpClient(http_client);
        let metadata = CoreProviderMetadata::discover(&issuer, &http_client)
            .map_err(|_| OidcAuthenticationError::DiscoveryFailed)?;
        let client = build_client(metadata, &config, &client_secret)?;
        Ok(Self {
            config,
            identity_mapping,
            client: Mutex::new(client),
            client_secret,
            http_client,
            state: Mutex::new(OidcState::default()),
        })
    }

    pub fn begin_login(&self) -> Result<OidcLoginStart, OidcAuthenticationError> {
        self.begin_login_at(now_ms()?)
    }

    fn begin_login_at(&self, now_ms: u64) -> Result<OidcLoginStart, OidcAuthenticationError> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let client = self
            .client
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)?
            .clone();
        let mut request = client.authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );
        for scope in &self.config.scopes {
            request = request.add_scope(Scope::new(scope.clone()));
        }
        let (authorization_url, state, nonce) = request.set_pkce_challenge(pkce_challenge).url();
        let browser_binding = OidcLoginBinding(new_opaque_secret()?);
        let expires_at_ms = now_ms.saturating_add(LOGIN_ATTEMPT_TTL_MS);
        let mut state_store = self
            .state
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)?;
        prune_expired(&mut state_store, now_ms);
        if state_store.pending_logins.len() >= self.config.max_pending_logins {
            return Err(OidcAuthenticationError::CapacityExceeded);
        }
        let state_digest = secret_digest(state.secret());
        if state_store.pending_logins.contains_key(&state_digest) {
            return Err(OidcAuthenticationError::InternalState);
        }
        state_store.pending_logins.insert(
            state_digest,
            PendingLogin {
                nonce,
                pkce_verifier,
                browser_binding_digest: secret_digest(browser_binding.expose_secret()),
                expires_at_ms,
            },
        );
        Ok(OidcLoginStart {
            authorization_url: authorization_url.to_string(),
            browser_binding,
            expires_at_ms,
        })
    }

    pub fn complete_login(
        &self,
        authorization_code: String,
        state: String,
        browser_binding: &OidcLoginBinding,
    ) -> Result<OidcSession, OidcAuthenticationError> {
        self.complete_login_at_with_binding_and_session_clock(
            authorization_code,
            state,
            Some(browser_binding),
            now_ms()?,
            true,
        )
    }

    #[cfg(test)]
    fn complete_login_at(
        &self,
        authorization_code: String,
        state: String,
        now_ms: u64,
    ) -> Result<OidcSession, OidcAuthenticationError> {
        self.complete_login_at_with_binding_and_session_clock(
            authorization_code,
            state,
            None,
            now_ms,
            false,
        )
    }

    #[cfg(test)]
    fn complete_login_at_with_binding(
        &self,
        authorization_code: String,
        state: String,
        browser_binding: Option<&OidcLoginBinding>,
        now_ms: u64,
    ) -> Result<OidcSession, OidcAuthenticationError> {
        self.complete_login_at_with_binding_and_session_clock(
            authorization_code,
            state,
            browser_binding,
            now_ms,
            false,
        )
    }

    fn complete_login_at_with_binding_and_session_clock(
        &self,
        authorization_code: String,
        state: String,
        browser_binding: Option<&OidcLoginBinding>,
        callback_now_ms: u64,
        use_current_session_time: bool,
    ) -> Result<OidcSession, OidcAuthenticationError> {
        validate_non_empty("authorizationCode", &authorization_code, 8_192)
            .map_err(|_| OidcAuthenticationError::TokenExchangeFailed)?;
        validate_non_empty("state", &state, 8_192)
            .map_err(|_| OidcAuthenticationError::InvalidState)?;
        let pending = {
            let mut state_store = self
                .state
                .lock()
                .map_err(|_| OidcAuthenticationError::InternalState)?;
            state_store
                .sessions
                .retain(|_, session| session.expires_at_ms > callback_now_ms);
            let pending = state_store
                .pending_logins
                .get(&secret_digest(&state))
                .ok_or(OidcAuthenticationError::InvalidState)?;
            if let Some(browser_binding) = browser_binding {
                if pending.browser_binding_digest != secret_digest(browser_binding.expose_secret())
                {
                    return Err(OidcAuthenticationError::InvalidState);
                }
            }
            if pending.expires_at_ms <= callback_now_ms {
                state_store.pending_logins.remove(&secret_digest(&state));
                return Err(OidcAuthenticationError::LoginExpired);
            }
            if state_store.sessions.len() + state_store.reserved_sessions
                >= self.config.max_sessions
            {
                return Err(OidcAuthenticationError::CapacityExceeded);
            }
            state_store.reserved_sessions += 1;
            state_store
                .pending_logins
                .remove(&secret_digest(&state))
                .ok_or(OidcAuthenticationError::InvalidState)?
        };
        let mut reservation = SessionReservation {
            authenticator: self,
            committed: false,
        };
        self.refresh_client()?;

        let client = self
            .client
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)?
            .clone();
        let response = client
            .exchange_code(AuthorizationCode::new(authorization_code))
            .map_err(|_| OidcAuthenticationError::TokenExchangeFailed)?
            .set_pkce_verifier(pending.pkce_verifier)
            .request(&self.http_client)
            .map_err(|_| OidcAuthenticationError::TokenExchangeFailed)?;
        let id_token = response
            .extra_fields()
            .id_token()
            .ok_or(OidcAuthenticationError::MissingIdToken)?;
        let claims = id_token
            .claims(&client.id_token_verifier(), &pending.nonce)
            .map_err(|_| OidcAuthenticationError::InvalidIdToken)?;
        let subject = claims.subject().to_owned();
        let user_info: UserInfoClaims<DynamicClaims, CoreGenderClaim> = client
            .user_info(response.access_token().to_owned(), Some(subject.clone()))
            .map_err(|_| OidcAuthenticationError::UserInfoFailed)?
            .request(&self.http_client)
            .map_err(|_| OidcAuthenticationError::UserInfoFailed)?;
        let groups = parse_groups(
            user_info
                .additional_claims()
                .0
                .get(&self.config.group_claim),
        )?;
        let actor = self.map_actor(subject.as_str(), &groups)?;
        let session_created_at_ms = if use_current_session_time {
            now_ms()?
        } else {
            callback_now_ms
        };
        self.create_session(actor, session_created_at_ms, &mut reservation)
    }

    fn refresh_client(&self) -> Result<(), OidcAuthenticationError> {
        let issuer = IssuerUrl::new(self.config.issuer_url.clone())
            .map_err(|_| OidcAuthenticationError::InvalidConfiguration("issuerUrl"))?;
        let metadata = CoreProviderMetadata::discover(&issuer, &self.http_client)
            .map_err(|_| OidcAuthenticationError::DiscoveryFailed)?;
        let refreshed = build_client(metadata, &self.config, &self.client_secret)?;
        *self
            .client
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)? = refreshed;
        Ok(())
    }

    pub fn resolve_session(
        &self,
        session: &OidcSessionHandle,
    ) -> Result<TeamActor, OidcAuthenticationError> {
        self.resolve_session_at(session, now_ms()?)
    }

    fn resolve_session_at(
        &self,
        session: &OidcSessionHandle,
        now_ms: u64,
    ) -> Result<TeamActor, OidcAuthenticationError> {
        let digest = secret_digest(session.expose_secret());
        let mut state_store = self
            .state
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)?;
        let stored = state_store
            .sessions
            .get(&digest)
            .cloned()
            .ok_or(OidcAuthenticationError::InvalidSession)?;
        if stored.expires_at_ms <= now_ms {
            state_store.sessions.remove(&digest);
            return Err(OidcAuthenticationError::SessionExpired);
        }
        Ok(stored.actor)
    }

    pub fn logout(&self, session: &OidcSessionHandle) -> Result<(), OidcAuthenticationError> {
        self.logout_at(session, now_ms()?)
    }

    fn logout_at(
        &self,
        session: &OidcSessionHandle,
        now_ms: u64,
    ) -> Result<(), OidcAuthenticationError> {
        let mut state_store = self
            .state
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)?;
        let digest = secret_digest(session.expose_secret());
        let expired = state_store
            .sessions
            .get(&digest)
            .is_some_and(|stored| stored.expires_at_ms <= now_ms);
        state_store
            .sessions
            .retain(|_, stored| stored.expires_at_ms > now_ms);
        if expired {
            return Err(OidcAuthenticationError::SessionExpired);
        }
        state_store
            .sessions
            .remove(&digest)
            .map(|_| ())
            .ok_or(OidcAuthenticationError::InvalidSession)
    }

    fn map_actor(
        &self,
        subject: &str,
        groups: &[String],
    ) -> Result<TeamActor, OidcAuthenticationError> {
        validate_non_empty("subject", subject, 4_096)
            .map_err(|_| OidcAuthenticationError::InvalidIdentity)?;
        let matched = self
            .identity_mapping
            .groups
            .iter()
            .filter(|mapping| groups.iter().any(|group| group == &mapping.group))
            .collect::<Vec<_>>();
        if matched.is_empty() {
            return Err(OidcAuthenticationError::Unauthorized);
        }
        let role = matched
            .first()
            .map_or(TeamRole::Unknown, |mapping| mapping.role);
        if matched.iter().any(|mapping| mapping.role != role) {
            return Err(OidcAuthenticationError::AmbiguousRoleMapping);
        }
        let team_ids = matched
            .iter()
            .flat_map(|mapping| mapping.team_ids.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let stable_id = format!(
            "user:oidc:{}",
            secret_digest(&format!("{}\0{subject}", self.config.issuer_url))
        );
        TeamActor::from_authenticated_claims(
            stable_id,
            TeamActorKind::User,
            self.identity_mapping.organization_id.clone(),
            team_ids,
            role,
        )
        .map_err(|_| OidcAuthenticationError::InvalidIdentity)
    }

    fn create_session(
        &self,
        actor: TeamActor,
        now_ms: u64,
        reservation: &mut SessionReservation<'_, C>,
    ) -> Result<OidcSession, OidcAuthenticationError> {
        let handle = OidcSessionHandle(new_opaque_secret()?);
        let expires_at_ms = now_ms.saturating_add(self.config.session_ttl_ms);
        let mut state_store = self
            .state
            .lock()
            .map_err(|_| OidcAuthenticationError::InternalState)?;
        prune_expired(&mut state_store, now_ms);
        let session_digest = secret_digest(handle.expose_secret());
        if state_store.sessions.contains_key(&session_digest) {
            return Err(OidcAuthenticationError::InternalState);
        }
        state_store.sessions.insert(
            session_digest,
            StoredSession {
                actor,
                expires_at_ms,
            },
        );
        state_store.reserved_sessions = state_store.reserved_sessions.saturating_sub(1);
        reservation.committed = true;
        Ok(OidcSession {
            handle,
            expires_at_ms,
        })
    }
}

fn validate_provider_metadata(
    metadata: &CoreProviderMetadata,
) -> Result<(), OidcAuthenticationError> {
    if !metadata
        .response_types_supported()
        .iter()
        .any(|response_types| {
            response_types.len() == 1 && response_types.contains(&CoreResponseType::Code)
        })
    {
        return Err(OidcAuthenticationError::InvalidConfiguration(
            "providerMetadata.responseTypesSupported",
        ));
    }
    validate_https_endpoint(
        "providerMetadata.authorizationEndpoint",
        metadata.authorization_endpoint().url().scheme(),
    )?;
    validate_https_endpoint(
        "providerMetadata.jwksUri",
        metadata.jwks_uri().url().scheme(),
    )?;
    let token_endpoint =
        metadata
            .token_endpoint()
            .ok_or(OidcAuthenticationError::InvalidConfiguration(
                "providerMetadata.tokenEndpoint",
            ))?;
    validate_https_endpoint(
        "providerMetadata.tokenEndpoint",
        token_endpoint.url().scheme(),
    )?;
    let userinfo_endpoint =
        metadata
            .userinfo_endpoint()
            .ok_or(OidcAuthenticationError::InvalidConfiguration(
                "providerMetadata.userinfoEndpoint",
            ))?;
    validate_https_endpoint(
        "providerMetadata.userinfoEndpoint",
        userinfo_endpoint.url().scheme(),
    )?;
    validate_token_endpoint_auth_method(metadata)
}

fn build_client(
    metadata: CoreProviderMetadata,
    config: &OidcPublicConfig,
    client_secret: &OidcClientSecret,
) -> Result<TeamOidcClient, OidcAuthenticationError> {
    validate_provider_metadata(&metadata)?;
    let auth_method = selected_token_endpoint_auth_method(&metadata)?;
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(client_secret.0.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.redirect_url.clone())
            .map_err(|_| OidcAuthenticationError::InvalidConfiguration("redirectUrl"))?,
    );
    Ok(match auth_method {
        CoreClientAuthMethod::ClientSecretPost => client.set_auth_type(AuthType::RequestBody),
        CoreClientAuthMethod::ClientSecretBasic => client.set_auth_type(AuthType::BasicAuth),
        _ => {
            return Err(OidcAuthenticationError::InvalidConfiguration(
                "providerMetadata.tokenEndpointAuthMethod",
            ))
        }
    })
}

fn validate_token_endpoint_auth_method(
    metadata: &CoreProviderMetadata,
) -> Result<(), OidcAuthenticationError> {
    selected_token_endpoint_auth_method(metadata).map(|_| ())
}

fn selected_token_endpoint_auth_method(
    metadata: &CoreProviderMetadata,
) -> Result<CoreClientAuthMethod, OidcAuthenticationError> {
    let Some(methods) = metadata.token_endpoint_auth_methods_supported() else {
        return Ok(CoreClientAuthMethod::ClientSecretBasic);
    };
    if methods.contains(&CoreClientAuthMethod::ClientSecretBasic) {
        return Ok(CoreClientAuthMethod::ClientSecretBasic);
    }
    if methods.contains(&CoreClientAuthMethod::ClientSecretPost) {
        return Ok(CoreClientAuthMethod::ClientSecretPost);
    }
    Err(OidcAuthenticationError::InvalidConfiguration(
        "providerMetadata.tokenEndpointAuthMethod",
    ))
}

fn validate_https_endpoint(
    field: &'static str,
    scheme: &str,
) -> Result<(), OidcAuthenticationError> {
    if scheme == "https" {
        Ok(())
    } else {
        Err(OidcAuthenticationError::InvalidConfiguration(field))
    }
}

fn parse_groups(claim: Option<&Value>) -> Result<Vec<String>, OidcAuthenticationError> {
    let Some(Value::Array(values)) = claim else {
        return if claim.is_none() {
            Ok(Vec::new())
        } else {
            Err(OidcAuthenticationError::InvalidGroupClaim)
        };
    };
    if values.len() > MAX_GROUPS {
        return Err(OidcAuthenticationError::InvalidGroupClaim);
    }
    let mut groups = BTreeSet::new();
    for value in values {
        let Value::String(group) = value else {
            return Err(OidcAuthenticationError::InvalidGroupClaim);
        };
        validate_non_empty("groups", group, MAX_GROUP_LENGTH)
            .map_err(|_| OidcAuthenticationError::InvalidGroupClaim)?;
        groups.insert(group.clone());
    }
    Ok(groups.into_iter().collect())
}

fn prune_expired(state: &mut OidcState, now_ms: u64) {
    state
        .pending_logins
        .retain(|_, pending| pending.expires_at_ms > now_ms);
    state
        .sessions
        .retain(|_, session| session.expires_at_ms > now_ms);
}

fn secret_digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn new_opaque_secret() -> Result<String, OidcAuthenticationError> {
    let mut bytes = [0_u8; OPAQUE_SECRET_BYTES];
    getrandom::getrandom(&mut bytes).map_err(|_| OidcAuthenticationError::InternalState)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn validate_opaque_secret(secret: &str) -> Result<(), ()> {
    let decoded = URL_SAFE_NO_PAD.decode(secret).map_err(|_| ())?;
    (decoded.len() == OPAQUE_SECRET_BYTES)
        .then_some(())
        .ok_or(())
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
    max_length: usize,
) -> Result<(), OidcAuthenticationError> {
    if value.trim().is_empty() || value.len() > max_length || value.chars().any(char::is_control) {
        return Err(OidcAuthenticationError::InvalidConfiguration(field));
    }
    Ok(())
}

fn now_ms() -> Result<u64, OidcAuthenticationError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| OidcAuthenticationError::InternalState)?
        .as_millis();
    u64::try_from(millis).map_err(|_| OidcAuthenticationError::InternalState)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use chrono::{Duration, Utc};
    use openidconnect::core::{
        CoreHmacKey, CoreIdToken, CoreIdTokenClaims, CoreJwsSigningAlgorithm,
    };
    use openidconnect::http::header::{HeaderValue, CONTENT_TYPE};
    use openidconnect::http::{Response, StatusCode};
    use openidconnect::{
        AccessToken, Audience, EmptyAdditionalClaims, HttpRequest, HttpResponse, IssuerUrl, Nonce,
        StandardClaims, SubjectIdentifier,
    };
    use serde_json::json;

    use super::*;

    const ISSUER: &str = "https://issuer.example";
    const CLIENT_ID: &str = "lachesi-team-client";
    const CLIENT_SECRET: &str = "test-client-secret-with-sufficient-entropy";
    const SUBJECT: &str = "stable-subject-123";

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum TokenFault {
        #[default]
        None,
        Issuer,
        Audience,
        Signature,
        Nonce,
        Expired,
    }

    #[derive(Debug, Default)]
    struct TestIssuerState {
        nonce: Option<String>,
        expected_pkce_challenge: Option<String>,
        pkce_verified: bool,
        token_fault: TokenFault,
        insecure_endpoint: Option<&'static str>,
        token_auth_method: Option<&'static str>,
        token_request_used_post: bool,
        discovery_count: u32,
        groups: Value,
    }

    fn config() -> OidcPublicConfig {
        OidcPublicConfig {
            issuer_url: ISSUER.to_string(),
            client_id: CLIENT_ID.to_string(),
            redirect_url: "https://team.example/auth/callback".to_string(),
            group_claim: "lachesi_groups".to_string(),
            scopes: vec!["profile".to_string(), "groups".to_string()],
            session_ttl_ms: 10 * 60 * 1_000,
            max_pending_logins: 8,
            max_sessions: 8,
        }
    }

    fn mapping() -> OidcIdentityMapping {
        OidcIdentityMapping {
            organization_id: "organization-acme".to_string(),
            groups: vec![
                OidcGroupMapping {
                    group: "engineering".to_string(),
                    role: TeamRole::Member,
                    team_ids: vec!["team-engineering".to_string()],
                },
                OidcGroupMapping {
                    group: "security-admins".to_string(),
                    role: TeamRole::Admin,
                    team_ids: vec!["team-security".to_string()],
                },
            ],
        }
    }

    fn json_response(body: Value) -> HttpResponse {
        Response::builder()
            .status(StatusCode::OK)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/json; charset=utf-8"),
            )
            .body(serde_json::to_vec(&body).expect("serialize response"))
            .expect("build response")
    }

    fn test_http_client(
        issuer_state: Arc<Mutex<TestIssuerState>>,
    ) -> impl Fn(HttpRequest) -> Result<HttpResponse, io::Error> + Send + Sync {
        move |request: HttpRequest| {
            let path = request.uri().path();
            match path {
                "/.well-known/openid-configuration" => {
                    let (insecure_endpoint, token_auth_method) = {
                        let mut state = issuer_state
                            .lock()
                            .map_err(|_| io::Error::other("issuer state unavailable"))?;
                        state.discovery_count += 1;
                        (state.insecure_endpoint, state.token_auth_method)
                    };
                    let endpoint = |name: &str, path: &str| {
                        let scheme = if insecure_endpoint == Some(name) {
                            "http"
                        } else {
                            "https"
                        };
                        format!("{scheme}://issuer.example{path}")
                    };
                    let mut metadata = json!({
                    "issuer": ISSUER,
                    "authorization_endpoint": endpoint("authorization", "/authorize"),
                    "token_endpoint": endpoint("token", "/token"),
                    "userinfo_endpoint": endpoint("userinfo", "/userinfo"),
                    "jwks_uri": endpoint("jwks", "/jwks"),
                    "response_types_supported": ["code"],
                    "subject_types_supported": ["public"],
                    "id_token_signing_alg_values_supported": ["HS256"]
                    });
                    if let Some(method) = token_auth_method {
                        metadata["token_endpoint_auth_methods_supported"] = json!([method]);
                    }
                    Ok(json_response(metadata))
                }
                "/jwks" => Ok(json_response(json!({"keys": []}))),
                "/token" => {
                    let body = String::from_utf8(request.body().clone())
                        .map_err(|_| io::Error::other("invalid token request"))?;
                    let params = openidconnect::url::form_urlencoded::parse(body.as_bytes())
                        .into_owned()
                        .collect::<HashMap<_, _>>();
                    let mut state = issuer_state
                        .lock()
                        .map_err(|_| io::Error::other("issuer state unavailable"))?;
                    state.token_request_used_post = params.contains_key("client_id")
                        && params.contains_key("client_secret")
                        && request.headers().get("authorization").is_none();
                    let verifier = params
                        .get("code_verifier")
                        .ok_or_else(|| io::Error::other("missing PKCE verifier"))?;
                    let challenge = PkceCodeChallenge::from_code_verifier_sha256(
                        &PkceCodeVerifier::new(verifier.clone()),
                    );
                    state.pkce_verified = state
                        .expected_pkce_challenge
                        .as_deref()
                        .is_some_and(|expected| expected == challenge.as_str());

                    let fault = state.token_fault;
                    let nonce = state
                        .nonce
                        .clone()
                        .ok_or_else(|| io::Error::other("missing nonce"))?;
                    let issuer = if fault == TokenFault::Issuer {
                        "https://attacker.example"
                    } else {
                        ISSUER
                    };
                    let audience = if fault == TokenFault::Audience {
                        "other-client"
                    } else {
                        CLIENT_ID
                    };
                    let token_nonce = if fault == TokenFault::Nonce {
                        "wrong-nonce"
                    } else {
                        &nonce
                    };
                    let signing_secret = if fault == TokenFault::Signature {
                        "different-signing-secret-with-entropy"
                    } else {
                        CLIENT_SECRET
                    };
                    let issue_time = Utc::now();
                    let expiration = if fault == TokenFault::Expired {
                        issue_time - Duration::minutes(1)
                    } else {
                        issue_time + Duration::minutes(5)
                    };
                    let access_token = AccessToken::new("transient-access-token".to_string());
                    let claims = CoreIdTokenClaims::new(
                        IssuerUrl::new(issuer.to_string()).expect("issuer"),
                        vec![Audience::new(audience.to_string())],
                        expiration,
                        issue_time - Duration::seconds(1),
                        StandardClaims::new(SubjectIdentifier::new(SUBJECT.to_string())),
                        EmptyAdditionalClaims {},
                    )
                    .set_nonce(Some(Nonce::new(token_nonce.to_string())));
                    let id_token = CoreIdToken::new(
                        claims,
                        &CoreHmacKey::new(signing_secret.as_bytes()),
                        CoreJwsSigningAlgorithm::HmacSha256,
                        Some(&access_token),
                        None,
                    )
                    .map_err(|_| io::Error::other("token signing failed"))?;
                    Ok(json_response(json!({
                        "access_token": access_token.secret(),
                        "token_type": "Bearer",
                        "expires_in": 300,
                        "id_token": id_token.to_string()
                    })))
                }
                "/userinfo" => {
                    let state = issuer_state
                        .lock()
                        .map_err(|_| io::Error::other("issuer state unavailable"))?;
                    Ok(json_response(json!({
                        "sub": SUBJECT,
                        "lachesi_groups": state.groups
                    })))
                }
                _ => Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .expect("not found response")),
            }
        }
    }

    fn authenticator(
        issuer_state: Arc<Mutex<TestIssuerState>>,
    ) -> OidcAuthenticator<impl SyncHttpClient> {
        authenticator_with_config(issuer_state, config())
    }

    fn authenticator_with_config(
        issuer_state: Arc<Mutex<TestIssuerState>>,
        config: OidcPublicConfig,
    ) -> OidcAuthenticator<impl SyncHttpClient> {
        OidcAuthenticator::discover_with_http_client(
            config,
            OidcClientSecret::new(CLIENT_SECRET.to_string()).expect("client secret"),
            mapping(),
            test_http_client(issuer_state),
        )
        .expect("discover test issuer")
    }

    fn start_login<C: SyncHttpClient>(
        authenticator: &OidcAuthenticator<C>,
        issuer_state: &Arc<Mutex<TestIssuerState>>,
        now: u64,
    ) -> OidcLoginStart {
        let login = authenticator.begin_login_at(now).expect("begin login");
        let url =
            openidconnect::url::Url::parse(&login.authorization_url).expect("authorization URL");
        let params = url.query_pairs().into_owned().collect::<HashMap<_, _>>();
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert!(params.get("state").is_some_and(|value| !value.is_empty()));
        let mut state = issuer_state.lock().expect("issuer state");
        state.nonce = params.get("nonce").cloned();
        state.expected_pkce_challenge = params.get("code_challenge").cloned();
        login
    }

    fn callback_state(login: &OidcLoginStart) -> String {
        openidconnect::url::Url::parse(&login.authorization_url)
            .expect("authorization URL")
            .query_pairs()
            .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
            .expect("state")
    }

    #[test]
    fn standards_compliant_code_flow_uses_pkce_and_creates_bounded_session() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!(["engineering"]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let login = start_login(&authenticator, &issuer_state, 1_000);
        assert!(!login.authorization_url.contains(CLIENT_SECRET));
        let session = authenticator
            .complete_login_at(
                "authorization-code".to_string(),
                callback_state(&login),
                2_000,
            )
            .expect("complete login");
        assert!(issuer_state.lock().expect("issuer state").pkce_verified);
        assert_eq!(session.expires_at_ms, 602_000);
        assert!(!format!("{session:?}").contains(session.handle.expose_secret()));

        let actor = authenticator
            .resolve_session_at(&session.handle, 601_999)
            .expect("resolve session");
        assert_eq!(actor.organization_id(), "organization-acme");
        assert_eq!(actor.team_ids(), &["team-engineering".to_string()]);
        assert_eq!(actor.role(), TeamRole::Member);
        assert!(actor.id().starts_with("user:oidc:"));

        assert_eq!(
            authenticator.resolve_session_at(&session.handle, 602_000),
            Err(OidcAuthenticationError::SessionExpired)
        );
    }

    #[test]
    fn state_is_one_time_and_invalid_state_never_reaches_token_endpoint() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!(["engineering"]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let login = start_login(&authenticator, &issuer_state, 1_000);
        assert_eq!(
            authenticator.complete_login_at(
                "authorization-code".to_string(),
                "wrong-state".to_string(),
                2_000,
            ),
            Err(OidcAuthenticationError::InvalidState)
        );
        let state = callback_state(&login);
        let wrong_binding = OidcLoginBinding::from_secret(new_opaque_secret().expect("secret"))
            .expect("login binding");
        assert_eq!(
            authenticator.complete_login_at_with_binding(
                "authorization-code".to_string(),
                state.clone(),
                Some(&wrong_binding),
                2_000,
            ),
            Err(OidcAuthenticationError::InvalidState)
        );
        authenticator
            .complete_login_at_with_binding(
                "authorization-code".to_string(),
                state.clone(),
                Some(&login.browser_binding),
                2_000,
            )
            .expect("valid callback");
        assert_eq!(
            authenticator.complete_login_at("authorization-code".to_string(), state, 2_000),
            Err(OidcAuthenticationError::InvalidState)
        );
    }

    #[test]
    fn expired_login_is_consumed_before_token_exchange() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!([]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let login = start_login(&authenticator, &issuer_state, 1_000);
        let state = callback_state(&login);
        assert_eq!(
            authenticator.complete_login_at(
                "authorization-code".to_string(),
                state.clone(),
                1_000 + LOGIN_ATTEMPT_TTL_MS,
            ),
            Err(OidcAuthenticationError::LoginExpired)
        );
        assert_eq!(
            authenticator.complete_login_at("authorization-code".to_string(), state, 2_000),
            Err(OidcAuthenticationError::InvalidState)
        );
    }

    #[test]
    fn issuer_audience_signature_nonce_and_expiry_fail_closed() {
        for fault in [
            TokenFault::Issuer,
            TokenFault::Audience,
            TokenFault::Signature,
            TokenFault::Nonce,
            TokenFault::Expired,
        ] {
            let issuer_state = Arc::new(Mutex::new(TestIssuerState {
                token_fault: fault,
                groups: json!([]),
                ..TestIssuerState::default()
            }));
            let authenticator = authenticator(issuer_state.clone());
            let login = start_login(&authenticator, &issuer_state, 1_000);
            assert_eq!(
                authenticator.complete_login_at(
                    "authorization-code".to_string(),
                    callback_state(&login),
                    2_000,
                ),
                Err(OidcAuthenticationError::InvalidIdToken),
                "fault {fault:?}"
            );
        }
    }

    #[test]
    fn group_mapping_rejects_unmapped_users_and_ambiguity() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!(["unmapped"]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let login = start_login(&authenticator, &issuer_state, 1_000);
        assert_eq!(
            authenticator.complete_login_at(
                "authorization-code".to_string(),
                callback_state(&login),
                2_000,
            ),
            Err(OidcAuthenticationError::Unauthorized)
        );

        issuer_state.lock().expect("issuer state").groups =
            json!(["engineering", "security-admins"]);
        let login = start_login(&authenticator, &issuer_state, 3_000);
        assert_eq!(
            authenticator.complete_login_at(
                "authorization-code".to_string(),
                callback_state(&login),
                4_000,
            ),
            Err(OidcAuthenticationError::AmbiguousRoleMapping)
        );
    }

    #[test]
    fn malformed_group_claim_is_rejected() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!("engineering"),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let login = start_login(&authenticator, &issuer_state, 1_000);
        assert_eq!(
            authenticator.complete_login_at(
                "authorization-code".to_string(),
                callback_state(&login),
                2_000,
            ),
            Err(OidcAuthenticationError::InvalidGroupClaim)
        );
    }

    #[test]
    fn logout_invalidates_only_the_selected_session() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!(["engineering"]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let first_login = start_login(&authenticator, &issuer_state, 1_000);
        let first = authenticator
            .complete_login_at(
                "first-code".to_string(),
                callback_state(&first_login),
                2_000,
            )
            .expect("first session");
        let second_login = start_login(&authenticator, &issuer_state, 3_000);
        let second = authenticator
            .complete_login_at(
                "second-code".to_string(),
                callback_state(&second_login),
                4_000,
            )
            .expect("second session");

        authenticator
            .logout_at(&first.handle, 4_000)
            .expect("logout");
        assert_eq!(
            authenticator.resolve_session_at(&first.handle, 4_001),
            Err(OidcAuthenticationError::InvalidSession)
        );
        assert!(authenticator
            .resolve_session_at(&second.handle, 4_001)
            .is_ok());
    }

    #[test]
    fn pending_login_and_session_capacity_are_enforced() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!(["engineering"]),
            ..TestIssuerState::default()
        }));
        let mut bounded_config = config();
        bounded_config.max_pending_logins = 1;
        bounded_config.max_sessions = 1;
        let authenticator = authenticator_with_config(issuer_state.clone(), bounded_config);

        let first_login = start_login(&authenticator, &issuer_state, 1_000);
        assert_eq!(
            authenticator.begin_login_at(1_001),
            Err(OidcAuthenticationError::CapacityExceeded)
        );
        let first = authenticator
            .complete_login_at(
                "first-code".to_string(),
                callback_state(&first_login),
                2_000,
            )
            .expect("first session");

        let second_login = start_login(&authenticator, &issuer_state, 3_000);
        assert_eq!(
            authenticator.complete_login_at(
                "second-code".to_string(),
                callback_state(&second_login),
                4_000,
            ),
            Err(OidcAuthenticationError::CapacityExceeded)
        );
        assert!(authenticator
            .resolve_session_at(&first.handle, 4_001)
            .is_ok());
    }

    #[test]
    fn secrets_and_tokens_are_redacted_from_debug_and_errors() {
        let client_secret =
            OidcClientSecret::new(CLIENT_SECRET.to_string()).expect("client secret");
        assert_eq!(format!("{client_secret:?}"), "OidcClientSecret([REDACTED])");
        let session_secret = new_opaque_secret().expect("secret");
        let handle =
            OidcSessionHandle::from_secret(session_secret.clone()).expect("session handle");
        assert_eq!(format!("{handle:?}"), "OidcSessionHandle([REDACTED])");
        assert_eq!(handle.expose_secret(), session_secret);
        assert_eq!(
            OidcSessionHandle::from_secret(String::new()),
            Err(OidcAuthenticationError::InvalidSession)
        );
        let login = OidcLoginStart {
            authorization_url: "https://issuer.example/authorize?state=state-secret".to_string(),
            browser_binding: OidcLoginBinding::from_secret(new_opaque_secret().expect("secret"))
                .expect("login binding"),
            expires_at_ms: 1_000,
        };
        assert!(!format!("{login:?}").contains("state-secret"));
        for error in [
            OidcAuthenticationError::TokenExchangeFailed,
            OidcAuthenticationError::InvalidIdToken,
            OidcAuthenticationError::UserInfoFailed,
        ] {
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains(CLIENT_SECRET));
            assert!(!rendered.contains("transient-access-token"));
        }
    }

    #[test]
    fn configuration_rejects_insecure_endpoints_and_privileged_defaults() {
        let mut invalid = config();
        invalid.issuer_url = "http://issuer.example".to_string();
        assert_eq!(
            invalid.validate(),
            Err(OidcAuthenticationError::InvalidConfiguration(
                "issuerUrl.scheme"
            ))
        );
        invalid = config();
        invalid.redirect_url = "http://team.example/callback".to_string();
        assert_eq!(
            invalid.validate(),
            Err(OidcAuthenticationError::InvalidConfiguration(
                "redirectUrl.scheme"
            ))
        );
        invalid = config();
        invalid.issuer_url = "https://issuer.example?tenant=acme".to_string();
        assert_eq!(
            invalid.validate(),
            Err(OidcAuthenticationError::InvalidConfiguration(
                "issuerUrl.shape"
            ))
        );
        invalid = config();
        invalid.redirect_url = "https://team.example/callback#fragment".to_string();
        assert_eq!(
            invalid.validate(),
            Err(OidcAuthenticationError::InvalidConfiguration(
                "redirectUrl.fragment"
            ))
        );
        let mut invalid_mapping = mapping();
        invalid_mapping.groups[0].role = TeamRole::ServiceAccount;
        assert_eq!(
            invalid_mapping.validate(),
            Err(OidcAuthenticationError::InvalidConfiguration(
                "groupMappings.role"
            ))
        );
    }

    #[test]
    fn discovery_rejects_insecure_provider_endpoints() {
        for (endpoint, field) in [
            ("authorization", "providerMetadata.authorizationEndpoint"),
            ("token", "providerMetadata.tokenEndpoint"),
            ("userinfo", "providerMetadata.userinfoEndpoint"),
            ("jwks", "providerMetadata.jwksUri"),
        ] {
            let issuer_state = Arc::new(Mutex::new(TestIssuerState {
                insecure_endpoint: Some(endpoint),
                ..TestIssuerState::default()
            }));
            let result = OidcAuthenticator::discover_with_http_client(
                config(),
                OidcClientSecret::new(CLIENT_SECRET.to_string()).expect("client secret"),
                mapping(),
                test_http_client(issuer_state),
            );
            let expected = if endpoint == "jwks" {
                OidcAuthenticationError::DiscoveryFailed
            } else {
                OidcAuthenticationError::InvalidConfiguration(field)
            };
            assert_eq!(result.err(), Some(expected), "endpoint {endpoint}");
        }
    }

    #[test]
    fn discovery_selects_client_secret_post_and_rejects_unsupported_authentication() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            token_auth_method: Some("client_secret_post"),
            groups: json!(["engineering"]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        let login = start_login(&authenticator, &issuer_state, 1_000);
        authenticator
            .complete_login_at(
                "authorization-code".to_string(),
                callback_state(&login),
                2_000,
            )
            .expect("post authentication succeeds");
        assert!(
            issuer_state
                .lock()
                .expect("issuer state")
                .token_request_used_post
        );

        let unsupported_state = Arc::new(Mutex::new(TestIssuerState {
            token_auth_method: Some("private_key_jwt"),
            ..TestIssuerState::default()
        }));
        assert_eq!(
            OidcAuthenticator::discover_with_http_client(
                config(),
                OidcClientSecret::new(CLIENT_SECRET.to_string()).expect("client secret"),
                mapping(),
                test_http_client(unsupported_state),
            )
            .err(),
            Some(OidcAuthenticationError::InvalidConfiguration(
                "providerMetadata.tokenEndpointAuthMethod"
            ))
        );
    }

    #[test]
    fn callback_refreshes_discovery_and_jwks_before_token_validation() {
        let issuer_state = Arc::new(Mutex::new(TestIssuerState {
            groups: json!(["engineering"]),
            ..TestIssuerState::default()
        }));
        let authenticator = authenticator(issuer_state.clone());
        assert_eq!(
            issuer_state.lock().expect("issuer state").discovery_count,
            1
        );
        let login = start_login(&authenticator, &issuer_state, 1_000);
        authenticator
            .complete_login_at(
                "authorization-code".to_string(),
                callback_state(&login),
                2_000,
            )
            .expect("complete login after refresh");
        assert_eq!(
            issuer_state.lock().expect("issuer state").discovery_count,
            2
        );
    }
}
