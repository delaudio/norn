//! Qualified immutable local evidence (#358). No caller-provided filesystem path.
mod browser;
mod capture;
mod images;
mod listing;
use super::bitbucket::workspace_prs::RepositoryIdentity;
use crate::local_review::{
    GitRunControl, LocalReviewCancellation, LocalReviewDiffLayerKind as Layer,
};
pub use browser::{BrowserParams, BrowserSnapshotSource};
pub(crate) use capture::{Data, Manager, Ticket};
pub(crate) use images::ImageParams;
pub use listing::ListParams;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) const PATCH_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const METADATA_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const FILE_LIMIT: usize = 6000;
const GIT_METADATA_BYTES: usize = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn invalid() -> String {
    "The local snapshot could not be verified. Refresh the selected local comparison explicitly."
        .into()
}
fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn rank(layer: Layer) -> usize {
    match layer {
        Layer::Committed => 0,
        Layer::Staged => 1,
        Layer::Unstaged => 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Source {
    Configured { repository: RepositoryIdentity },
    Current {},
}
impl Source {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Configured { repository } => repository.validate(),
            Self::Current {} => Ok(()),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    WorkingTree,
    Branch,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Comparison {
    pub source: Source,
    pub mode: Mode,
    pub base_ref: Option<String>,
    pub layers: Vec<Layer>,
}
impl Comparison {
    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;
        if self.layers.is_empty()
            || self.layers.len() > 3
            || self
                .layers
                .windows(2)
                .any(|pair| rank(pair[0]) >= rank(pair[1]))
        {
            return Err(invalid());
        }
        match self.mode {
            Mode::WorkingTree if self.base_ref.is_none() => Ok(()),
            Mode::Branch
                if self.layers == [Layer::Committed]
                    && self.base_ref.as_ref().is_some_and(|base| {
                        !base.is_empty()
                            && base.len() <= 256
                            && !base.starts_with('-')
                            && !base.chars().any(|c| c.is_control() || c.is_whitespace())
                    }) =>
            {
                Ok(())
            }
            _ => Err(invalid()),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureParams {
    pub capture_id: String,
    pub comparison: Comparison,
}
impl CaptureParams {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.capture_id) {
            return Err(invalid());
        }
        self.comparison.validate()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptionsParams {
    pub source: Source,
    pub offset: usize,
    pub limit: usize,
}
impl OptionsParams {
    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;
        if self.offset > 1000 || !(1..=32).contains(&self.limit) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelParams {
    pub source: Source,
    pub capture_id: String,
}
impl CancelParams {
    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;
        if !valid_id(&self.capture_id) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotRef {
    pub source: Source,
    pub repository: RepositoryIdentity,
    pub root_id: String,
    pub snapshot_id: String,
}
impl SnapshotRef {
    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;
        self.repository.validate()?;
        if !valid_id(&self.root_id)
            || !valid_id(&self.snapshot_id)
            || matches!(&self.source, Source::Configured { repository } if repository != &self.repository)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LookupParams {
    pub snapshot: SnapshotRef,
}
impl LookupParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilesParams {
    pub snapshot: SnapshotRef,
    pub offset: usize,
    pub limit: usize,
}
impl FilesParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if self.offset > FILE_LIMIT || !(1..=32).contains(&self.limit) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffParams {
    pub snapshot: SnapshotRef,
    pub layer: Layer,
    pub path: String,
    pub offset: usize,
}
impl DiffParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if !super::provider_snapshot::safe_path(&self.path) || self.offset > PATCH_BYTES {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayerInfo {
    pub kind: Layer,
    pub file_count: usize,
    pub patch_bytes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Info {
    pub comparison: Comparison,
    pub snapshot: SnapshotRef,
    pub branch: String,
    pub head_sha: Option<String>,
    pub base_sha: String,
    pub native_snapshot_sha256: String,
    pub local_review_id: u32,
    pub layers: Vec<LayerInfo>,
    pub file_count: usize,
    pub patch_bytes: usize,
    pub warnings: Vec<String>,
    pub warnings_truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptionsResult {
    pub request: OptionsParams,
    pub repository: RepositoryIdentity,
    pub root_id: String,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub base_refs: Vec<String>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct File {
    pub layer: Layer,
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilesPage {
    pub request: FilesParams,
    pub files: Vec<File>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffPage {
    pub request: DiffParams,
    pub text: String,
    pub total_bytes: usize,
    pub content_sha256: String,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureResult {
    pub request: CaptureParams,
    pub info: Info,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoResult {
    pub request: LookupParams,
    pub info: Info,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecheckParams {
    pub snapshot: SnapshotRef,
    pub capture_id: String,
}
impl RecheckParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()?;
        if !valid_id(&self.capture_id) {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecheckResult {
    pub request: RecheckParams,
    pub info: Info,
    pub unchanged: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelResult {
    pub request: CancelParams,
    pub canceled: bool,
}

pub(crate) struct ResolvedSource {
    pub repository: RepositoryIdentity,
    pub root: PathBuf,
    pub root_id: String,
}
fn root_id(path: &Path) -> Result<String, String> {
    let canonical = path.to_str().ok_or_else(invalid)?;
    let bytes = serde_json::to_vec(&("norn-local-root-v1", canonical)).map_err(|_| invalid())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn git_text(path: &Path, args: &[&str], control: &GitRunControl) -> Result<String, String> {
    let output = if args == ["rev-parse", "--show-toplevel"] {
        crate::local_review::discover_git_root_bounded(path, control)
    } else {
        crate::local_review::run_git_bounded_with_control(
            path,
            args,
            None,
            GIT_METADATA_BYTES,
            "Local source metadata",
            control,
        )
    }
    .map_err(|_| invalid())?;
    if !output.status.success() {
        return Err(invalid());
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim_end_matches(['\r', '\n']).to_string())
        .map_err(|_| invalid())
}
fn exact_remote(remote: &str, repository: &RepositoryIdentity) -> bool {
    crate::local_repo::parse_git_remote(remote).is_ok_and(|(provider, workspace, repo)| {
        provider == repository.provider
            && workspace.eq_ignore_ascii_case(&repository.workspace)
            && repo.eq_ignore_ascii_case(&repository.repo)
    })
}
pub(crate) fn resolve_source(
    source: &Source,
    startup_root: &Path,
    control: &GitRunControl,
) -> Result<ResolvedSource, String> {
    resolve_source_policy(source, startup_root, control, false)
}
/// Repository browsing/editor compatibility accepts an explicitly configured
/// subdirectory, proving its enclosing checkout and matching origin. Captured
/// review sources keep their existing exact-root requirement.
pub(crate) fn resolve_repository_root(
    repository: &RepositoryIdentity,
    control: &GitRunControl,
) -> Result<ResolvedSource, String> {
    resolve_source_policy(
        &Source::Configured {
            repository: repository.clone(),
        },
        Path::new(""),
        control,
        true,
    )
}
fn resolve_source_policy(
    source: &Source,
    startup_root: &Path,
    control: &GitRunControl,
    allow_nested: bool,
) -> Result<ResolvedSource, String> {
    source.validate()?;
    control.check()?;
    let (path, configured) = match source {
        Source::Configured { repository } => {
            let matches: Vec<_> = crate::config::load()
                .repos
                .into_iter()
                .filter(|entry| {
                    entry.provider == repository.provider
                        && entry.workspace == repository.workspace
                        && entry.repo == repository.repo
                })
                .collect();
            if matches.len() != 1 {
                return Err(
                    "Select one explicitly configured local checkout for this provider repository."
                        .into(),
                );
            }
            let path = matches[0]
                .local_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| "This repository has no configured local checkout.".to_string())?;
            (PathBuf::from(path), Some(repository.clone()))
        }
        Source::Current {} => (startup_root.to_path_buf(), None),
    };
    let input = path
        .canonicalize()
        .map_err(|_| "The selected local checkout is unavailable.".to_string())?;
    let root = PathBuf::from(git_text(
        &input,
        &["rev-parse", "--show-toplevel"],
        control,
    )?)
    .canonicalize()
    .map_err(|_| invalid())?;
    if configured.is_some() && root != input && (!allow_nested || !input.starts_with(&root)) {
        return Err(
            "The configured checkout must point to the selected Git repository root.".into(),
        );
    }
    let remote = match git_text(&root, &["remote", "get-url", "origin"], control) {
        Ok(remote) if !remote.is_empty() => remote,
        _ if configured.is_some() => {
            return Err("The configured local checkout requires its matching origin remote.".into())
        }
        _ => {
            // Preserve the shipped current-repository fallback to its first remote.
            // The name comes from bounded Git metadata, never a caller argument.
            control.check()?;
            let remotes = git_text(&root, &["remote"], control)?;
            let name = remotes
                .lines()
                .find(|name| !name.is_empty())
                .filter(|name| {
                    name.len() <= 256
                        && !name.starts_with('-')
                        && !name.chars().any(char::is_control)
                })
                .ok_or_else(|| "Local review requires a GitHub or Bitbucket remote.".to_string())?;
            git_text(&root, &["remote", "get-url", name], control)?
        }
    };
    let repository = if let Some(repository) = configured {
        if !exact_remote(&remote, &repository) {
            return Err(
                "The checkout origin does not match the selected provider repository.".into(),
            );
        }
        repository
    } else {
        let (provider, workspace, repo) = crate::local_repo::parse_git_remote(&remote).map_err(|_| "The current repository must have a GitHub or Bitbucket remote; no provider was guessed.".to_string())?;
        let repository = RepositoryIdentity {
            provider,
            workspace,
            repo,
        };
        repository.validate()?;
        repository
    };
    let root_id = root_id(&root)?;
    Ok(ResolvedSource {
        repository,
        root,
        root_id,
    })
}
fn candidate_refs(output: &str, branch: Option<&str>) -> Result<Vec<String>, String> {
    let mut refs: Vec<String> = output
        .lines()
        .filter(|value| !value.is_empty() && !value.ends_with("/HEAD") && Some(*value) != branch)
        .map(str::to_string)
        .collect();
    refs.sort();
    refs.dedup();
    if refs.len() > 1000
        || refs.iter().any(|value| {
            value.len() > 256
                || value.starts_with('-')
                || value.chars().any(|c| c.is_control() || c.is_whitespace())
        })
    {
        return Err(invalid());
    }
    Ok(refs)
}
pub(crate) fn options(
    request: OptionsParams,
    startup_root: &Path,
    cancellation: LocalReviewCancellation,
) -> Result<OptionsResult, String> {
    request.validate()?;
    let control = GitRunControl::new(TIMEOUT, cancellation, "Local source options");
    let source = resolve_source(&request.source, startup_root, &control)?;
    let output = git_text(
        &source.root,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
        &control,
    )?;
    let branch = git_text(
        &source.root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        &control,
    )
    .ok();
    control.check()?;
    let head_sha = git_text(&source.root, &["rev-parse", "--verify", "HEAD"], &control).ok();
    control.check()?;
    if branch
        .as_ref()
        .is_some_and(|name| name.len() > 256 || name.chars().any(char::is_control))
    {
        return Err(invalid());
    }
    if let Some(head) = &head_sha {
        super::provider_snapshot::full_sha(head)?;
    }
    if head_sha.is_none()
        && (branch.is_none() || output.lines().any(|value| Some(value) == branch.as_deref()))
    {
        return Err(invalid());
    }
    let refs = candidate_refs(&output, branch.as_deref())?;
    if request.offset > refs.len() {
        return Err(invalid());
    }
    let end = (request.offset + request.limit).min(refs.len());
    let total = refs.len();
    let base_refs = refs[request.offset..end].to_vec();
    Ok(OptionsResult {
        request,
        repository: source.repository,
        root_id: source.root_id,
        branch,
        head_sha,
        base_refs,
        total,
        next_offset: (end < total).then_some(end),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn strict_sources_never_accept_path_or_extra_authority() {
        for value in [
            json!({"kind":"current","path":"/forbidden"}),
            json!({"kind":"current","provider":"github"}),
            json!({"kind":"configured","repository":{"provider":"github","workspace":"owned","repo":"same","path":"/forbidden"}}),
        ] {
            assert!(serde_json::from_value::<Source>(value).is_err());
        }
        assert!(Source::Configured {
            repository: RepositoryIdentity {
                provider: crate::config::ReviewProvider::Github,
                workspace: "../owned".into(),
                repo: "same".into()
            }
        }
        .validate()
        .is_err());
    }
    #[test]
    fn capture_comparison_layers_and_cancel_identity_are_strict() {
        let mut params = Comparison {
            source: Source::Current {},
            mode: Mode::WorkingTree,
            base_ref: None,
            layers: vec![Layer::Committed, Layer::Staged, Layer::Unstaged],
        };
        assert!(params.validate().is_ok());
        params.layers.reverse();
        assert!(params.validate().is_err());
        params.layers = vec![Layer::Staged, Layer::Staged];
        assert!(params.validate().is_err());
        params.layers = vec![Layer::Committed];
        params.mode = Mode::Branch;
        assert!(params.validate().is_err());
        for base in ["", "--exec-path", "line\ncontrol", " surrounding "] {
            params.base_ref = Some(base.into());
            assert!(params.validate().is_err());
        }
        params.base_ref = Some("origin/main".into());
        assert!(params.validate().is_ok());
        let capture = CaptureParams {
            capture_id: "A".repeat(64),
            comparison: params,
        };
        assert!(capture.validate().is_err());
    }
    #[test]
    fn snapshot_reference_keeps_provider_and_root_identity() {
        let repository = RepositoryIdentity {
            provider: crate::config::ReviewProvider::Github,
            workspace: "owned".into(),
            repo: "same".into(),
        };
        let mut snapshot = SnapshotRef {
            source: Source::Configured {
                repository: repository.clone(),
            },
            repository,
            root_id: "a".repeat(64),
            snapshot_id: "b".repeat(64),
        };
        assert!(snapshot.validate().is_ok());
        snapshot.repository.provider = crate::config::ReviewProvider::Bitbucket;
        assert!(snapshot.validate().is_err());
        assert_ne!(
            root_id(Path::new("/owned/one/same")).unwrap(),
            root_id(Path::new("/owned/two/same")).unwrap()
        );
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use std::process::Command;
    struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
    fn git(root: &Path, args: &[&str]) {
        let null_config = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", null_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "owned Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn qualified_options_prove_current_fallback_and_exact_configured_clone() {
        let _lock = crate::review_storage::TEST_DATA_DIR_ENV_LOCK
            .lock()
            .unwrap();
        let config_dir = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let restore = Restore(vec![(
            "NORN_CONFIG_DIR",
            std::env::var_os("NORN_CONFIG_DIR"),
        )]);
        std::env::set_var("NORN_CONFIG_DIR", config_dir.path());
        for path in [root.path(), second.path()] {
            git(path, &["init"]);
            git(path, &["config", "user.name", "Owned"]);
            git(path, &["config", "user.email", "owned@example.invalid"]);
            std::fs::write(path.join("tracked.txt"), "owned\n").unwrap();
            git(path, &["add", "tracked.txt"]);
            git(
                path,
                &["-c", "commit.gpgsign=false", "commit", "-m", "owned"],
            );
            git(path, &["branch", "comparison"]);
        }
        git(
            root.path(),
            &[
                "remote",
                "add",
                "upstream",
                "https://github.com/owned/same.git",
            ],
        );
        let current = options(
            OptionsParams {
                source: Source::Current {},
                offset: 0,
                limit: 32,
            },
            root.path(),
            LocalReviewCancellation::new(),
        )
        .unwrap();
        assert_eq!(
            current.repository.provider,
            crate::config::ReviewProvider::Github
        );
        assert_eq!(current.repository.workspace, "owned");
        assert_eq!(current.repository.repo, "same");
        assert_eq!(current.base_refs, ["comparison"]);
        assert!(current.head_sha.is_some());
        assert!(options(
            OptionsParams {
                source: Source::Current {},
                offset: 0,
                limit: 32
            },
            second.path(),
            LocalReviewCancellation::new()
        )
        .is_err());
        git(
            root.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/owned/same.git",
            ],
        );
        git(
            second.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://bitbucket.org/owned/same.git",
            ],
        );
        let config = serde_json::json!({"repos":[{"provider":"github","workspace":"owned","repo":"same","localPath":root.path()},{"provider":"bitbucket","workspace":"owned","repo":"same","localPath":second.path()}],"defaultDiffView":"unified","theme":"dark"});
        std::fs::write(config_dir.path().join("settings.json"), config.to_string()).unwrap();
        let request = |provider| OptionsParams {
            source: Source::Configured {
                repository: RepositoryIdentity {
                    provider,
                    workspace: "owned".into(),
                    repo: "same".into(),
                },
            },
            offset: 0,
            limit: 32,
        };
        let github = options(
            request(crate::config::ReviewProvider::Github),
            second.path(),
            LocalReviewCancellation::new(),
        )
        .unwrap();
        let bitbucket = options(
            request(crate::config::ReviewProvider::Bitbucket),
            root.path(),
            LocalReviewCancellation::new(),
        )
        .unwrap();
        assert_ne!(github.root_id, bitbucket.root_id);
        assert_eq!(github.root_id, current.root_id);
        git(
            root.path(),
            &[
                "remote",
                "set-url",
                "origin",
                "https://github.com/owned/same-other.git",
            ],
        );
        assert!(options(
            request(crate::config::ReviewProvider::Github),
            second.path(),
            LocalReviewCancellation::new()
        )
        .is_err());
        let cancelled = LocalReviewCancellation::new();
        cancelled.cancel();
        assert!(options(
            OptionsParams {
                source: Source::Current {},
                offset: 0,
                limit: 32
            },
            second.path(),
            cancelled
        )
        .is_err());
        drop(restore);
    }
}
