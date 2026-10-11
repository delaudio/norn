//! Machine readiness without repository scans, subprocesses or credential reads.
use crate::config::AiProvider;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolReadiness {
    pub name: &'static str,
    pub available: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReadiness {
    pub configured_repos: usize,
    pub selected_agent: AiProvider,
    pub tools: Vec<ToolReadiness>,
    pub config_root_available: bool,
    pub data_root_available: bool,
    pub credential_store: &'static str,
}

pub(crate) fn snapshot() -> Result<WorkspaceReadiness, String> {
    let settings = super::settings::snapshot()?.settings;
    Ok(collect(
        &settings,
        executable_in_path,
        crate::config::settings_paths().is_ok(),
        super::workspace_data::canonical_path().is_ok(),
    ))
}
fn collect(
    settings: &super::settings::WorkspaceSettings,
    probe: impl Fn(&str) -> bool,
    config_root_available: bool,
    data_root_available: bool,
) -> WorkspaceReadiness {
    WorkspaceReadiness {
        configured_repos: settings.repos.len(),
        selected_agent: settings.ai_provider,
        tools: ["git", "claude", "codex", "opencode"]
            .into_iter()
            .map(|name| ToolReadiness {
                name,
                available: probe(name),
            })
            .collect(),
        config_root_available,
        data_root_available,
        // Current Cargo features enable apple-native only. #251 tracks native
        // stores on other platforms; never substitute plaintext storage.
        credential_store: if cfg!(target_os = "macos") {
            "native"
        } else {
            "unsupported"
        },
    }
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
fn executable_in_path(name: &str) -> bool {
    #[cfg(windows)]
    let names = [
        format!("{name}.exe"),
        format!("{name}.cmd"),
        format!("{name}.bat"),
    ];
    #[cfg(not(windows))]
    let names = [name.to_string()];
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path)
            .any(|directory| names.iter().any(|name| executable(&directory.join(name))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_uses_fake_probes_and_contains_no_credentials_or_tool_output() {
        let settings: super::super::settings::WorkspaceSettings = serde_json::from_value(serde_json::json!({
            "repos":[],"reviewProvider":"github","aiProvider":"opencode","claudeModel":null,"claudeEffort":null,
            "codexModel":null,"codexEffort":null,"opencodeModel":"custom/vendor", "opencodeEffort":null,
            "defaultDiffView":"split","theme":"light","jiraBaseUrl":null,"automaticSyncIntervalSeconds":null
        })).unwrap();
        let result = collect(
            &settings,
            |name| matches!(name, "git" | "opencode"),
            true,
            false,
        );
        assert_eq!(result.configured_repos, 0);
        assert_eq!(result.selected_agent, AiProvider::Opencode);
        assert!(result.tools[3].available);
        assert!(!result.tools[1].available);
        let data = serde_json::to_value(result).unwrap();
        assert!(data.get("credentials").is_none());
        assert!(data.get("token").is_none());
        assert!(!data.to_string().contains("custom/vendor"));
        assert!(!data["dataRootAvailable"].as_bool().unwrap());
    }
    #[cfg(unix)]
    #[test]
    fn directories_and_nonexecutable_files_are_not_available_tools() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("fake-cli");
        std::fs::write(&file, "synthetic never executed").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(!executable(&file));
        assert!(!executable(dir.path()));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(executable(&file));
    }
}
