//! Explicit editor handoff for a qualified configured repository. No request
//! accepts an executable, absolute file path, shell command or process ID.
mod integration;
mod manager;
mod terminal;
#[cfg(test)]
mod tests;

use crate::local_review::{GitRunControl, LocalReviewCancellation};
use crate::services::repository_reader::{context, Context};
pub(crate) use manager::Manager;
pub(crate) fn terminal_child(value: &str) -> Result<i32, String> {
    terminal::run_child(value)
}
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Graphical,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareParams {
    pub context: Context,
    pub path: String,
    pub line: Option<u32>,
    pub mode: Mode,
}
impl PrepareParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        context::validate_path(&self.path)?;
        if self.line.is_some_and(|line| line == 0 || line > 10_000_000) {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenParams {
    pub context: Context,
    pub token: String,
}
impl OpenParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        if !valid_token(&self.token) {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusParams {
    pub context: Context,
    pub launch_id: String,
}
impl StatusParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        if !valid_token(&self.launch_id) {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum State {
    Starting,
    Launched,
    Running,
    Exited,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Editor {
    pub launch_id: String,
    pub context: Context,
    pub path: String,
    pub line: Option<u32>,
    pub mode: Mode,
    pub state: State,
    pub integration: String,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Prepared {
    pub request: PrepareParams,
    pub token: Option<String>,
    pub integration: Option<String>,
    pub expires_in_ms: u32,
    pub message: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Opened {
    pub request: OpenParams,
    pub editor: Editor,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub request: StatusParams,
    pub editor: Editor,
}

#[derive(Clone, PartialEq, Eq)]
struct FileProof {
    canonical: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    inode: u64,
}
fn file(params: &PrepareParams, control: &GitRunControl) -> Result<FileProof, String> {
    params.validate()?;
    let root = context::resolve_context(&params.context, control)?;
    let opened = context::verified_file(&root, &params.path, control)?
        .ok_or_else(|| "This file is missing or deleted. Refresh its inventory.".to_owned())?;
    let metadata = opened.metadata().map_err(|_| invalid())?;
    if !metadata.is_file() {
        return Err("Select a regular repository file.".into());
    }
    let canonical = root
        .join(&params.path)
        .canonicalize()
        .map_err(|_| invalid())?;
    if !canonical.starts_with(&root) {
        return Err(invalid());
    }
    Ok(FileProof {
        canonical,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        #[cfg(unix)]
        inode: {
            use std::os::unix::fs::MetadataExt;
            metadata.ino()
        },
    })
}
fn control(cancel: LocalReviewCancellation) -> GitRunControl {
    GitRunControl::new(Duration::from_secs(20), cancel, "Repository editor")
}
fn valid_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn token() -> Result<String, String> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| "Editor authorization is unavailable.".to_owned())?;
    Ok(hex::encode(bytes))
}
fn invalid() -> String {
    "The selected editor action could not be verified. Refresh the repository.".into()
}

/// Desktop compatibility adapter uses the same qualified native launcher.
#[cfg(feature = "desktop")]
pub(crate) fn open_graphical(
    repository: crate::services::bitbucket::workspace_prs::RepositoryIdentity,
    path: String,
    line: Option<u32>,
) -> Result<(), String> {
    static MANAGER: std::sync::OnceLock<Manager> = std::sync::OnceLock::new();
    let manager = MANAGER.get_or_init(Manager::new);
    let source = context::resolve(&repository, &control(LocalReviewCancellation::new()))?;
    let params = PrepareParams {
        context: Context {
            repository,
            root_id: source.root_id,
        },
        path,
        line,
        mode: Mode::Graphical,
    };
    let prepared = manager.prepare(params.clone())?;
    let token = prepared.token.ok_or_else(|| {
        prepared
            .message
            .unwrap_or_else(|| "Editor integration is unavailable.".into())
    })?;
    let opened = manager.open(OpenParams {
        context: params.context.clone(),
        token,
    })?;
    let deadline = Instant::now() + Duration::from_secs(25);
    let request = StatusParams {
        context: params.context,
        launch_id: opened.editor.launch_id,
    };
    loop {
        let editor = manager.status(request.clone(), false)?.editor;
        match editor.state {
            State::Launched | State::Exited => return Ok(()),
            State::Failed | State::Canceled => {
                return Err(editor
                    .message
                    .unwrap_or_else(|| "The editor could not start.".into()))
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            let _ = manager.status(request, true);
            return Err("The editor launcher has not confirmed startup.".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
