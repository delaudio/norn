#![cfg(not(feature = "desktop-bundle"))]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use norn_lib::repo_config;
use serde_json::Value;

fn temp_repo(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let repo = std::env::temp_dir().join(format!("norn-onboarding-e2e-{name}-{nonce}"));
    fs::create_dir_all(&repo).expect("create temp repo");
    repo
}

fn run_norn(repo: &Path, args: &[&str]) -> (i32, String, String) {
    let mut cli_args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    cli_args.push("--repo-path".to_string());
    cli_args.push(repo.as_os_str().to_string_lossy().to_string());

    let output = Command::new(env!("CARGO_BIN_EXE_norn"))
        .args(cli_args)
        .output()
        .expect("run norn");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn init_git_repo(repo: &Path) {
    fs::write(repo.join("README.md"), "repo\n").expect("write readme");
    assert!(Command::new("/usr/bin/git")
        .arg("-C")
        .arg(repo)
        .args(["init", "--initial-branch", "main"])
        .status()
        .expect("git init")
        .success());
    assert!(Command::new("/usr/bin/git")
        .arg("-C")
        .arg(repo)
        .args(["add", "README.md"])
        .status()
        .expect("git add readme")
        .success());
    assert!(Command::new("/usr/bin/git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.email=ci@example.com",
            "-c",
            "user.name=CI",
            "commit",
            "-m",
            "init",
        ])
        .status()
        .expect("git commit")
        .success());
}

#[test]
fn onboarding_e2e_smoke_journey_with_mixed_repo_and_remote() {
    let repo = temp_repo("mixed-journey");
    init_git_repo(&repo);
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"journey\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo");
    fs::write(
        repo.join("package.json"),
        r#"{"name":"journey","private":true}"#,
    )
    .expect("write package");

    let (code, doctor_no_remote, stderr) = run_norn(&repo, &["doctor", "--json"]);
    assert_eq!(code, 2);
    assert!(stderr.is_empty());
    let doctor_no_remote: Value =
        serde_json::from_str(&doctor_no_remote).expect("doctor no remote output");
    assert_eq!(doctor_no_remote["status"], "fail");
    let has_remote_missing = doctor_no_remote["issues"]
        .as_array()
        .expect("doctor issues")
        .iter()
        .any(|issue| issue["code"] == "repository.remoteMissing");
    assert!(has_remote_missing);

    let (code, output, stderr) = run_norn(&repo, &["init", "--json", "--dry-run"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let payload: Value = serde_json::from_str(&output).expect("init dry-run payload");
    assert_eq!(payload["schemaVersion"], "norn.init.v1");
    let project_types = payload["proposal"]["projectTypes"]
        .as_array()
        .expect("project types");
    assert!(project_types
        .iter()
        .any(|project_type| project_type == "javascript" || project_type == "rust"));
    assert!(!repo.join(".norn.yaml").exists());

    assert!(Command::new("/usr/bin/git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/example/repo.git"
        ])
        .status()
        .expect("git remote add")
        .success());

    let (code, output, stderr) = run_norn(&repo, &["init", "--json", "--yes"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(repo.join(".norn.yaml").exists());

    let config = repo_config::load_from_repo_path(&repo).expect("load generated repo config");
    assert!(config.exists);
    assert!(config.errors.is_empty());

    let payload: Value = serde_json::from_str(&output).expect("init apply output");
    assert_eq!(payload["mode"], "quick");
    assert_eq!(payload["wouldApply"], true);
    assert_eq!(payload["dryRun"], false);

    let generated = fs::read_to_string(repo.join(".norn.yaml")).expect("generated config");

    let (code, output, stderr) = run_norn(&repo, &["init", "--json", "--dry-run"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(
        fs::read_to_string(repo.join(".norn.yaml")).expect("reloaded config"),
        generated
    );
    assert_eq!(
        serde_json::from_str::<Value>(&output).expect("rerun output")["wouldApply"],
        false
    );

    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn onboarding_e2e_guided_init_requires_yes() {
    let repo = temp_repo("guided-gated");
    init_git_repo(&repo);
    assert!(Command::new("/usr/bin/git")
        .arg("-C")
        .arg(&repo)
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/example/repo.git"
        ])
        .status()
        .expect("git remote add")
        .success());

    let (code, stdout, stderr) = run_norn(&repo, &["init", "--guided"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Guided mode requires `--yes` and is not interactive in this release."));
    let (code, stdout, stderr) = run_norn(&repo, &["init", "--guided", "--yes", "--json"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("norn.init.v1"));

    let _ = fs::remove_dir_all(&repo);
}
