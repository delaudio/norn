#![cfg(not(feature = "desktop-bundle"))]

use std::process::Command;

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
