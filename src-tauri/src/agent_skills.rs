use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const SKILL_NAME: &str = "norn-review";
const MARKER_FILE: &str = ".norn-managed.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Agent {
    Codex,
    Claude,
}

impl Agent {
    fn label(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Install,
    Status,
    Uninstall,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Status => "status",
            Self::Uninstall => "uninstall",
        }
    }
}

#[derive(Debug)]
struct Args {
    action: Action,
    agents: Vec<Agent>,
    force: bool,
    format: Format,
}

#[derive(Debug, Clone)]
struct SkillPaths {
    source_root: PathBuf,
    codex_skills: PathBuf,
    claude_skills: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillStatus {
    agent: Agent,
    state: &'static str,
    installed_version: Option<String>,
    packaged_version: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillsOutput {
    schema_version: &'static str,
    action: &'static str,
    skill: &'static str,
    targets: Vec<SkillStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedMarker {
    schema_version: String,
    skill: String,
    package_version: String,
    agent: String,
}

pub fn run(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        let _ = writeln!(stdout, "{}", usage());
        return 0;
    }
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            let _ = writeln!(stderr, "{error}\n\n{}", usage());
            return 2;
        }
    };
    let paths = match SkillPaths::resolve(parsed.action == Action::Install) {
        Ok(paths) => paths,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 7;
        }
    };
    run_with_paths(parsed, &paths, stdout, stderr)
}

impl SkillPaths {
    fn resolve(require_source: bool) -> Result<Self, String> {
        let home = dirs::home_dir()
            .ok_or_else(|| "Could not resolve the user home directory.".to_string())?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        let claude_home = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"));
        Ok(Self {
            source_root: if require_source {
                resolve_packaged_source()?
            } else {
                resolve_packaged_source().unwrap_or_default()
            },
            codex_skills: codex_home.join("skills"),
            claude_skills: claude_home.join("skills"),
        })
    }

    fn destination(&self, agent: Agent) -> PathBuf {
        match agent {
            Agent::Codex => self.codex_skills.join(SKILL_NAME),
            Agent::Claude => self.claude_skills.join(SKILL_NAME),
        }
    }
}

fn resolve_packaged_source() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Some(root) = std::env::var_os("NORN_AGENT_SKILLS_DIR") {
        candidates.push(PathBuf::from(root));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(prefix) = executable.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share/norn/agent-skills"));
        }
    }
    #[cfg(debug_assertions)]
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../integrations/agent-skills"));
    candidates
        .into_iter()
        .find(|candidate| candidate.join(SKILL_NAME).join("SKILL.md").is_file())
        .ok_or_else(|| {
            "Packaged agent skill assets are missing. Reinstall or upgrade Norn, then retry."
                .to_string()
        })
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let action = match args.first().map(String::as_str) {
        Some("install") => Action::Install,
        Some("status") => Action::Status,
        Some("uninstall") => Action::Uninstall,
        Some(value) => return Err(format!("Unknown skills action `{value}`.")),
        None => return Err("Missing skills action.".to_string()),
    };
    let mut agents = vec![Agent::Codex, Agent::Claude];
    let mut force = false;
    let mut format = Format::Human;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--agent" => {
                index += 1;
                agents = match args.get(index).map(String::as_str) {
                    Some("codex") => vec![Agent::Codex],
                    Some("claude") => vec![Agent::Claude],
                    Some("all") => vec![Agent::Codex, Agent::Claude],
                    Some(_) => {
                        return Err("`--agent` must be `codex`, `claude`, or `all`.".to_string())
                    }
                    None => return Err("`--agent` requires a value.".to_string()),
                };
            }
            "--force" => force = true,
            "--json" => format = Format::Json,
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("human" | "text") => Format::Human,
                    Some("json") => Format::Json,
                    Some(_) => return Err("`--format` must be `human` or `json`.".to_string()),
                    None => return Err("`--format` requires a value.".to_string()),
                };
            }
            unknown => return Err(format!("Unknown skills option `{unknown}`.")),
        }
        index += 1;
    }
    if force && action != Action::Install {
        return Err("`--force` is supported only by `norn skills install`.".to_string());
    }
    Ok(Args {
        action,
        agents,
        force,
        format,
    })
}

fn run_with_paths(
    args: Args,
    paths: &SkillPaths,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if args.action == Action::Install {
        for agent in &args.agents {
            let destination = paths.destination(*agent);
            if destination.exists() && managed_marker(&destination).is_none() && !args.force {
                let _ = writeln!(stderr, "The {} {} destination contains unmanaged content. Re-run with `--force` only if replacement is intended.", agent.label(), SKILL_NAME);
                return 3;
            }
        }
        for agent in &args.agents {
            if let Err(error) = install_for_agent(paths, *agent, args.force) {
                let _ = writeln!(stderr, "{error}");
                return 7;
            }
        }
    } else if args.action == Action::Uninstall {
        for agent in &args.agents {
            let destination = paths.destination(*agent);
            if destination.exists() && managed_marker(&destination).is_none() {
                let _ = writeln!(
                    stderr,
                    "Refusing to remove unmanaged content from the {} skill destination.",
                    agent.label()
                );
                return 3;
            }
        }
        for agent in &args.agents {
            if let Err(error) = uninstall_for_agent(paths, *agent) {
                let _ = writeln!(stderr, "{error}");
                return 3;
            }
        }
    }

    let targets = args
        .agents
        .iter()
        .map(|agent| status_for_agent(paths, *agent))
        .collect::<Vec<_>>();
    let output = SkillsOutput {
        schema_version: "norn.skills.v1",
        action: args.action.label(),
        skill: SKILL_NAME,
        targets,
    };
    match args.format {
        Format::Json => match serde_json::to_string_pretty(&output) {
            Ok(json) => {
                let _ = writeln!(stdout, "{json}");
            }
            Err(error) => {
                let _ = writeln!(stderr, "Failed to serialize skill status: {error}");
                return 7;
            }
        },
        Format::Human => {
            for target in output.targets {
                let version = target.installed_version.as_deref().unwrap_or("-");
                let _ = writeln!(
                    stdout,
                    "- {}: {} (installed {version}, packaged {})",
                    target.agent.label(),
                    target.state,
                    target.packaged_version
                );
            }
        }
    }
    0
}

fn status_for_agent(paths: &SkillPaths, agent: Agent) -> SkillStatus {
    let destination = paths.destination(agent);
    let marker = managed_marker(&destination);
    let (state, installed_version) = if let Some(marker) = marker {
        let state = if marker.package_version == env!("CARGO_PKG_VERSION") {
            "managed"
        } else {
            "upgrade_available"
        };
        (state, Some(marker.package_version))
    } else if destination.exists() {
        ("unmanaged", None)
    } else {
        ("missing", None)
    };
    SkillStatus {
        agent,
        state,
        installed_version,
        packaged_version: env!("CARGO_PKG_VERSION"),
    }
}

fn managed_marker(destination: &Path) -> Option<ManagedMarker> {
    let marker = fs::read_to_string(destination.join(MARKER_FILE)).ok()?;
    let marker = serde_json::from_str::<ManagedMarker>(&marker).ok()?;
    (marker.schema_version == "norn.skill.v1" && marker.skill == SKILL_NAME).then_some(marker)
}

fn install_for_agent(paths: &SkillPaths, agent: Agent, force: bool) -> Result<(), String> {
    let source = paths.source_root.join(SKILL_NAME);
    let destination = paths.destination(agent);
    let parent = destination
        .parent()
        .ok_or_else(|| "Invalid skill destination.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to prepare {} skills directory: {error}",
            agent.label()
        )
    })?;
    if destination.exists() && managed_marker(&destination).is_none() && !force {
        return Err(format!(
            "The {} {} destination contains unmanaged content.",
            agent.label(),
            SKILL_NAME
        ));
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = parent.join(format!(
        ".{SKILL_NAME}.staging-{}-{nonce}",
        std::process::id()
    ));
    let backup = parent.join(format!(
        ".{SKILL_NAME}.backup-{}-{nonce}",
        std::process::id()
    ));
    if let Err(error) = copy_directory(&source, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let marker = ManagedMarker {
        schema_version: "norn.skill.v1".to_string(),
        skill: SKILL_NAME.to_string(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        agent: agent.label().to_string(),
    };
    let marker_json = serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?;
    fs::write(staging.join(MARKER_FILE), marker_json)
        .map_err(|error| format!("Failed to stage managed skill metadata: {error}"))?;

    let had_destination = destination.exists();
    if had_destination {
        fs::rename(&destination, &backup).map_err(|error| {
            format!(
                "Failed to stage the existing {} skill for replacement: {error}",
                agent.label()
            )
        })?;
    }
    if let Err(error) = fs::rename(&staging, &destination) {
        if had_destination {
            let _ = fs::rename(&backup, &destination);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(format!(
            "Failed to atomically install the {} skill: {error}",
            agent.label()
        ));
    }
    if had_destination {
        // Replacement is already committed at this point. Cleanup cannot turn
        // a successful atomic install into a reported failure.
        let _ = fs::remove_dir_all(&backup);
    }
    Ok(())
}

fn uninstall_for_agent(paths: &SkillPaths, agent: Agent) -> Result<(), String> {
    let destination = paths.destination(agent);
    if !destination.exists() {
        return Ok(());
    }
    if managed_marker(&destination).is_none() {
        return Err(format!(
            "Refusing to remove unmanaged content from the {} skill destination.",
            agent.label()
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| "Invalid skill destination.".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tombstone = parent.join(format!(
        ".{SKILL_NAME}.uninstall-{}-{nonce}",
        std::process::id()
    ));
    fs::rename(&destination, &tombstone).map_err(|error| {
        format!(
            "Failed to stage the {} skill for removal: {error}",
            agent.label()
        )
    })?;
    if let Err(error) = fs::remove_dir_all(&tombstone) {
        let _ = fs::rename(&tombstone, &destination);
        return Err(format!(
            "Failed to remove the {} skill: {error}",
            agent.label()
        ));
    }
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
    }
    fs::create_dir(destination)
        .map_err(|error| format!("Failed to stage skill directory: {error}"))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("Failed to read packaged skill assets: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("Failed to read packaged skill asset: {error}"))?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if file_type.is_symlink() {
            return Err("Packaged skill assets must not contain symbolic links.".to_string());
        }
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)
                .map_err(|error| format!("Failed to copy packaged skill asset: {error}"))?;
        }
    }
    Ok(())
}

pub fn usage() -> &'static str {
    "Usage:
  norn skills install [--agent codex|claude|all] [--force]
                      [--format human|json] [--json]
  norn skills status [--agent codex|claude|all]
                     [--format human|json] [--json]
  norn skills uninstall [--agent codex|claude|all]
                        [--format human|json] [--json]

The default target is `all`. Install and upgrade are atomic per agent.
Unmanaged destination content is preserved unless install receives `--force`;
uninstall removes only Norn-managed content."
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, SkillPaths) {
        let root = tempfile::tempdir().expect("tempdir");
        let source_root = root.path().join("packaged");
        let skill = source_root.join(SKILL_NAME);
        fs::create_dir_all(skill.join("agents")).expect("source dirs");
        fs::write(skill.join("SKILL.md"), "managed skill\n").expect("skill");
        fs::write(skill.join("agents/openai.yaml"), "interface: test\n").expect("metadata");
        let paths = SkillPaths {
            source_root,
            codex_skills: root.path().join("codex/skills"),
            claude_skills: root.path().join("claude/skills"),
        };
        (root, paths)
    }

    fn args(action: Action, agents: Vec<Agent>, force: bool) -> Args {
        Args {
            action,
            agents,
            force,
            format: Format::Json,
        }
    }

    fn run_test(args: Args, paths: &SkillPaths) -> (i32, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_with_paths(args, paths, &mut stdout, &mut stderr);
        (
            code,
            String::from_utf8(stdout).expect("stdout"),
            String::from_utf8(stderr).expect("stderr"),
        )
    }

    #[test]
    fn installs_repeats_and_uninstalls_for_both_agents() {
        let (_root, paths) = fixture();
        for _ in 0..2 {
            let (code, stdout, stderr) = run_test(
                args(Action::Install, vec![Agent::Codex, Agent::Claude], false),
                &paths,
            );
            assert_eq!(code, 0);
            assert!(stderr.is_empty());
            assert!(stdout.contains("\"state\": \"managed\""));
        }
        for agent in [Agent::Codex, Agent::Claude] {
            let destination = paths.destination(agent);
            assert!(destination.join("SKILL.md").is_file());
            assert!(destination.join("agents/openai.yaml").is_file());
            assert!(destination.join(MARKER_FILE).is_file());
        }
        let (code, _, stderr) = run_test(
            args(Action::Uninstall, vec![Agent::Codex, Agent::Claude], false),
            &paths,
        );
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(!paths.destination(Agent::Codex).exists());
        assert!(!paths.destination(Agent::Claude).exists());
    }

    #[test]
    fn preserves_unmanaged_conflicts_unless_force_is_explicit() {
        let (_root, paths) = fixture();
        let destination = paths.destination(Agent::Codex);
        fs::create_dir_all(&destination).expect("unmanaged dir");
        fs::write(destination.join("custom.txt"), "keep").expect("custom");
        let (code, _, stderr) = run_test(args(Action::Install, vec![Agent::Codex], false), &paths);
        assert_eq!(code, 3);
        assert!(stderr.contains("unmanaged content"));
        assert!(destination.join("custom.txt").is_file());

        let (code, _, stderr) = run_test(args(Action::Install, vec![Agent::Codex], true), &paths);
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(!destination.join("custom.txt").exists());
        assert!(destination.join(MARKER_FILE).is_file());
    }

    #[test]
    fn status_is_versioned_and_does_not_expose_paths() {
        let (root, paths) = fixture();
        let (code, stdout, stderr) = run_test(
            args(Action::Status, vec![Agent::Codex, Agent::Claude], false),
            &paths,
        );
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
        assert!(!stdout.contains(root.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn managed_install_upgrades_an_older_marker_and_skill_content() {
        let (_root, paths) = fixture();
        let destination = paths.destination(Agent::Codex);
        let (code, _, stderr) = run_test(args(Action::Install, vec![Agent::Codex], false), &paths);
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        let mut marker = managed_marker(&destination).expect("managed marker");
        marker.package_version = "0.0.1".to_string();
        fs::write(
            destination.join(MARKER_FILE),
            serde_json::to_vec_pretty(&marker).expect("marker json"),
        )
        .expect("old marker");
        fs::write(destination.join("SKILL.md"), "old content\n").expect("old skill");

        let (code, stdout, stderr) =
            run_test(args(Action::Install, vec![Agent::Codex], false), &paths);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.contains("\"state\": \"managed\""));
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).expect("upgraded skill"),
            "managed skill\n"
        );
        assert_eq!(
            managed_marker(&destination)
                .expect("upgraded marker")
                .package_version,
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn uninstall_refuses_unmanaged_content() {
        let (_root, paths) = fixture();
        let destination = paths.destination(Agent::Claude);
        fs::create_dir_all(&destination).expect("unmanaged dir");
        let (code, _, stderr) =
            run_test(args(Action::Uninstall, vec![Agent::Claude], false), &paths);
        assert_eq!(code, 3);
        assert!(stderr.contains("Refusing to remove unmanaged"));
        assert!(destination.exists());
    }
}
