//! Retained human PR approval: native identity, explicit consent, uncertain-write recovery.
use crate::services::bitbucket::workspace_prs::{DetailParams, RepositoryIdentity};
use crate::services::review_scope::Scope;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) fn invalid() -> String {
    "The provider approval could not be verified. Refresh its preview or inspect provider status."
        .into()
}
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Actor {
    pub account_id: String,
    pub display_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Proof {
    pub request: DetailParams,
    pub repository_id: String,
    pub actor: Actor,
    pub title: String,
    pub head_sha: String,
    pub destination_sha: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub source_repository: Option<RepositoryIdentity>,
    pub state: String,
    pub draft: bool,
    pub already_approved: bool,
}
impl Proof {
    pub fn validate(&self) -> Result<(), String> {
        self.request.validate()?;
        if !identifier(&self.repository_id)
            || !identifier(&self.actor.account_id)
            || self.actor.display_name.len() > 256
            || self.actor.display_name.chars().any(char::is_control)
            || self.title.len() > 4096
            || self.title.chars().any(char::is_control)
            || !identifier(&self.source_branch)
            || !identifier(&self.destination_branch)
            || !matches!(
                self.state.as_str(),
                "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED"
            )
            || self.head_sha.len() != self.destination_sha.len()
        {
            return Err(invalid());
        }
        for sha in [&self.head_sha, &self.destination_sha] {
            if crate::services::provider_snapshot::full_sha(sha)? != *sha {
                return Err(invalid());
            }
        }
        if let Some(source) = &self.source_repository {
            source.validate()?;
            if source.provider != self.request.repository.provider {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub fn scope(&self) -> Scope {
        Scope {
            provider: match self.request.repository.provider {
                crate::config::ReviewProvider::Github => {
                    crate::services::review::ReviewProvider::Github
                }
                crate::config::ReviewProvider::Bitbucket => {
                    crate::services::review::ReviewProvider::Bitbucket
                }
            },
            workspace: self.request.repository.workspace.clone(),
            repo: self.request.repository.repo.clone(),
            pr_id: self.request.pr_id,
        }
    }
    pub fn identity(&self) -> String {
        // Native immutable IDs prevent repository spelling/title/cache-handle
        // changes from bypassing an unresolved POST for the same actual actor.
        let bytes = serde_json::to_vec(&(
            "norn-pr-approval-v1",
            self.request.repository.provider,
            &self.repository_id,
            self.request.pr_id,
            &self.actor.account_id,
            &self.head_sha,
            &self.destination_sha,
        ))
        .expect("serializable approval identity");
        format!("{:x}", Sha256::digest(bytes))
    }
    pub fn same_preview(&self, other: &Self) -> bool {
        let mut current = other.clone();
        current.already_approved = self.already_approved;
        *self == current
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum State {
    Writing,
    Approved,
    Rejected,
    Uncertain,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Entry {
    pub format_version: u32,
    pub proof: Proof,
    pub state: State,
    pub remote_review_id: Option<String>,
    pub updated_at: u64,
    pub attempt: u32,
}
impl Entry {
    pub fn validate(&self) -> Result<(), String> {
        self.proof.validate()?;
        if self.format_version != 1
            || self.attempt == 0
            || self
                .remote_review_id
                .as_deref()
                .is_some_and(|id| !identifier(id))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
