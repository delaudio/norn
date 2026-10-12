use super::*;
use crate::local_review::{run_command_bounded_with_control, BoundedGitOutput, GitRunControl};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};

#[cfg(windows)]
const NULL: &str = "NUL";
#[cfg(not(windows))]
const NULL: &str = "/dev/null";

pub(super) struct Configuration {
    pub digest: String,
    pub filters: Vec<String>,
}
fn command(root: &Path, readonly: bool) -> Result<Command, String> {
    let root = root.canonicalize().map_err(|_| invalid())?;
    let mut command = Command::new(crate::local_repo::trusted_git_path()?);
    command
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("diff.external=")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("GIT_NO_LAZY_FETCH", "1");
    // Unit fixtures cannot inspect the developer's global configuration.
    // Owned production IPC fixtures set these on their child environment.
    #[cfg(test)]
    command
        .arg("-c")
        .arg(format!("core.attributesFile={NULL}"))
        .env("GIT_CONFIG_GLOBAL", NULL)
        .env("GIT_CONFIG_SYSTEM", NULL)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    // Preserve the user's normal credential, SSH and global Git configuration.
    // Remove alternate checkout/object/index and injected command configuration.
    for (name, _) in std::env::vars_os() {
        let key = name.to_string_lossy().to_ascii_uppercase();
        if matches!(
            key.as_str(),
            "GIT_DIR"
                | "GIT_WORK_TREE"
                | "GIT_COMMON_DIR"
                | "GIT_INDEX_FILE"
                | "GIT_OBJECT_DIRECTORY"
                | "GIT_ALTERNATE_OBJECT_DIRECTORIES"
                | "GIT_CONFIG"
                | "GIT_CONFIG_COUNT"
                | "GIT_CONFIG_PARAMETERS"
                | "GIT_EXEC_PATH"
                | "GIT_EXTERNAL_DIFF"
                | "GIT_DIFF_OPTS"
                | "GIT_LITERAL_PATHSPECS"
                | "GIT_GLOB_PATHSPECS"
                | "GIT_NOGLOB_PATHSPECS"
                | "GIT_ICASE_PATHSPECS"
        ) || key.starts_with("GIT_CONFIG_KEY_")
            || key.starts_with("GIT_CONFIG_VALUE_")
        {
            command.env_remove(name);
        }
    }
    if readonly {
        command
            .arg("--literal-pathspecs")
            .env("GIT_LITERAL_PATHSPECS", "1");
        command
            .arg("--no-optional-locks")
            .env("GIT_OPTIONAL_LOCKS", "0");
    } else {
        command.env("GIT_OPTIONAL_LOCKS", "1");
    }
    Ok(command)
}
pub(super) fn run(
    root: &Path,
    args: &[&str],
    maximum: usize,
    configuration: Option<&Configuration>,
    readonly: bool,
    control: &GitRunControl,
) -> Result<BoundedGitOutput, String> {
    let mut command = command(root, readonly)?;
    if readonly {
        // Previews cannot execute repository hooks or clean/process filters.
        // The explicit apply runs normal Git only after approval/config recheck.
        command.arg("-c").arg(format!("core.hooksPath={NULL}"));
        if let Some(configuration) = configuration {
            for driver in &configuration.filters {
                for option in ["clean", "smudge", "process"] {
                    command.arg("-c").arg(format!("filter.{driver}.{option}="));
                }
                command
                    .arg("-c")
                    .arg(format!("filter.{driver}.required=false"));
            }
        }
    }
    command.args(args);
    run_command_bounded_with_control(&mut command, None, maximum, "Repository operation", control)
        .map_err(|_| "The repository command failed, was canceled or exceeded its limits.".into())
}
pub(super) fn checked(
    root: &Path,
    args: &[&str],
    maximum: usize,
    configuration: Option<&Configuration>,
    control: &GitRunControl,
) -> Result<Vec<u8>, String> {
    let output = run(root, args, maximum, configuration, true, control)?;
    if !output.status.success() {
        return Err(invalid());
    }
    Ok(output.stdout)
}
pub(super) fn text(
    root: &Path,
    args: &[&str],
    configuration: &Configuration,
    control: &GitRunControl,
) -> Result<String, String> {
    String::from_utf8(checked(
        root,
        args,
        64 * 1024,
        Some(configuration),
        control,
    )?)
    .map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
    .map_err(|_| invalid())
}
pub(super) fn configuration(root: &Path, control: &GitRunControl) -> Result<Configuration, String> {
    let values = checked(
        root,
        &[
            "config",
            "--includes",
            "--null",
            "--show-origin",
            "--show-scope",
            "--list",
        ],
        1024 * 1024,
        None,
        control,
    )?;
    let keys = run(
        root,
        &[
            "config",
            "--includes",
            "--null",
            "--name-only",
            "--get-regexp",
            "^filter\\..*\\.(clean|smudge|process)$",
        ],
        64 * 1024,
        None,
        true,
        control,
    )?;
    if !keys.status.success() && keys.status.code() != Some(1) {
        return Err(invalid());
    }
    let mut filters = Vec::new();
    for key in keys.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let key = std::str::from_utf8(key).map_err(|_| invalid())?;
        let driver = key
            .strip_prefix("filter.")
            .and_then(|s| s.rsplit_once('.').map(|(name, _)| name))
            .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
            .ok_or_else(invalid)?;
        filters.push(driver.to_owned());
    }
    filters.sort();
    filters.dedup();
    if filters.len() > 128 {
        return Err(invalid());
    }
    // Config may contain private credential/helper values. A process-local
    // secret keeps its opaque UI version from becoming a dictionary oracle.
    static KEY: std::sync::OnceLock<Result<[u8; 32], String>> = std::sync::OnceLock::new();
    let key = KEY.get_or_init(|| {
        let mut bytes = [0; 32];
        getrandom::getrandom(&mut bytes)
            .map_err(|_| "Repository authorization is unavailable.".to_string())?;
        Ok(bytes)
    });
    let key = key.as_ref().map_err(Clone::clone)?;
    let mut digest = Sha256::new();
    digest.update(key);
    digest.update((values.len() as u64).to_be_bytes());
    digest.update(values);
    #[cfg(not(test))]
    for key in [
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
        "SSH_ASKPASS",
        "SSH_AUTH_SOCK",
    ] {
        digest.update(key.as_bytes());
        if let Some(value) = std::env::var_os(key) {
            digest.update([1]);
            let value = value.to_string_lossy();
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        } else {
            digest.update([0]);
        }
    }
    Ok(Configuration {
        digest: format!("{:x}", digest.finalize()),
        filters,
    })
}
