//! Rust-owned terminal preferences. Secrets are never part of this schema.
use crate::config::{AiProvider, AppConfig, ReviewProvider};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const MAX_SETTINGS_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsRepo {
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSettings {
    pub repos: Vec<SettingsRepo>,
    pub review_provider: ReviewProvider,
    pub ai_provider: AiProvider,
    pub claude_model: Option<String>,
    pub claude_effort: Option<String>,
    pub codex_model: Option<String>,
    pub codex_effort: Option<String>,
    pub opencode_model: Option<String>,
    pub opencode_effort: Option<String>,
    pub default_diff_view: String,
    pub theme: String,
    pub jira_base_url: Option<String>,
    pub automatic_sync_interval_seconds: Option<u64>,
}

impl WorkspaceSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.repos.len() > 100 {
            return Err("At most 100 repositories can be configured.".to_string());
        }
        let mut identities = std::collections::HashSet::new();
        for repo in &self.repos {
            for value in [&repo.workspace, &repo.repo] {
                if value.trim().is_empty()
                    || value.chars().count() > 256
                    || value.chars().any(char::is_control)
                {
                    return Err(
                        "Repository owner and name must contain 1–256 printable characters."
                            .to_string(),
                    );
                }
            }
            let key = (
                repo.provider == ReviewProvider::Github,
                repo.workspace.to_lowercase(),
                repo.repo.to_lowercase(),
            );
            if !identities.insert(key) {
                return Err("A repository can be configured only once per provider.".to_string());
            }
            validate_optional(&repo.local_path, 4096)?;
        }
        for value in [&self.claude_model, &self.codex_model, &self.opencode_model] {
            validate_optional(value, 512)?;
        }
        for value in [
            &self.claude_effort,
            &self.codex_effort,
            &self.opencode_effort,
        ] {
            validate_optional(value, 64)?;
        }
        if !matches!(self.default_diff_view.as_str(), "unified" | "split")
            || !matches!(self.theme.as_str(), "dark" | "light")
        {
            return Err("Choose a supported diff view and theme.".to_string());
        }
        validate_optional(&self.jira_base_url, 2048)?;
        if self.jira_base_url.as_ref().is_some_and(|value| {
            reqwest::Url::parse(value).map_or(true, |url| {
                !matches!(url.scheme(), "https" | "http")
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
            })
        }) {
            return Err("Jira URL must be HTTP(S) without embedded credentials.".to_string());
        }
        if self
            .automatic_sync_interval_seconds
            .is_some_and(|value| !(10..=86400).contains(&value))
        {
            return Err(
                "Automatic sync interval must be 10–86400 seconds, or disabled.".to_string(),
            );
        }
        Ok(())
    }

    fn from_config(config: &AppConfig) -> Self {
        Self {
            repos: config
                .repos
                .iter()
                .map(|repo| SettingsRepo {
                    provider: repo.provider,
                    workspace: repo.workspace.clone(),
                    repo: repo.repo.clone(),
                    local_path: repo.local_path.clone(),
                })
                .collect(),
            review_provider: config.review_provider,
            ai_provider: config.ai_provider,
            claude_model: config.claude_model.clone(),
            claude_effort: config.claude_effort.clone(),
            codex_model: config.codex_model.clone(),
            codex_effort: config.codex_effort.clone(),
            opencode_model: config.opencode_model.clone(),
            opencode_effort: config.opencode_effort.clone(),
            default_diff_view: config.default_diff_view.clone(),
            theme: config.theme.clone(),
            jira_base_url: config.jira_base_url.clone(),
            automatic_sync_interval_seconds: config.automatic_sync_interval_seconds,
        }
    }

    fn apply_to(self, config: &mut AppConfig) {
        config.repos = self
            .repos
            .into_iter()
            .map(|repo| crate::config::RepoRef {
                provider: repo.provider,
                workspace: repo.workspace,
                repo: repo.repo,
                local_path: repo.local_path,
            })
            .collect();
        config.review_provider = self.review_provider;
        config.ai_provider = self.ai_provider;
        config.claude_model = self.claude_model;
        config.claude_effort = self.claude_effort;
        config.codex_model = self.codex_model;
        config.codex_effort = self.codex_effort;
        config.opencode_model = self.opencode_model;
        config.opencode_effort = self.opencode_effort;
        config.default_diff_view = self.default_diff_view;
        config.theme = self.theme;
        config.jira_base_url = self.jira_base_url;
        config.automatic_sync_interval_seconds = self.automatic_sync_interval_seconds;
    }
}

fn validate_optional(value: &Option<String>, max: usize) -> Result<(), String> {
    if value.as_ref().is_some_and(|value| {
        value.trim().is_empty()
            || value.chars().count() > max
            || value.chars().any(char::is_control)
    }) {
        return Err(
            "A settings value is empty, too long or contains control characters.".to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub version: String,
    pub settings: WorkspaceSettings,
}

fn settings_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn read_config(canonical: &Path, legacy: &Path) -> Result<(AppConfig, String), String> {
    let path = if canonical.exists() {
        canonical
    } else {
        legacy
    };
    let mut bytes = Vec::new();
    let mut config = match fs::File::open(path) {
        Ok(file) => {
            file.take(MAX_SETTINGS_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read stored settings.".to_string())?;
            if bytes.len() > MAX_SETTINGS_BYTES {
                return Err("Stored settings exceed the supported size. Preserve the file and inspect it before saving.".to_string());
            }
            serde_json::from_slice::<AppConfig>(&bytes).map_err(|_| {
                "Stored settings are invalid. Preserve the file and inspect it before saving."
                    .to_string()
            })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
        Err(_) => {
            return Err("Cannot read stored settings. No preferences were changed.".to_string())
        }
    };
    if config.repos.is_empty() {
        if let (Some(workspace), Some(repo)) = (config.workspace.take(), config.repo.take()) {
            if !workspace.is_empty() && !repo.is_empty() {
                config.repos.push(crate::config::RepoRef {
                    provider: ReviewProvider::Bitbucket,
                    workspace,
                    repo,
                    local_path: None,
                });
            }
        }
    }
    Ok((config, hex::encode(Sha256::digest(&bytes))))
}

pub(crate) fn snapshot() -> Result<SettingsSnapshot, String> {
    let _guard = settings_lock()
        .lock()
        .map_err(|_| "Settings are unavailable.".to_string())?;
    let (canonical, legacy) = crate::config::settings_paths()?;
    snapshot_at(&canonical, &legacy)
}

fn snapshot_at(canonical: &Path, legacy: &Path) -> Result<SettingsSnapshot, String> {
    let (config, version) = read_config(canonical, legacy)?;
    let settings = WorkspaceSettings::from_config(&config);
    settings.validate()?;
    Ok(SettingsSnapshot { version, settings })
}

pub(crate) fn save(
    expected_version: &str,
    settings: WorkspaceSettings,
) -> Result<SettingsSnapshot, String> {
    let _guard = settings_lock()
        .lock()
        .map_err(|_| "Settings are unavailable.".to_string())?;
    let (canonical, legacy) = crate::config::settings_paths()?;
    save_at(&canonical, &legacy, expected_version, settings)
}

fn save_at(
    canonical: &Path,
    legacy: &Path,
    expected_version: &str,
    settings: WorkspaceSettings,
) -> Result<SettingsSnapshot, String> {
    settings.validate()?;
    let (mut config, version) = read_config(canonical, legacy)?;
    if version != expected_version {
        return Err("Settings changed since they were opened. Reload before saving; your draft was not applied.".to_string());
    }
    if fs::symlink_metadata(canonical)
        .is_ok_and(|meta| !meta.is_file() || meta.file_type().is_symlink())
    {
        return Err("The settings destination is not a regular file. Preserve it and inspect the configuration path.".to_string());
    }
    settings.apply_to(&mut config);
    let bytes =
        serde_json::to_vec_pretty(&config).map_err(|_| "Cannot encode settings.".to_string())?;
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err("Settings exceed the supported size.".to_string());
    }
    let parent = canonical
        .parent()
        .ok_or_else(|| "Settings path has no parent directory.".to_string())?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create the settings directory.".to_string())?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".norn-settings-")
        .tempfile_in(parent)
        .map_err(|_| "Cannot prepare settings for saving.".to_string())?;
    temporary
        .write_all(&bytes)
        .map_err(|_| "Cannot write settings; the previous file was preserved.".to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "Cannot flush settings; the previous file was preserved.".to_string())?;
    temporary
        .persist(canonical)
        .map_err(|_| "Cannot replace settings; the previous file was preserved.".to_string())?;
    snapshot_at(canonical, legacy)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_preserves_custom_models_and_unrelated_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let canonical = directory.path().join("norn/settings.json");
        let legacy = directory.path().join("legacy.json");
        let config = AppConfig {
            headless_ai_diff_sharing_allowed: true,
            review_terminal: Some(crate::config::ReviewTerminal::WezTerm),
            notifications_enabled: true,
            claude_model: Some("custom/org:model-v3 ✓".to_string()),
            ..AppConfig::default()
        };
        let original = serde_json::to_vec_pretty(&config).unwrap();
        fs::write(&legacy, &original).unwrap();
        let before = snapshot_at(&canonical, &legacy).unwrap();
        assert_eq!(before.settings.claude_model, config.claude_model);
        let mut edited = before.settings.clone();
        edited.ai_provider = AiProvider::Opencode;
        edited.opencode_model = Some("vendor/custom:reasoning".to_string());
        edited.repos.push(SettingsRepo {
            provider: ReviewProvider::Github,
            workspace: "fixture".to_string(),
            repo: "example".to_string(),
            local_path: None,
        });
        let after = save_at(&canonical, &legacy, &before.version, edited.clone()).unwrap();
        assert_eq!(after.settings, edited);
        assert_ne!(after.version, before.version);
        let saved: AppConfig = serde_json::from_slice(&fs::read(&canonical).unwrap()).unwrap();
        assert!(saved.headless_ai_diff_sharing_allowed && saved.notifications_enabled);
        assert_eq!(saved.review_terminal, config.review_terminal);
        assert_eq!(fs::read(&legacy).unwrap(), original);
        let raw = fs::read_to_string(&canonical).unwrap();
        assert!(!raw.contains("token") && !raw.contains("hasCredentials"));
    }
    #[test]
    fn stale_and_invalid_saves_preserve_the_current_file() {
        let directory = tempfile::tempdir().unwrap();
        let canonical = directory.path().join("settings.json");
        let before = snapshot_at(&canonical, &canonical).unwrap();
        let newer = serde_json::to_vec(&AppConfig {
            theme: "light".to_string(),
            ..AppConfig::default()
        })
        .unwrap();
        fs::write(&canonical, &newer).unwrap();
        assert!(save_at(
            &canonical,
            &canonical,
            &before.version,
            before.settings.clone()
        )
        .is_err());
        assert_eq!(fs::read(&canonical).unwrap(), newer);
        fs::write(&canonical, b"invalid preserved input").unwrap();
        assert!(snapshot_at(&canonical, &canonical).is_err());
        assert!(save_at(&canonical, &canonical, &before.version, before.settings).is_err());
        assert_eq!(fs::read(&canonical).unwrap(), b"invalid preserved input");
    }
    #[test]
    fn validation_rejects_embedded_credentials_duplicate_repos_and_unbounded_values() {
        let mut settings = WorkspaceSettings::from_config(&AppConfig::default());
        settings.jira_base_url = Some("https://private:secret@fixture.invalid".to_string());
        assert!(settings.validate().is_err());
        settings.jira_base_url = None;
        settings.codex_model = Some("x".repeat(513));
        assert!(settings.validate().is_err());
        settings.codex_model = Some("arbitrary/provider_model-v4".to_string());
        assert!(settings.validate().is_ok());
        let repo = SettingsRepo {
            provider: ReviewProvider::Github,
            workspace: "Fixture".to_string(),
            repo: "Demo".to_string(),
            local_path: None,
        };
        settings.repos = vec![
            repo.clone(),
            SettingsRepo {
                workspace: "fixture".to_string(),
                repo: "demo".to_string(),
                ..repo
            },
        ];
        assert!(settings.validate().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn linked_config_destination_is_preserved_instead_of_silently_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("source.json");
        let link = directory.path().join("settings.json");
        let original = serde_json::to_vec(&AppConfig::default()).unwrap();
        fs::write(&target, &original).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let before = snapshot_at(&link, &link).unwrap();
        assert!(save_at(&link, &link, &before.version, before.settings).is_err());
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), original);
    }
}
