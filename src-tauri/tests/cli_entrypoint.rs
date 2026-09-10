#![cfg(not(feature = "desktop-bundle"))]

use std::process::Command;
use std::{fs, path::PathBuf};

fn norn(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_norn"))
        .args(args)
        .env_remove("__CFBundleIdentifier")
        .output()
        .expect("norn process")
}

#[test]
fn bare_noninteractive_invocation_prints_help_instead_of_opening_the_tui() {
    let output = Command::new(env!("CARGO_BIN_EXE_norn"))
        .output()
        .expect("norn process");

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .expect("UTF-8 stdout")
        .starts_with("Usage:\n"));
    assert!(output.stderr.is_empty());
}

#[test]
fn global_help_and_version_flags_are_routed_to_the_cli() {
    for flag in ["--help", "-h"] {
        let output = norn(&[flag]);
        assert!(output.status.success(), "{flag} should succeed");
        assert!(String::from_utf8(output.stdout)
            .expect("UTF-8 stdout")
            .starts_with("Usage:\n"));
        assert!(output.stderr.is_empty());
    }

    for flag in ["--version", "-V"] {
        let output = norn(&[flag]);
        assert!(output.status.success(), "{flag} should succeed");
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 stdout"),
            format!("norn {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn unknown_top_level_commands_fail_with_usage_guidance() {
    let output = norn(&["unknown-command"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("UTF-8 stderr")
        .contains("Use `norn --help`"));
}

#[test]
fn auth_and_skill_commands_are_available_without_desktop_routing() {
    let auth = norn(&["auth", "status", "--json"]);
    assert!(auth.status.success());
    let auth_stdout = String::from_utf8(auth.stdout).expect("auth stdout");
    assert!(auth_stdout.contains("norn.auth.v1"));
    assert!(!auth_stdout.contains("token"));

    let skills = norn(&["skills", "status", "--agent", "all", "--json"]);
    assert!(skills.status.success());
    let skills_stdout = String::from_utf8(skills.stdout).expect("skills stdout");
    assert!(skills_stdout.contains("norn.skills.v1"));
    assert!(!skills_stdout.contains("/Users/"));
}

#[test]
fn auth_rejects_token_command_arguments_without_echoing_the_value() {
    let secret = "SECRET_TOKEN_MUST_NOT_APPEAR";
    let output = norn(&["auth", "login", "github", "--token", secret]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("auth stderr");
    assert!(stderr.contains("not accepted as command arguments"));
    assert!(!stderr.contains(secret));
}

#[test]
fn evaluate_runs_in_command_distribution_and_returns_baseline_regressions() {
    let corpus = "../fixtures/review-evaluation/v1/corpus.json";
    let baseline = "../fixtures/review-evaluation/v1/baseline.json";

    let passing = norn(&["evaluate", "--corpus", corpus, "--baseline", baseline]);
    assert!(passing.status.success());
    assert!(String::from_utf8(passing.stdout)
        .expect("evaluation output")
        .contains("norn.review-evaluation-result.v1"));
    assert!(passing.stderr.is_empty());

    let failing_corpus = unique_temp_path("norn-evaluate-failing-corpus", "json");
    fs::write(
        &failing_corpus,
        r#"{
  "schemaVersion": "norn.review-evaluation-corpus.v1",
  "corpusVersion": "2026.1",
  "cases": [{
    "id": "missed-finding",
    "area": "logic",
    "diffPath": "cases/missed-finding.diff",
    "provider": "codex",
    "model": "gpt-5.5",
    "configVersion": "balanced-v1",
    "durationMs": 1,
    "expected": [{
      "id": "missing",
      "disposition": "expected",
      "anchor": { "path": "src/lib.rs", "line": 1, "side": "new" }
    }],
    "observed": []
  }]
}"#,
    )
    .expect("failing corpus");

    let failing_baseline = unique_temp_path("norn-evaluate-failing-baseline", "json");
    fs::write(
        &failing_baseline,
        r#"{
  "schemaVersion": "norn.review-evaluation-result.v1",
  "corpusVersion": "2026.1",
  "minimumPrecisionMilli": 0,
  "maximumMissedExpected": 0,
  "minimumAnchorAccuracyMilli": 0
}"#,
    )
    .expect("failing baseline");

    let failing = norn(&[
        "evaluate",
        "--corpus",
        failing_corpus.to_str().expect("corpus path"),
        "--baseline",
        failing_baseline.to_str().expect("baseline path"),
    ]);
    assert_eq!(failing.status.code(), Some(1));
    assert!(String::from_utf8(failing.stdout)
        .expect("evaluation output")
        .contains("precisionMilli"));
    assert!(failing.stderr.is_empty());
    let _ = fs::remove_file(failing_corpus);
    let _ = fs::remove_file(failing_baseline);
}

fn unique_temp_path(prefix: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.{extension}", std::process::id()))
}
