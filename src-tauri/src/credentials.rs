use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SERVICE: &str = "app.norn.desktop";
const LEGACY_SERVICE: &str = "app.lachesi.desktop";
const ACCOUNT: &str = "bitbucket";
const APP_DIR: &str = "norn";
const LEGACY_APP_DIR: &str = "lachesi";
const TERMINAL_CONFIG_FILE: &str = "config.toml";

#[derive(Serialize, Deserialize, Clone)]
pub struct Credentials {
    pub username: String,
    pub token: String,
}

fn entry_for_service(service: &str, account: &str) -> Result<Entry, String> {
    Entry::new(service, account).map_err(|e| e.to_string())
}

fn keychain_secret_no_copy(service: &str, account: &str) -> Option<String> {
    entry_for_service(service, account)
        .ok()?
        .get_password()
        .ok()
}

#[derive(Default, Deserialize)]
struct TerminalConfig {
    credentials: Option<TerminalCredentialConfig>,
}

#[derive(Default, Deserialize)]
struct TerminalCredentialConfig {
    github: Option<TerminalGithubCredentials>,
    bitbucket: Option<TerminalBitbucketCredentials>,
}

#[derive(Default, Deserialize)]
struct TerminalGithubCredentials {
    token_env: Option<String>,
}

#[derive(Default, Deserialize)]
struct TerminalBitbucketCredentials {
    username_env: Option<String>,
    token_env: Option<String>,
}

fn terminal_config_path() -> Option<PathBuf> {
    let mut dir = dirs::config_dir()?;
    dir.push(APP_DIR);
    Some(dir.join(TERMINAL_CONFIG_FILE))
}

fn legacy_terminal_config_path() -> Option<PathBuf> {
    let mut dir = dirs::config_dir()?;
    dir.push(LEGACY_APP_DIR);
    Some(dir.join(TERMINAL_CONFIG_FILE))
}

fn parse_terminal_config(contents: &str) -> Result<TerminalConfig, String> {
    toml::from_str(contents).map_err(|e| e.to_string())
}

fn load_terminal_config() -> TerminalConfig {
    let (Some(canonical), Some(legacy)) = (terminal_config_path(), legacy_terminal_config_path())
    else {
        return TerminalConfig::default();
    };
    load_terminal_config_from_paths(&canonical, &legacy)
}

fn load_terminal_config_from_paths(canonical: &Path, legacy: &Path) -> TerminalConfig {
    if canonical.exists() {
        return load_terminal_config_from_path(canonical).unwrap_or_default();
    }
    if legacy.is_file() {
        let migration =
            crate::runtime_identity::migrate_file_atomically(legacy, canonical, |bytes| {
                let contents = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
                parse_terminal_config(contents).map(|_| ())
            });
        if canonical.exists() {
            return load_terminal_config_from_path(canonical).unwrap_or_default();
        }
        if migration.is_err() {
            return load_terminal_config_from_path(legacy).unwrap_or_default();
        }
    }
    TerminalConfig::default()
}

fn resolve_secret_with(
    account: &str,
    mut get: impl FnMut(&str, &str) -> Option<String>,
    mut set: impl FnMut(&str, &str, &str) -> Result<(), String>,
) -> Option<String> {
    if let Some(secret) = get(SERVICE, account)
        .filter(|secret| !secret.is_empty() && secret_is_valid_for_account(account, secret))
    {
        return Some(secret);
    }
    let secret = get(LEGACY_SERVICE, account)
        .filter(|secret| !secret.is_empty() && secret_is_valid_for_account(account, secret))?;
    // A failed copy is deliberately silent: the legacy reference remains the
    // usable source and no keychain error is allowed to include secret data.
    let _ = set(SERVICE, account, &secret);
    Some(secret)
}

fn secret_is_valid_for_account(account: &str, secret: &str) -> bool {
    if account != ACCOUNT {
        return true;
    }
    serde_json::from_str::<Credentials>(secret)
        .is_ok_and(|credentials| !credentials.username.is_empty() && !credentials.token.is_empty())
}

fn load_keychain_secret(account: &str) -> Option<String> {
    resolve_secret_with(
        account,
        |service, account| {
            entry_for_service(service, account)
                .ok()?
                .get_password()
                .ok()
        },
        |service, account, secret| {
            entry_for_service(service, account)?
                .set_password(secret)
                .map_err(|error| error.to_string())
        },
    )
}

fn load_terminal_config_from_path(path: &Path) -> Result<TerminalConfig, String> {
    let contents = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_terminal_config(&contents)
}

fn configured_env_value(env_name: Option<&str>) -> Option<String> {
    let env_name = env_name?.trim();
    if env_name.is_empty() {
        return None;
    }
    std::env::var(env_name)
        .ok()
        .filter(|value| !value.is_empty())
}

fn bitbucket_from_terminal_config(config: &TerminalConfig) -> Option<Credentials> {
    let bitbucket = config.credentials.as_ref()?.bitbucket.as_ref()?;
    let username = configured_env_value(bitbucket.username_env.as_deref())?;
    let token = configured_env_value(bitbucket.token_env.as_deref())?;
    Some(Credentials { username, token })
}

fn github_from_terminal_config(config: &TerminalConfig) -> Option<String> {
    let github = config.credentials.as_ref()?.github.as_ref()?;
    configured_env_value(github.token_env.as_deref())
}

pub fn has_bitbucket_credential_source() -> bool {
    let mut valid_secret = false;
    for service in [SERVICE, LEGACY_SERVICE] {
        if let Some(secret) = keychain_secret_no_copy(service, ACCOUNT) {
            if serde_json::from_str::<Credentials>(&secret)
                .is_ok_and(|cred| !cred.username.is_empty() && !cred.token.is_empty())
            {
                valid_secret = true;
                break;
            }
        }
    }
    if valid_secret {
        return true;
    }

    let config = load_terminal_config();
    if let Some(creds) = bitbucket_from_terminal_config(&config) {
        return !creds.username.is_empty() && !creds.token.is_empty();
    }

    let username = std::env::var("BITBUCKET_USERNAME").ok();
    let token = std::env::var("BITBUCKET_TOKEN").ok();
    username.is_some_and(|username| !username.is_empty())
        && token.is_some_and(|token| !token.is_empty())
}

pub fn has_github_credential_source() -> bool {
    if keychain_secret_no_copy(SERVICE, ACCOUNT_GITHUB).is_some() {
        return true;
    }
    if keychain_secret_no_copy(LEGACY_SERVICE, ACCOUNT_GITHUB).is_some() {
        return true;
    }
    let config = load_terminal_config();
    github_from_terminal_config(&config).is_some()
        || std::env::var("GITHUB_TOKEN").is_ok_and(|token| !token.is_empty())
}

/// Resolve credentials: keychain first, terminal config env refs, then
/// `BITBUCKET_*` env vars (dev fallback).
pub fn load() -> Option<Credentials> {
    if let Some(secret) = load_keychain_secret(ACCOUNT) {
        if let Ok(creds) = serde_json::from_str::<Credentials>(&secret) {
            if !creds.username.is_empty() && !creds.token.is_empty() {
                return Some(creds);
            }
        }
    }

    if let Some(creds) = bitbucket_from_terminal_config(&load_terminal_config()) {
        return Some(creds);
    }

    let username = std::env::var("BITBUCKET_USERNAME").ok();
    let token = std::env::var("BITBUCKET_TOKEN").ok();
    if let (Some(username), Some(token)) = (username, token) {
        if !username.is_empty() && !token.is_empty() {
            return Some(Credentials { username, token });
        }
    }

    None
}

/// Store credentials in the OS keychain. Never called for env-sourced creds.
pub fn store(creds: &Credentials) -> Result<(), String> {
    let entry = entry_for_service(SERVICE, ACCOUNT)?;
    let json = serde_json::to_string(creds).map_err(|e| e.to_string())?;
    entry.set_password(&json).map_err(|e| e.to_string())
}

pub fn clear() -> Result<(), String> {
    for service in [SERVICE, LEGACY_SERVICE] {
        match entry_for_service(service, ACCOUNT)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

pub fn has() -> bool {
    load().is_some()
}

const ACCOUNT_JIRA: &str = "jira";
const ACCOUNT_NOTION: &str = "notion";
const ACCOUNT_GITHUB: &str = "github";

fn entry_for(account: &str) -> Result<Entry, String> {
    entry_for_service(SERVICE, account)
}

fn load_token(account: &str, env_var: &str) -> Option<String> {
    if let Some(secret) = load_keychain_secret(account) {
        return Some(secret);
    }
    std::env::var(env_var).ok().filter(|s| !s.is_empty())
}

fn store_token(account: &str, token: &str) -> Result<(), String> {
    entry_for(account)?
        .set_password(token)
        .map_err(|e| e.to_string())
}

fn clear_token(account: &str) -> Result<(), String> {
    for service in [SERVICE, LEGACY_SERVICE] {
        match entry_for_service(service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

pub fn load_jira_token() -> Option<String> {
    load_token(ACCOUNT_JIRA, "JIRA_TOKEN")
}
pub fn store_jira_token(token: &str) -> Result<(), String> {
    store_token(ACCOUNT_JIRA, token)
}
pub fn clear_jira_token() -> Result<(), String> {
    clear_token(ACCOUNT_JIRA)
}
pub fn has_jira() -> bool {
    load_jira_token().is_some()
}

pub fn load_notion_token() -> Option<String> {
    load_token(ACCOUNT_NOTION, "NOTION_TOKEN")
}
pub fn store_notion_token(token: &str) -> Result<(), String> {
    store_token(ACCOUNT_NOTION, token)
}
pub fn clear_notion_token() -> Result<(), String> {
    clear_token(ACCOUNT_NOTION)
}
pub fn has_notion() -> bool {
    load_notion_token().is_some()
}

pub fn load_github_token() -> Option<String> {
    if let Some(secret) = load_keychain_secret(ACCOUNT_GITHUB) {
        return Some(secret);
    }
    github_from_terminal_config(&load_terminal_config())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()))
}
pub fn store_github_token(token: &str) -> Result<(), String> {
    store_token(ACCOUNT_GITHUB, token)
}
pub fn clear_github_token() -> Result<(), String> {
    clear_token(ACCOUNT_GITHUB)
}
pub fn has_github() -> bool {
    load_github_token().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    fn unique_env(prefix: &str) -> String {
        format!("{prefix}_{}_{}", std::process::id(), line!())
    }

    #[test]
    fn terminal_config_resolves_bitbucket_env_refs() {
        let username_env = unique_env("LACHESI_TEST_BB_USER");
        let token_env = unique_env("LACHESI_TEST_BB_TOKEN");
        std::env::set_var(&username_env, "reviewer@example.com");
        std::env::set_var(&token_env, "bb-token");
        let config = parse_terminal_config(&format!(
            r#"
[credentials.bitbucket]
username_env = "{username_env}"
token_env = "{token_env}"
"#
        ))
        .expect("config");

        let creds = bitbucket_from_terminal_config(&config).expect("bitbucket credentials");

        assert_eq!(creds.username, "reviewer@example.com");
        assert_eq!(creds.token, "bb-token");
        std::env::remove_var(username_env);
        std::env::remove_var(token_env);
    }

    #[test]
    fn terminal_config_resolves_github_env_ref() {
        let token_env = unique_env("LACHESI_TEST_GH_TOKEN");
        std::env::set_var(&token_env, "gh-token");
        let config = parse_terminal_config(&format!(
            r#"
[credentials.github]
token_env = "{token_env}"
"#
        ))
        .expect("config");

        assert_eq!(
            github_from_terminal_config(&config).as_deref(),
            Some("gh-token")
        );
        std::env::remove_var(token_env);
    }

    #[test]
    fn terminal_config_ignores_missing_env_refs() {
        let config = parse_terminal_config(
            r#"
[credentials.github]
token_env = "LACHESI_TEST_GH_TOKEN_MISSING"
"#,
        )
        .expect("config");

        assert!(github_from_terminal_config(&config).is_none());
    }

    #[test]
    fn terminal_config_migrates_from_legacy_path_and_keeps_source() {
        let root = tempfile::tempdir().expect("tempdir");
        let legacy = root.path().join("lachesi/config.toml");
        let canonical = root.path().join("norn/config.toml");
        fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("legacy directory");
        fs::write(
            &legacy,
            "[credentials.github]\ntoken_env = \"NORN_TEST_GH_TOKEN\"\n",
        )
        .expect("legacy terminal config");

        let config = load_terminal_config_from_paths(&canonical, &legacy);

        assert_eq!(
            config
                .credentials
                .and_then(|credentials| credentials.github)
                .and_then(|github| github.token_env)
                .as_deref(),
            Some("NORN_TEST_GH_TOKEN")
        );
        assert!(legacy.exists());
        assert!(canonical.exists());
    }

    #[test]
    fn credential_migration_prefers_canonical_and_is_idempotent() {
        let secrets = RefCell::new(HashMap::from([
            (
                (SERVICE.to_string(), ACCOUNT_GITHUB.to_string()),
                "canonical".to_string(),
            ),
            (
                (LEGACY_SERVICE.to_string(), ACCOUNT_GITHUB.to_string()),
                "legacy".to_string(),
            ),
        ]));
        let writes = Cell::new(0);

        let resolved = resolve_secret_with(
            ACCOUNT_GITHUB,
            |service, account| {
                secrets
                    .borrow()
                    .get(&(service.to_string(), account.to_string()))
                    .cloned()
            },
            |service, account, secret| {
                writes.set(writes.get() + 1);
                secrets.borrow_mut().insert(
                    (service.to_string(), account.to_string()),
                    secret.to_string(),
                );
                Ok(())
            },
        );

        assert_eq!(resolved.as_deref(), Some("canonical"));
        assert_eq!(writes.get(), 0);
    }

    #[test]
    fn credential_migration_copies_legacy_secret_once_without_removing_it() {
        let secrets = RefCell::new(HashMap::from([(
            (LEGACY_SERVICE.to_string(), ACCOUNT_JIRA.to_string()),
            "sensitive-value".to_string(),
        )]));
        let writes = Cell::new(0);
        let resolve = || {
            resolve_secret_with(
                ACCOUNT_JIRA,
                |service, account| {
                    secrets
                        .borrow()
                        .get(&(service.to_string(), account.to_string()))
                        .cloned()
                },
                |service, account, secret| {
                    writes.set(writes.get() + 1);
                    secrets.borrow_mut().insert(
                        (service.to_string(), account.to_string()),
                        secret.to_string(),
                    );
                    Ok(())
                },
            )
        };

        assert_eq!(resolve().as_deref(), Some("sensitive-value"));
        assert_eq!(resolve().as_deref(), Some("sensitive-value"));
        assert_eq!(writes.get(), 1);
        assert_eq!(
            secrets
                .borrow()
                .get(&(LEGACY_SERVICE.to_string(), ACCOUNT_JIRA.to_string()))
                .map(String::as_str),
            Some("sensitive-value")
        );
    }

    #[test]
    fn failed_keychain_copy_keeps_legacy_secret_usable_without_a_diagnostic() {
        let secret = "must-not-appear-in-output";
        let resolved = resolve_secret_with(
            ACCOUNT_NOTION,
            |service, _| (service == LEGACY_SERVICE).then(|| secret.to_string()),
            |_, _, _| Err("keychain unavailable".to_string()),
        );

        assert_eq!(resolved.as_deref(), Some(secret));
    }

    #[test]
    fn malformed_legacy_bitbucket_secret_is_not_copied_or_preferred() {
        let secrets = RefCell::new(HashMap::from([(
            (LEGACY_SERVICE.to_string(), ACCOUNT.to_string()),
            "not-json".to_string(),
        )]));
        let writes = Cell::new(0);

        let resolved = resolve_secret_with(
            ACCOUNT,
            |service, account| {
                secrets
                    .borrow()
                    .get(&(service.to_string(), account.to_string()))
                    .cloned()
            },
            |service, account, secret| {
                writes.set(writes.get() + 1);
                secrets.borrow_mut().insert(
                    (service.to_string(), account.to_string()),
                    secret.to_string(),
                );
                Ok(())
            },
        );

        assert!(resolved.is_none());
        assert_eq!(writes.get(), 0);
        assert!(!secrets
            .borrow()
            .contains_key(&(SERVICE.to_string(), ACCOUNT.to_string())));
        assert_eq!(
            secrets
                .borrow()
                .get(&(LEGACY_SERVICE.to_string(), ACCOUNT.to_string()))
                .map(String::as_str),
            Some("not-json")
        );
    }
}
