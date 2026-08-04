use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{self, ReviewTerminal};
use crate::local_repo::{configured_or_discovered_repo, find_in_path};

/// Single-quote a string for safe interpolation into a shell script.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn applescript_quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn is_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

fn find_wezterm_cli() -> Option<PathBuf> {
    if let Some(path) = find_in_path("wezterm") {
        return Some(path);
    }

    let mut candidates = vec![PathBuf::from(
        "/Applications/WezTerm.app/Contents/MacOS/wezterm",
    )];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Applications/WezTerm.app/Contents/MacOS/wezterm"));
    }

    candidates.into_iter().find(|candidate| is_file(candidate))
}

fn find_iterm_app() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("/Applications/iTerm.app")];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Applications/iTerm.app"));
    }

    candidates.into_iter().find(|candidate| candidate.is_dir())
}

fn normalize_claude_model(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn normalize_claude_effort(value: &str) -> Option<&'static str> {
    match value.trim() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "max" => Some("max"),
        _ => None,
    }
}

fn claude_command(
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> String {
    let model_arg = claude_model
        .and_then(normalize_claude_model)
        .map(|model| format!(" --model {}", shell_quote(&model)))
        .unwrap_or_default();
    let effort_arg = claude_effort
        .and_then(normalize_claude_effort)
        .map(|effort| format!(" --effort {}", shell_quote(effort)))
        .unwrap_or_default();
    format!(
        "claude{model_arg}{effort_arg} \"$(cat {})\"",
        shell_quote(&prompt_path.to_string_lossy())
    )
}

fn interactive_shell_command(
    cwd: &Path,
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> String {
    format!(
        "cd {} && {}",
        shell_quote(&cwd.to_string_lossy()),
        claude_command(prompt_path, claude_model, claude_effort),
    )
}

fn terminal_script(
    cwd: &Path,
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> String {
    format!(
        "#!/bin/zsh\ncd {}\n{}\n",
        shell_quote(&cwd.to_string_lossy()),
        claude_command(prompt_path, claude_model, claude_effort),
    )
}

fn launch_wezterm(
    cwd: &Path,
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> Result<(), String> {
    let wezterm = find_wezterm_cli().ok_or_else(|| {
        "WezTerm is not installed. Pick another terminal in Settings.".to_string()
    })?;

    std::process::Command::new(wezterm)
        .arg("start")
        .arg("--cwd")
        .arg(cwd)
        .arg("--")
        .arg("/bin/zsh")
        .arg("-lc")
        .arg(claude_command(prompt_path, claude_model, claude_effort))
        .spawn()
        .map_err(|e| format!("failed to open WezTerm: {e}"))?;

    Ok(())
}

fn launch_iterm(
    cwd: &Path,
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> Result<(), String> {
    if find_iterm_app().is_none() {
        return Err("iTerm2 is not installed. Pick another terminal in Settings.".to_string());
    }

    let script = format!(
        "tell application \"iTerm\"\n\
         activate\n\
         set newWindow to (create window with default profile)\n\
         tell current session of newWindow\n\
         write text \"{}\"\n\
         end tell\n\
         end tell",
        applescript_quote(&interactive_shell_command(
            cwd,
            prompt_path,
            claude_model,
            claude_effort
        )),
    );

    std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .spawn()
        .map_err(|e| format!("failed to open iTerm2: {e}"))?;

    Ok(())
}

fn launch_terminal(
    cwd: &Path,
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> Result<(), String> {
    let script = terminal_script(cwd, prompt_path, claude_model, claude_effort);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let script_path = std::env::temp_dir().join(format!("norn-review-{ts}.command"));
    fs::write(&script_path, script).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).map_err(|e| e.to_string())?;
    }

    std::process::Command::new("/usr/bin/open")
        .arg(&script_path)
        .spawn()
        .map_err(|e| format!("failed to open Terminal: {e}"))?;

    Ok(())
}

fn launch_with_review_terminal(
    review_terminal: ReviewTerminal,
    cwd: &Path,
    prompt_path: &Path,
    claude_model: Option<&str>,
    claude_effort: Option<&str>,
) -> Result<(), String> {
    match review_terminal {
        ReviewTerminal::WezTerm => launch_wezterm(cwd, prompt_path, claude_model, claude_effort),
        ReviewTerminal::ITerm => launch_iterm(cwd, prompt_path, claude_model, claude_effort),
        ReviewTerminal::Terminal => launch_terminal(cwd, prompt_path, claude_model, claude_effort),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTerminalOption {
    id: ReviewTerminal,
    label: &'static str,
    available: bool,
}

fn review_terminal_options() -> Vec<ReviewTerminalOption> {
    vec![
        ReviewTerminalOption {
            id: ReviewTerminal::WezTerm,
            label: "WezTerm",
            available: cfg!(target_os = "macos") && find_wezterm_cli().is_some(),
        },
        ReviewTerminalOption {
            id: ReviewTerminal::ITerm,
            label: "iTerm2",
            available: cfg!(target_os = "macos") && find_iterm_app().is_some(),
        },
        ReviewTerminalOption {
            id: ReviewTerminal::Terminal,
            label: "Terminal",
            available: cfg!(target_os = "macos"),
        },
    ]
}

#[tauri::command]
pub fn list_review_terminals() -> Result<Vec<ReviewTerminalOption>, String> {
    Ok(review_terminal_options())
}

/// Write the review payload to a temp file and open `claude` in the configured
/// terminal. The shell starts `cd`-ed into the local clone of the repo if one
/// can be found. Returns the repo path used, or None if no local clone was
/// found (runs in $HOME then).
#[tauri::command]
pub async fn launch_claude_review(
    workspace: String,
    repo: String,
    payload: String,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        if !cfg!(target_os = "macos") {
            return Err("Launching Claude is currently supported on macOS only.".to_string());
        }

        let repo_path = configured_or_discovered_repo(&workspace, &repo);
        let cwd = repo_path
            .clone()
            .or_else(dirs::home_dir)
            .ok_or_else(|| "could not resolve a working directory".to_string())?;
        let cfg = config::load();
        let review_terminal = cfg
            .review_terminal
            .ok_or_else(|| "Choose a terminal for Review with Claude first.".to_string())?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp = std::env::temp_dir();

        let prompt_path = tmp.join(format!("norn-review-{ts}.md"));
        fs::write(&prompt_path, &payload).map_err(|e| e.to_string())?;

        launch_with_review_terminal(
            review_terminal,
            &cwd,
            &prompt_path,
            cfg.claude_model.as_deref(),
            cfg.claude_effort.as_deref(),
        )?;

        Ok(repo_path.map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{
        applescript_quote, claude_command, interactive_shell_command, shell_quote, terminal_script,
    };
    use std::path::Path;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn claude_command_quotes_the_prompt_path() {
        let cmd = claude_command(Path::new("/tmp/review prompt's.md"), None, None);
        assert_eq!(cmd, "claude \"$(cat '/tmp/review prompt'\\''s.md')\"");
    }

    #[test]
    fn claude_command_includes_model_and_effort() {
        let cmd = claude_command(Path::new("/tmp/review.md"), Some("sonnet"), Some("high"));
        assert_eq!(
            cmd,
            "claude --model 'sonnet' --effort 'high' \"$(cat '/tmp/review.md')\""
        );
    }

    #[test]
    fn applescript_quote_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_quote("a\\b\"c"), "a\\\\b\\\"c");
    }

    #[test]
    fn interactive_shell_command_combines_cd_and_claude() {
        let cmd = interactive_shell_command(
            Path::new("/Users/example/dev/my repo"),
            Path::new("/tmp/review.md"),
            None,
            None,
        );
        assert_eq!(
            cmd,
            "cd '/Users/example/dev/my repo' && claude \"$(cat '/tmp/review.md')\""
        );
    }

    #[test]
    fn terminal_script_changes_directory_before_running_claude() {
        let script = terminal_script(
            Path::new("/Users/example/dev/my repo"),
            Path::new("/tmp/review.md"),
            None,
            None,
        );
        assert!(script.starts_with("#!/bin/zsh\ncd '/Users/example/dev/my repo'\n"));
        assert!(script.contains("claude \"$(cat '/tmp/review.md')\"\n"));
    }
}
