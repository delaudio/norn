fn main() {
    if let Err(error) = norn_lib::tui::run_from_env() {
        eprintln!("norn-tui: {error}");
        std::process::exit(1);
    }
}
