//! Explicit, provider-qualified local Git operations. Requests carry no root,
//! executable, remote URL, shell command or process identifier.
mod git;
mod manager;
mod state;
#[cfg(test)]
mod tests;
pub(crate) use manager::Manager;

use crate::services::bitbucket::workspace_prs::RepositoryIdentity;
use crate::services::repository_reader::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BranchKind {
    Local,
    Remote,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Branch {
    pub reference: String,
    pub name: String,
    pub kind: BranchKind,
    pub oid: String,
    pub current: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Action {
    Checkout {
        reference: String,
        #[serde(rename = "branchKind")]
        branch_kind: BranchKind,
    },
    Fetch {},
    Pull {},
    Stash {},
}
impl Action {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::Checkout {
            reference,
            branch_kind,
        } = self
        {
            let prefix = match branch_kind {
                BranchKind::Local => "refs/heads/",
                BranchKind::Remote => "refs/remotes/",
            };
            if !reference.starts_with(prefix)
                || !safe_reference(reference)
                || reference
                    .strip_prefix("refs/heads/")
                    .is_some_and(|name| name.starts_with('-'))
                || reference
                    .strip_prefix("refs/remotes/origin/")
                    .is_some_and(|name| name.starts_with('-'))
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListParams {
    pub repository: RepositoryIdentity,
    pub offset: usize,
    pub limit: usize,
    #[serde(deserialize_with = "required_nullable")]
    pub version: Option<String>,
}
impl ListParams {
    pub fn validate(&self) -> Result<(), String> {
        self.repository.validate()?;
        if self.limit == 0
            || self.limit > 100
            || self.offset > 10_000
            || (self.offset > 0 && self.version.is_none())
            || self.version.as_ref().is_some_and(|v| !valid_hash(v))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewParams {
    pub context: Context,
    pub action: Action,
}
impl PreviewParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        self.action.validate()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyParams {
    pub context: Context,
    pub action: Action,
    pub token: String,
    pub confirm: bool,
}
impl ApplyParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        self.action.validate()?;
        if !self.confirm || !valid_hash(&self.token) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobParams {
    pub context: Context,
    pub operation_id: String,
}
impl JobParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        if !valid_hash(&self.operation_id) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckoutState {
    Ready,
    Unborn,
    Missing,
    Invalid,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    pub repository: RepositoryIdentity,
    pub context: Option<Context>,
    pub state: CheckoutState,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub upstream: Option<String>,
    pub stash_sha: Option<String>,
    pub dirty: bool,
    pub filtered: bool,
    pub conflicted: bool,
    pub pending: bool,
    pub version: Option<String>,
    pub message: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub request: ListParams,
    pub checkout: Checkout,
    pub branches: Vec<Branch>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
fn required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
fn invalid() -> String {
    "The repository operation could not be verified. Refresh its state explicitly.".into()
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}
fn safe_reference(value: &str) -> bool {
    value.len() <= 1024
        && !value.is_empty()
        && !value.contains(['\0', '~', '^', ':', '?', '*', '[', '\\'])
        && !value.contains("..")
        && !value.contains("@{")
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && !value.ends_with(['/', '.'])
        && value
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    pub request: PreviewParams,
    pub token: String,
    pub expires_in_ms: u32,
    pub before: Checkout,
    pub summary: String,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Starting,
    Checking,
    Running,
    Inspecting,
    Completed,
    Failed,
    Canceled,
    Uncertain,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub operation_id: String,
    pub context: Context,
    pub action: Action,
    pub state: OperationState,
    pub before: Checkout,
    pub after: Option<Checkout>,
    pub exit_code: Option<i32>,
    pub state_changed: Option<bool>,
    pub recovery_required: bool,
    pub message: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub request: ApplyParams,
    pub operation: Operation,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub request: JobParams,
    pub operation: Operation,
    pub cancel_requested: bool,
}
