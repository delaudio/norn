//! Read-only declared profile catalog. Policy and target ownership stay in Rust.
use crate::protocol::{ProtocolError, ProtocolErrorCode, TargetIdentity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Params {
    #[serde(deserialize_with = "strict_target")]
    pub target: TargetIdentity,
}
fn strict_target<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<TargetIdentity, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    let object = value
        .as_object()
        .ok_or_else(|| serde::de::Error::custom("Invalid review target."))?;
    let fields: &[&str] = match object.get("kind").and_then(|v| v.as_str()) {
        Some("provider") => &["kind", "provider", "workspace", "repo", "prId", "runId"],
        Some("local") => &["kind", "localSnapshotSha256"],
        _ => return Err(serde::de::Error::custom("Invalid review target.")),
    };
    if object.keys().any(|key| !fields.contains(&key.as_str()))
        || ["prId", "runId"]
            .iter()
            .any(|key| object.get(*key).is_some_and(|v| v.is_null()))
    {
        return Err(serde::de::Error::custom("Invalid review target fields."));
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}
impl Params {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let invalid = || {
            ProtocolError::new(
                ProtocolErrorCode::InvalidParams,
                "Invalid review profile target.",
            )
        };
        match &self.target {
            TargetIdentity::Provider {
                provider,
                workspace,
                repo,
                pr_id,
                run_id,
            } => {
                let identity = super::bitbucket::workspace_prs::RepositoryIdentity {
                    provider: match provider {
                        crate::protocol::ProviderKind::Github => {
                            crate::config::ReviewProvider::Github
                        }
                        crate::protocol::ProviderKind::Bitbucket => {
                            crate::config::ReviewProvider::Bitbucket
                        }
                    },
                    workspace: workspace.clone(),
                    repo: repo.clone(),
                };
                identity.validate().map_err(|_| invalid())?;
                if pr_id
                    .is_some_and(|id| id == 0 || id >= crate::local_review::LOCAL_REVIEW_ID_FLAG)
                    || run_id.as_ref().is_some_and(|id| {
                        pr_id.is_none()
                            || id.is_empty()
                            || id.len() > 256
                            || id.trim() != id
                            || id.chars().any(char::is_control)
                    })
                {
                    return Err(invalid());
                }
            }
            TargetIdentity::Local {
                local_snapshot_sha256,
            } => {
                if local_snapshot_sha256.len() != 64
                    || !local_snapshot_sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub target: TargetIdentity,
    pub profiles: Vec<String>,
    pub default_profile: Option<String>,
    pub source: Source,
    pub notice: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Policy,
    Base,
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 512
        && name.chars().count() <= 128
        && name.trim() == name
        && !name.chars().any(char::is_control)
}
impl Catalog {
    pub fn from_policy(
        target: TargetIdentity,
        policy: crate::repo_config::RepoReviewConfigLoadResult,
    ) -> Result<Self, ProtocolError> {
        if !policy.errors.is_empty() {
            return Err(ProtocolError::new(
                ProtocolErrorCode::InvalidParams,
                "This repository's review policy is invalid. Correct it before choosing a profile.",
            ));
        }
        let profiles: Vec<String> = policy
            .config
            .as_ref()
            .map(|config| config.profiles.keys().cloned().collect())
            .unwrap_or_default();
        if profiles.len() > 100
            || profiles.iter().any(|name| !valid_name(name))
            || policy
                .selected_profile
                .as_ref()
                .is_some_and(|name| !valid_name(name) || !profiles.contains(name))
        {
            return Err(ProtocolError::new(
                ProtocolErrorCode::InvalidParams,
                "The review profile catalog exceeds supported bounds or contains invalid names.",
            ));
        }
        Ok(Self {
            target,
            profiles,
            default_profile: policy.selected_profile,
            source: if policy.config.is_some() {
                Source::Policy
            } else {
                Source::Base
            },
            notice: (!policy.warnings.is_empty()).then(|| {
                "The review policy has validation warnings. Check the policy before reviewing."
                    .into()
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn target() -> TargetIdentity {
        TargetIdentity::Provider {
            provider: crate::protocol::ProviderKind::Github,
            workspace: "owned".into(),
            repo: "same".into(),
            pr_id: Some(7),
            run_id: None,
        }
    }
    #[test]
    fn strict_catalog_target_rejects_unknown_null_and_unsafe_identity() {
        let value = serde_json::to_value(target()).unwrap();
        for key in ["path", "profile", "secret"] {
            let mut invalid = value.clone();
            invalid[key] = json!("invalid");
            assert!(serde_json::from_value::<Params>(json!({"target":invalid})).is_err());
        }
        for key in ["prId", "runId"] {
            let mut invalid = value.clone();
            invalid[key] = serde_json::Value::Null;
            assert!(serde_json::from_value::<Params>(json!({"target":invalid})).is_err());
        }
        for invalid in [
            json!({"kind":"provider","provider":"github","workspace":"../owned","repo":"same"}),
            json!({"kind":"provider","provider":"github","workspace":"owned","repo":"same","prId":0}),
            json!({"kind":"provider","provider":"github","workspace":"owned","repo":"same","runId":"saved"}),
            json!({"kind":"local","localSnapshotSha256":"A".repeat(64)}),
        ] {
            let params: Params = serde_json::from_value(json!({"target":invalid})).unwrap();
            assert!(params.validate().is_err());
        }
    }
    #[test]
    fn native_policy_catalog_default_named_and_native_local_semantics_have_no_raw_configuration() {
        let owned = tempfile::tempdir().unwrap();
        std::fs::write(owned.path().join(".norn.yaml"), "version: 0.1\nreview:\n  profile: security\nprofiles:\n  default: {}\n  security: {}\n").unwrap();
        let policy = crate::repo_config::load_from_repo_path(owned.path()).unwrap();
        let catalog = Catalog::from_policy(target(), policy).unwrap();
        assert_eq!(catalog.profiles, ["default", "security"]);
        assert_eq!(catalog.default_profile.as_deref(), Some("security"));
        let json = serde_json::to_value(catalog).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 5);
        assert!(json.get("config").is_none());
        std::fs::write(
            owned.path().join(".norn.local.yaml"),
            "version: 0.1\nreview:\n  profile: default\n",
        )
        .unwrap();
        let policy = crate::repo_config::load_from_repo_path(owned.path()).unwrap();
        assert_eq!(
            Catalog::from_policy(target(), policy)
                .unwrap()
                .default_profile
                .as_deref(),
            Some("security"),
            "Without organization policy, use the native repository loader's existing semantics."
        );
    }
    #[test]
    fn absent_missing_default_and_invalid_policy_are_honest() {
        let owned = tempfile::tempdir().unwrap();
        let empty = Catalog::from_policy(
            target(),
            crate::repo_config::load_from_repo_path(owned.path()).unwrap(),
        )
        .unwrap();
        assert_eq!(empty.source, Source::Base);
        assert!(empty.profiles.is_empty());
        assert!(empty.default_profile.is_none());
        std::fs::write(
            owned.path().join(".norn.yaml"),
            "version: 0.1\nreview:\n  profile: missing\nprofiles:\n  security: {}\n",
        )
        .unwrap();
        let missing = Catalog::from_policy(
            target(),
            crate::repo_config::load_from_repo_path(owned.path()).unwrap(),
        )
        .unwrap();
        assert!(missing.default_profile.is_none());
        assert!(missing.notice.is_some());
        std::fs::write(
            owned.path().join(".norn.yaml"),
            "version: 9.0\nprofiles: {}\n",
        )
        .unwrap();
        assert!(Catalog::from_policy(
            target(),
            crate::repo_config::load_from_repo_path(owned.path()).unwrap()
        )
        .is_err());
    }
    #[test]
    fn names_and_collection_are_bounded_without_truncating() {
        for name in ["", " space", "line\ncontrol", &"a".repeat(129)] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("security-世界"));
        let owned = tempfile::tempdir().unwrap();
        let mut policy = crate::repo_config::load_from_repo_path(owned.path()).unwrap();
        let mut config = crate::repo_config::RepoReviewConfig::default();
        for i in 0..101 {
            config.profiles.insert(format!("p{i}"), Default::default());
        }
        policy.config = Some(config);
        assert!(Catalog::from_policy(target(), policy).is_err());
    }
}
