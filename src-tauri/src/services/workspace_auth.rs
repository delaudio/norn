//! Status-only credential readback and explicit OS credential writes.
use crate::credentials::{self, CredentialProvider};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    Github,
    Bitbucket,
    Jira,
    Notion,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(pub String);
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthSetParams {
    pub provider: AuthProvider,
    pub username: Option<String>,
    pub token: Secret,
}
impl AuthSetParams {
    pub fn validate(&self) -> Result<(), String> {
        let value = self.token.0.trim();
        if value.is_empty()
            || self.token.0.chars().count() > 8192
            || self.token.0.chars().any(char::is_control)
        {
            return Err(
                "A nonempty credential up to 8192 printable characters is required.".to_string(),
            );
        }
        if self
            .username
            .as_ref()
            .is_some_and(|value| value.chars().count() > 256 || value.chars().any(char::is_control))
            || (self.provider == AuthProvider::Bitbucket
                && self
                    .username
                    .as_ref()
                    .is_none_or(|value| value.trim().is_empty()))
        {
            return Err(
                "A Bitbucket username up to 256 printable characters is required.".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthProviderParams {
    pub provider: AuthProvider,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub provider: AuthProvider,
    pub available: bool,
    pub source: &'static str,
}

trait CredentialAccess {
    fn status(&self, provider: AuthProvider) -> AuthStatus;
    fn store(&self, params: &AuthSetParams) -> Result<(), String>;
    fn clear(&self, provider: AuthProvider) -> Result<(), String>;
}
struct NativeCredentials;
impl CredentialAccess for NativeCredentials {
    fn status(&self, provider: AuthProvider) -> AuthStatus {
        let native = match provider {
            AuthProvider::Github => {
                Some(credentials::credential_status(CredentialProvider::Github))
            }
            AuthProvider::Bitbucket => Some(credentials::credential_status(
                CredentialProvider::Bitbucket,
            )),
            _ => None,
        };
        if let Some(native) = native {
            let source = match native.source {
                credentials::CredentialSource::Keychain => "keychain",
                credentials::CredentialSource::Environment => "environment",
                credentials::CredentialSource::None => "none",
            };
            return AuthStatus {
                provider,
                available: native.available,
                source,
            };
        }
        let available = match provider {
            AuthProvider::Jira => credentials::has_jira(),
            AuthProvider::Notion => credentials::has_notion(),
            _ => false,
        };
        AuthStatus {
            provider,
            available,
            source: if available { "keychain" } else { "none" },
        }
    }
    fn store(&self, params: &AuthSetParams) -> Result<(), String> {
        match params.provider {
            AuthProvider::Github => credentials::store_provider_credential(
                CredentialProvider::Github,
                None,
                &params.token.0,
            ),
            AuthProvider::Bitbucket => credentials::store_provider_credential(
                CredentialProvider::Bitbucket,
                params.username.as_deref(),
                &params.token.0,
            ),
            AuthProvider::Jira => credentials::store_jira_token(params.token.0.trim()),
            AuthProvider::Notion => credentials::store_notion_token(params.token.0.trim()),
        }
        .map_err(|_| {
            "The OS credential store could not save this credential. No plaintext fallback is used."
                .to_string()
        })
    }
    fn clear(&self, provider: AuthProvider) -> Result<(), String> {
        match provider {
            AuthProvider::Github => {
                credentials::clear_provider_credential(CredentialProvider::Github)
            }
            AuthProvider::Bitbucket => {
                credentials::clear_provider_credential(CredentialProvider::Bitbucket)
            }
            AuthProvider::Jira => credentials::clear_jira_token(),
            AuthProvider::Notion => credentials::clear_notion_token(),
        }
        .map_err(|_| "The OS credential store could not remove this credential.".to_string())
    }
}

pub(crate) fn statuses() -> Vec<AuthStatus> {
    [
        AuthProvider::Github,
        AuthProvider::Bitbucket,
        AuthProvider::Jira,
        AuthProvider::Notion,
    ]
    .into_iter()
    .map(|provider| NativeCredentials.status(provider))
    .collect()
}
pub(crate) fn set(params: AuthSetParams) -> Result<(), String> {
    set_with(&NativeCredentials, &params)
}
fn set_with(access: &impl CredentialAccess, params: &AuthSetParams) -> Result<(), String> {
    params.validate()?;
    access.store(params)
}
pub(crate) fn clear(provider: AuthProvider) -> Result<(), String> {
    NativeCredentials.clear(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct FakeCredentials(RefCell<Option<AuthSetParams>>);
    impl CredentialAccess for FakeCredentials {
        fn status(&self, provider: AuthProvider) -> AuthStatus {
            AuthStatus {
                provider,
                available: self.0.borrow().is_some(),
                source: "keychain",
            }
        }
        fn store(&self, params: &AuthSetParams) -> Result<(), String> {
            self.0.replace(Some(params.clone()));
            Ok(())
        }
        fn clear(&self, _: AuthProvider) -> Result<(), String> {
            self.0.replace(None);
            Ok(())
        }
    }
    #[test]
    fn credential_writes_validate_and_status_never_returns_secret_values() {
        let fake = FakeCredentials(RefCell::new(None));
        let mut params = AuthSetParams {
            provider: AuthProvider::Bitbucket,
            username: Some("fixture-user".to_string()),
            token: Secret("fixture-sensitive-value".to_string()),
        };
        set_with(&fake, &params).unwrap();
        assert!(!format!("{params:?}").contains("fixture-sensitive-value"));
        let status = serde_json::to_string(&fake.status(params.provider)).unwrap();
        assert!(!status.contains("fixture-user") && !status.contains("fixture-sensitive-value"));
        fake.clear(params.provider).unwrap();
        assert!(!fake.status(params.provider).available);
        params.username = None;
        assert!(set_with(&fake, &params).is_err());
        params.provider = AuthProvider::Github;
        params.token.0 = "\n".to_string();
        assert!(set_with(&fake, &params).is_err());
    }
}
