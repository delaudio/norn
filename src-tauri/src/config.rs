use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const APP_DIR: &str = "norn";
const LEGACY_APP_DIR: &str = "lachesi";
const CONFIG_FILE: &str = "settings.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewProvider {
    #[default]
    Bitbucket,
    Github,
}

/// A single source-control repository the app tracks.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    #[serde(default)]
    pub provider: ReviewProvider,
    pub workspace: String,
    pub repo: String,
    #[serde(default)]
    pub local_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewTerminal {
    #[serde(rename = "wezterm")]
    WezTerm,
    #[serde(rename = "iterm")]
    ITerm,
    #[serde(rename = "terminal")]
    Terminal,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    Claude,
    Codex,
}

/// Non-secret application configuration, persisted as JSON in the OS config dir.
/// Secrets (username/token) live in the keychain — see `credentials`.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub repos: Vec<RepoRef>,
    #[serde(default)]
    pub review_provider: ReviewProvider,
    pub default_diff_view: String,
    pub theme: String,
    #[serde(default)]
    pub review_terminal: Option<ReviewTerminal>,
    #[serde(default)]
    pub ai_provider: AiProvider,
    #[serde(default)]
    pub claude_model: Option<String>,
    #[serde(default)]
    pub claude_effort: Option<String>,
    #[serde(default)]
    pub codex_model: Option<String>,
    #[serde(default)]
    pub codex_effort: Option<String>,
    /// Local consent for headless review to send the selected diff to the
    /// configured AI provider. Desktop and TUI review have their own explicit
    /// user actions and do not use this automation-specific setting.
    #[serde(default)]
    pub headless_ai_diff_sharing_allowed: bool,
    /// Jira site base URL for issue links, e.g. https://example.atlassian.net
    #[serde(default)]
    pub jira_base_url: Option<String>,
    /// Automatic pull request sync interval in seconds. None disables polling.
    #[serde(default)]
    pub automatic_sync_interval_seconds: Option<u64>,
    #[serde(default = "default_true")]
    pub menu_bar_sync_enabled: bool,
    #[serde(default)]
    pub notifications_enabled: bool,
    /// Derived at read time. Not persisted.
    #[serde(default, skip_serializing)]
    pub configured: bool,
    /// Derived at read time. Not persisted.
    #[serde(default, skip_serializing)]
    pub has_credentials: bool,
    #[serde(default, skip_serializing)]
    pub has_github_credentials: bool,
    #[serde(default, skip_serializing)]
    pub has_jira: bool,
    #[serde(default, skip_serializing)]
    pub has_notion: bool,
    /// Legacy single-repo fields, read for migration, never written back.
    #[serde(default, skip_serializing)]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing)]
    pub repo: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            review_provider: ReviewProvider::Bitbucket,
            default_diff_view: "unified".to_string(),
            theme: "dark".to_string(),
            review_terminal: None,
            ai_provider: AiProvider::Claude,
            claude_model: None,
            claude_effort: None,
            codex_model: None,
            codex_effort: None,
            headless_ai_diff_sharing_allowed: false,
            jira_base_url: None,
            automatic_sync_interval_seconds: None,
            menu_bar_sync_enabled: true,
            notifications_enabled: false,
            configured: false,
            has_credentials: false,
            has_github_credentials: false,
            has_jira: false,
            has_notion: false,
            workspace: None,
            repo: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn config_dir() -> Result<PathBuf, String> {
    let mut dir =
        dirs::config_dir().ok_or_else(|| "could not resolve config directory".to_string())?;
    dir.push(APP_DIR);
    Ok(dir)
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join(CONFIG_FILE))
}

fn legacy_config_path() -> Result<PathBuf, String> {
    let mut dir =
        dirs::config_dir().ok_or_else(|| "could not resolve config directory".to_string())?;
    dir.push(LEGACY_APP_DIR);
    Ok(dir.join(CONFIG_FILE))
}

fn resolve_read_path(canonical: &Path, legacy: &Path) -> PathBuf {
    if canonical.exists() {
        return canonical.to_path_buf();
    }
    if legacy.is_file() {
        let migration =
            crate::runtime_identity::migrate_file_atomically(legacy, canonical, |bytes| {
                serde_json::from_slice::<AppConfig>(bytes)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
        match migration {
            Ok(_) if canonical.exists() => return canonical.to_path_buf(),
            Ok(_) => {}
            Err(error) => eprintln!(
                "Norn could not migrate settings to {}: {error}. The legacy settings at {} remain usable.",
                canonical.display(),
                legacy.display()
            ),
        }
        return legacy.to_path_buf();
    }
    canonical.to_path_buf()
}

fn load_from_paths(canonical: &Path, legacy: &Path) -> AppConfig {
    let path = resolve_read_path(canonical, legacy);
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Read config from disk, migrating the legacy single-repo shape if present.
pub fn load() -> AppConfig {
    let mut cfg = match (config_path(), legacy_config_path()) {
        (Ok(canonical), Ok(legacy)) => load_from_paths(&canonical, &legacy),
        _ => AppConfig::default(),
    };

    if cfg.repos.is_empty() {
        if let (Some(ws), Some(repo)) = (cfg.workspace.clone(), cfg.repo.clone()) {
            if !ws.is_empty() && !repo.is_empty() {
                cfg.repos.push(RepoRef {
                    provider: ReviewProvider::Bitbucket,
                    workspace: ws,
                    repo,
                    local_path: None,
                });
            }
        }
    }
    cfg.workspace = None;
    cfg.repo = None;
    cfg
}

/// Persist the non-secret config fields to disk.
pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(dir.join(CONFIG_FILE), json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{load_from_paths, AiProvider, AppConfig, ReviewProvider};
    use std::fs;

    #[test]
    fn serializes_codex_provider_settings_in_local_config_shape() {
        let config = AppConfig {
            ai_provider: AiProvider::Codex,
            review_provider: ReviewProvider::Github,
            codex_model: Some("gpt-5-codex".to_string()),
            codex_effort: Some("high".to_string()),
            ..AppConfig::default()
        };

        let json = serde_json::to_string(&config).expect("config should serialize");
        assert!(json.contains(r#""aiProvider":"codex""#));
        assert!(json.contains(r#""reviewProvider":"github""#));
        assert!(json.contains(r#""codexModel":"gpt-5-codex""#));
        assert!(json.contains(r#""codexEffort":"high""#));

        let parsed: AppConfig = serde_json::from_str(&json).expect("config should deserialize");
        assert_eq!(parsed.ai_provider, AiProvider::Codex);
        assert_eq!(parsed.review_provider, ReviewProvider::Github);
        assert_eq!(parsed.codex_model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(parsed.codex_effort.as_deref(), Some("high"));
        assert!(!parsed.headless_ai_diff_sharing_allowed);
    }

    #[test]
    fn serializes_headless_ai_diff_sharing_consent() {
        let config = AppConfig {
            headless_ai_diff_sharing_allowed: true,
            ..AppConfig::default()
        };

        let json = serde_json::to_string(&config).expect("config should serialize");
        assert!(json.contains(r#""headlessAiDiffSharingAllowed":true"#));

        let parsed: AppConfig = serde_json::from_str(&json).expect("config should deserialize");
        assert!(parsed.headless_ai_diff_sharing_allowed);
    }

    #[test]
    fn serializes_optional_automatic_sync_interval() {
        let config = AppConfig {
            automatic_sync_interval_seconds: Some(300),
            ..AppConfig::default()
        };

        let json = serde_json::to_string(&config).expect("config should serialize");
        assert!(json.contains(r#""automaticSyncIntervalSeconds":300"#));

        let parsed: AppConfig = serde_json::from_str(&json).expect("config should deserialize");
        assert_eq!(parsed.automatic_sync_interval_seconds, Some(300));
    }

    #[test]
    fn load_migrates_legacy_settings_once_and_preserves_the_source() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("lachesi/settings.json");
        let canonical = root.path().join("norn/settings.json");
        fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("legacy directory");
        fs::write(
            &legacy,
            r#"{"defaultDiffView":"split","theme":"light","repos":[]}"#,
        )
        .expect("legacy settings");

        let first = load_from_paths(&canonical, &legacy);
        let second = load_from_paths(&canonical, &legacy);

        assert_eq!(first.default_diff_view, "split");
        assert_eq!(second.theme, "light");
        assert!(legacy.exists());
        assert!(canonical.exists());
    }

    #[test]
    fn canonical_settings_take_precedence_over_legacy_settings() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("lachesi/settings.json");
        let canonical = root.path().join("norn/settings.json");
        fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("legacy directory");
        fs::create_dir_all(canonical.parent().expect("canonical parent"))
            .expect("canonical directory");
        fs::write(
            &legacy,
            r#"{"defaultDiffView":"unified","theme":"legacy","repos":[]}"#,
        )
        .expect("legacy settings");
        fs::write(
            &canonical,
            r#"{"defaultDiffView":"split","theme":"canonical","repos":[]}"#,
        )
        .expect("canonical settings");

        assert_eq!(load_from_paths(&canonical, &legacy).theme, "canonical");
    }
}
