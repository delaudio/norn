//! Private stdio backend host for the OpenTUI workspace. Speaks the versioned
//! protocol on standard input and output; diagnostics go to standard error.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|value| value == "--terminal-child")
    {
        if args.len() != 2 {
            std::process::exit(2);
        }
        match norn_lib::backend::run_terminal_child(&args[1]) {
            Ok(code) => std::process::exit(code),
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(1);
            }
        }
    }
    if !args.is_empty() {
        eprintln!("This private host accepts protocol input only.");
        std::process::exit(2);
    }
    if let Err(code) = norn_lib::backend::run() {
        std::process::exit(code);
    }
}
