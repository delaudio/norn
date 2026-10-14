fn main() {
    #[cfg(feature = "tui")]
    if let Err(error) = norn_lib::tui::run_from_env() {
        eprintln!("norn-tui: {error}");
        std::process::exit(1);
    }
    #[cfg(not(feature = "tui"))]
    match norn_lib::opentui_launcher::run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("norn-tui: {error}");
            std::process::exit(1);
        }
    }
}
