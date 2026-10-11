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

#[test]
fn workspace_data_cli_preview_confirm_repeat_and_export_use_isolated_storage() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let source = data.path().join("desktop-export.json");
    fs::write(&source,r#"{"schemaVersion":1,"source":"desktopLocalStorage","entries":[{"key":"norn.reviewPrompt.v1.owner/repo","value":"custom instructions"}]}"#).unwrap();
    let original = fs::read(&source).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_norn"))
            .env("NORN_DATA_DIR", data.path())
            .env("NORN_CONFIG_DIR", config.path())
            .args(args)
            .output()
            .unwrap()
    };
    let preview = run(&[
        "data",
        "preview",
        source.to_str().unwrap(),
        "--provider",
        "github",
    ]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let value: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(value["additionsCount"], 1);
    assert!(!data.path().join("workspace-data-v1.json").exists());
    let unconfirmed = run(&["data", "import", source.to_str().unwrap()]);
    assert_eq!(unconfirmed.status.code(), Some(2));
    assert!(!data.path().join("workspace-data-v1.json").exists());
    let import = run(&[
        "data",
        "import",
        source.to_str().unwrap(),
        "--provider",
        "github",
        "--input-version",
        value["inputVersion"].as_str().unwrap(),
        "--destination-version",
        value["destinationVersion"].as_str().unwrap(),
        "--confirm",
    ]);
    assert!(
        import.status.success(),
        "{}",
        String::from_utf8_lossy(&import.stderr)
    );
    let result: Value = serde_json::from_slice(&import.stdout).unwrap();
    assert_eq!(result["imported"], 1);
    let destination = fs::read(data.path().join("workspace-data-v1.json")).unwrap();
    let repeat_preview = run(&[
        "data",
        "preview",
        source.to_str().unwrap(),
        "--provider",
        "github",
    ]);
    assert!(repeat_preview.status.success());
    let value: Value = serde_json::from_slice(&repeat_preview.stdout).unwrap();
    assert_eq!(value["unchangedCount"], 1);
    let repeat = run(&[
        "data",
        "import",
        source.to_str().unwrap(),
        "--provider",
        "github",
        "--input-version",
        value["inputVersion"].as_str().unwrap(),
        "--destination-version",
        value["destinationVersion"].as_str().unwrap(),
        "--confirm",
    ]);
    assert!(repeat.status.success());
    assert_eq!(
        fs::read(data.path().join("workspace-data-v1.json")).unwrap(),
        destination
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    let export = data.path().join("new-export.json");
    let exported = run(&["data", "export", export.to_str().unwrap()]);
    assert!(exported.status.success());
    let before = fs::read(&export).unwrap();
    assert_eq!(
        run(&["data", "export", export.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(fs::read(&export).unwrap(), before);
}
