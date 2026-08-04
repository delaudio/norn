fn main() {
    eprintln!("`lachesi` is deprecated; use `norn` instead.");
    if let Some(exit_code) = norn_lib::cli::run_from_env_if_cli() {
        std::process::exit(exit_code);
    }
    norn_lib::run()
}
