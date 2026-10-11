#![cfg(not(feature = "desktop-bundle"))]

use std::process::Command;

fn norn(args: &[&str]) -> std::process::Output {
    let isolated = tempfile::tempdir().unwrap();
    let owned_status = args.starts_with(&["auth", "status"]);
    let mut command = if owned_status {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "isolated_cli_status_fixture_child",
            "--nocapture",
        ]);
        command
    } else {
        let mut command = Command::new(env!("CARGO_BIN_EXE_norn"));
        command.args(args);
        command
    };
    command
        .env_clear()
        .env("HOME", isolated.path())
        .env("USERPROFILE", isolated.path())
        .env("XDG_CONFIG_HOME", isolated.path().join("config"))
        .env("XDG_DATA_HOME", isolated.path().join("data"))
        .env("NORN_CONFIG_DIR", isolated.path().join("config/norn"))
        .env("NORN_DATA_DIR", isolated.path().join("data/norn"))
        .env("NORN_REVIEW_DATA_DIR", isolated.path().join("reviews"))
        .env("PATH", "/usr/bin:/bin")
        .current_dir(isolated.path());
    if owned_status {
        let result_path = isolated.path().join("cli-result.json");
        let output = command
            .env("NORN_TEST_CLI_ARGS", serde_json::to_string(args).unwrap())
            .env("NORN_TEST_CLI_RESULT", &result_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "owned CLI mock child failed");
        let (code, stdout, stderr): (i32, Vec<u8>, Vec<u8>) =
            serde_json::from_slice(&std::fs::read(result_path).unwrap()).unwrap();
        assert_eq!(code, 0);
        return std::process::Output {
            status: output.status,
            stdout,
            stderr,
        };
    }
    command.output().expect("norn process")
}

#[test]
fn isolated_cli_status_fixture_child() {
    let Some(arguments) = std::env::var_os("NORN_TEST_CLI_ARGS") else {
        return;
    };
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    let args: Vec<String> = serde_json::from_str(arguments.to_str().unwrap()).unwrap();
    assert!(args.starts_with(&["auth".into(), "status".into()]));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = norn_lib::cli::run_args(&args, &mut stdout, &mut stderr);
    std::fs::write(
        std::env::var_os("NORN_TEST_CLI_RESULT").unwrap(),
        serde_json::to_vec(&(code, stdout, stderr)).unwrap(),
    )
    .unwrap();
    std::process::exit(0);
}

#[test]
fn bare_noninteractive_invocation_prints_help_instead_of_opening_the_tui() {
    let output = norn(&[]);

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
    let auth_help = norn(&["auth", "--help"]);
    assert!(auth_help.status.success());
    assert!(String::from_utf8(auth_help.stdout)
        .unwrap()
        .contains("norn auth status"));
    let auth = norn(&["auth", "status", "--json"]);
    assert!(auth.status.success());
    let auth_stdout = String::from_utf8(auth.stdout).expect("auth stdout");
    assert!(auth_stdout.contains("norn.auth.v1"));
    assert!(!auth_stdout.contains("token"));
    let statuses: serde_json::Value = serde_json::from_str(&auth_stdout).unwrap();
    assert!(statuses["credentials"]
        .as_array()
        .unwrap()
        .iter()
        .all(|status| status["available"] == false && status["source"] == "none"));

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
