// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(exit_code) = norn_lib::cli::run_from_env_if_cli() {
        std::process::exit(exit_code);
    }
    norn_lib::run()
}
