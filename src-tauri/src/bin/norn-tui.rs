fn main() {
    #[cfg(feature = "tui")]
    if let Err(error) = norn_lib::tui::run_from_env() {
        eprintln!("norn-tui: {error}");
        std::process::exit(1);
    }
    #[cfg(not(feature = "tui"))]
    {
        eprintln!(
            "norn-tui: this build does not include the terminal UI (enable the `tui` feature)."
        );
        std::process::exit(2);
    }
}
