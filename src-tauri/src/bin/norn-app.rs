fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [] => norn_lib::run(),
        [argument] if argument == "--version" => {
            println!("norn-app {}", env!("CARGO_PKG_VERSION"));
        }
        [argument] if argument == "--help" => {
            println!("Usage: norn-app [--help | --version]");
        }
        _ => {
            eprintln!("norn-app: unknown argument; use --help for usage.");
            std::process::exit(2);
        }
    }
}
