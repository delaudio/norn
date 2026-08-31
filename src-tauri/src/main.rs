// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(
    all(not(debug_assertions), feature = "desktop-bundle"),
    windows_subsystem = "windows"
)]

use std::io::IsTerminal;

#[derive(Debug, PartialEq, Eq)]
enum DistributionMode {
    Desktop,
    Command,
}

fn distribution_mode(desktop_bundle: bool) -> DistributionMode {
    if desktop_bundle {
        DistributionMode::Desktop
    } else {
        DistributionMode::Command
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ZeroArgumentLaunch {
    Tui,
    Help,
}

fn zero_argument_launch(stdin_is_terminal: bool, stdout_is_terminal: bool) -> ZeroArgumentLaunch {
    if stdin_is_terminal && stdout_is_terminal {
        ZeroArgumentLaunch::Tui
    } else {
        ZeroArgumentLaunch::Help
    }
}

fn main() {
    // A bundled desktop executable is GUI-only on every platform. In
    // particular, a Windows GUI-subsystem binary has no reliable console for
    // CLI output; command behavior belongs to the separately built CLI.
    if distribution_mode(cfg!(feature = "desktop-bundle")) == DistributionMode::Desktop {
        norn_lib::run();
        return;
    }

    let args = std::env::args().collect::<Vec<_>>();
    let positional_args = args.iter().skip(1).collect::<Vec<_>>();

    if positional_args.is_empty() {
        match zero_argument_launch(
            std::io::stdin().is_terminal(),
            std::io::stdout().is_terminal(),
        ) {
            ZeroArgumentLaunch::Tui => {
                if let Err(error) = norn_lib::tui::run_from_env() {
                    eprintln!("norn-tui: {error}");
                    std::process::exit(1);
                }
            }
            ZeroArgumentLaunch::Help => std::process::exit(norn_lib::cli::print_usage()),
        }
        return;
    }

    if let Some(exit_code) = norn_lib::cli::run_from_env_if_cli() {
        std::process::exit(exit_code);
    }

    eprintln!(
        "norn: unknown command `{}`. Use `norn --help` for CLI usage or `norn-app` for GUI.",
        positional_args
            .first()
            .copied()
            .map(|value| value.as_str())
            .unwrap_or("<unknown>")
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::{distribution_mode, zero_argument_launch, DistributionMode, ZeroArgumentLaunch};

    #[test]
    fn desktop_and_command_distributions_are_compile_time_modes() {
        assert_eq!(distribution_mode(true), DistributionMode::Desktop);
        assert_eq!(distribution_mode(false), DistributionMode::Command);
    }

    #[test]
    fn command_build_routes_interactive_and_noninteractive_invocations() {
        assert_eq!(zero_argument_launch(true, true), ZeroArgumentLaunch::Tui);
        assert_eq!(zero_argument_launch(false, true), ZeroArgumentLaunch::Help);
        assert_eq!(zero_argument_launch(true, false), ZeroArgumentLaunch::Help);
    }

    #[test]
    fn compiled_binary_uses_its_declared_distribution_mode() {
        let expected = if cfg!(feature = "desktop-bundle") {
            DistributionMode::Desktop
        } else {
            DistributionMode::Command
        };
        assert_eq!(
            distribution_mode(cfg!(feature = "desktop-bundle")),
            expected
        );
    }
}
